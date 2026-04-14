use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::{broadcast, mpsc, Mutex};
use tracing::{debug, info, warn};

use bpane_protocol::channel::ChannelId;
use bpane_protocol::frame::Frame;
use bpane_protocol::{ControlMessage, TileMessage};

use crate::relay::Relay;

/// Broadcast channel capacity. At 30fps, 1024 frames is ~34 seconds of buffer.
const BROADCAST_CAPACITY: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscribeError {
    ViewerLimitReached { max_viewers: u32 },
}

impl std::fmt::Display for SubscribeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ViewerLimitReached { max_viewers } => {
                write!(f, "viewer limit reached (max {max_viewers})")
            }
        }
    }
}

impl std::error::Error for SubscribeError {}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SessionTelemetrySnapshot {
    pub browser_clients: u32,
    pub viewer_clients: u32,
    pub max_viewers: u32,
    pub viewer_slots_remaining: u32,
    pub exclusive_browser_owner: bool,
    pub mcp_owner: bool,
    pub resolution: (u16, u16),
    pub joins_accepted: u64,
    pub joins_rejected_viewer_cap: u64,
    pub last_join_latency_ms: u64,
    pub average_join_latency_ms: f64,
    pub max_join_latency_ms: u64,
    pub full_refresh_requests: u64,
    pub full_refresh_tiles_requested: u64,
    pub last_full_refresh_tiles: u64,
    pub max_full_refresh_tiles: u64,
}

/// Handle returned to each connecting client.
#[derive(Debug)]
pub struct ClientHandle {
    /// Broadcast receiver for host->client frames.
    pub from_host: broadcast::Receiver<Arc<Frame>>,
    /// Sender for client->host frames (cloned from hub).
    pub to_host: mpsc::Sender<Frame>,
    /// Unique client ID within this session.
    pub client_id: u64,
    /// Whether this client is the session owner (first to connect).
    pub is_owner: bool,
    /// Frames to send immediately on connect (cached SessionReady + last keyframe).
    pub initial_frames: Vec<Arc<Frame>>,
    /// The current locked resolution (only set for non-owner clients).
    pub locked_resolution: Option<(u16, u16)>,
}

/// Central fan-out/merge coordinator for a single host agent session.
///
/// Maintains exactly ONE connection to the host agent and broadcasts
/// all host-to-client frames to N subscribers. Client-to-host input
/// is naturally merged via cloned mpsc senders.
pub struct SessionHub {
    broadcast_tx: broadcast::Sender<Arc<Frame>>,
    to_agent: mpsc::Sender<Frame>,
    max_viewers: u32,
    exclusive_browser_owner: bool,
    current_resolution: Arc<Mutex<(u16, u16)>>,
    connected_clients: Mutex<Vec<u64>>,
    client_counter: AtomicU64,
    client_count: AtomicU32,
    owner_id: AtomicU64,
    active: Arc<AtomicBool>,
    cached_session_ready: Arc<Mutex<Option<Arc<Frame>>>>,
    cached_keyframe: Arc<Mutex<Option<Arc<Frame>>>>,
    cached_grid_config: Arc<Mutex<Option<Arc<Frame>>>>,
    /// When true, MCP agent owns the session — all browser clients are viewers.
    mcp_is_owner: AtomicBool,
    /// Resolution set by the MCP agent (used for ResolutionLocked).
    mcp_resolution: Mutex<Option<(u16, u16)>>,
    joins_accepted: AtomicU64,
    joins_rejected_viewer_cap: AtomicU64,
    total_join_latency_ms: AtomicU64,
    last_join_latency_ms: AtomicU64,
    max_join_latency_ms: AtomicU64,
    full_refresh_requests: AtomicU64,
    full_refresh_tiles_requested: AtomicU64,
    last_full_refresh_tiles: AtomicU64,
    max_full_refresh_tiles: AtomicU64,
    _relay_handle: tokio::task::JoinHandle<()>,
    _pump_handle: tokio::task::JoinHandle<()>,
}

impl SessionHub {
    /// Create a new hub by connecting to the host agent Unix socket.
    /// Spawns a pump task that reads from the agent and broadcasts to all subscribers.
    pub async fn new(
        agent_socket_path: &str,
        max_viewers: u32,
        exclusive_browser_owner: bool,
    ) -> anyhow::Result<Self> {
        let relay = Relay::new(agent_socket_path.to_string());
        let (from_agent_rx, to_agent_tx, relay_handle) = relay.connect().await?;

        let (broadcast_tx, _) = broadcast::channel::<Arc<Frame>>(BROADCAST_CAPACITY);
        let current_resolution = Arc::new(Mutex::new((0u16, 0u16)));
        let cached_session_ready: Arc<Mutex<Option<Arc<Frame>>>> = Arc::new(Mutex::new(None));
        let cached_keyframe: Arc<Mutex<Option<Arc<Frame>>>> = Arc::new(Mutex::new(None));
        let cached_grid_config: Arc<Mutex<Option<Arc<Frame>>>> = Arc::new(Mutex::new(None));
        let active = Arc::new(AtomicBool::new(true));

        let pump_handle = tokio::spawn(Self::pump_loop(
            from_agent_rx,
            broadcast_tx.clone(),
            current_resolution.clone(),
            cached_session_ready.clone(),
            cached_keyframe.clone(),
            cached_grid_config.clone(),
            active.clone(),
        ));

        Ok(Self {
            broadcast_tx,
            to_agent: to_agent_tx,
            max_viewers,
            exclusive_browser_owner,
            current_resolution,
            connected_clients: Mutex::new(Vec::new()),
            client_counter: AtomicU64::new(0),
            client_count: AtomicU32::new(0),
            owner_id: AtomicU64::new(0),
            active,
            mcp_is_owner: AtomicBool::new(false),
            mcp_resolution: Mutex::new(None),
            cached_session_ready,
            cached_keyframe,
            cached_grid_config,
            joins_accepted: AtomicU64::new(0),
            joins_rejected_viewer_cap: AtomicU64::new(0),
            total_join_latency_ms: AtomicU64::new(0),
            last_join_latency_ms: AtomicU64::new(0),
            max_join_latency_ms: AtomicU64::new(0),
            full_refresh_requests: AtomicU64::new(0),
            full_refresh_tiles_requested: AtomicU64::new(0),
            last_full_refresh_tiles: AtomicU64::new(0),
            max_full_refresh_tiles: AtomicU64::new(0),
            _relay_handle: relay_handle,
            _pump_handle: pump_handle,
        })
    }

    /// Reads frames from the agent relay and broadcasts to all subscribers.
    /// Also caches SessionReady, ResolutionAck, GridConfig, and keyframes for late-joining clients.
    async fn pump_loop(
        mut from_agent: mpsc::Receiver<Frame>,
        broadcast_tx: broadcast::Sender<Arc<Frame>>,
        resolution: Arc<Mutex<(u16, u16)>>,
        cached_session_ready: Arc<Mutex<Option<Arc<Frame>>>>,
        cached_keyframe: Arc<Mutex<Option<Arc<Frame>>>>,
        cached_grid_config: Arc<Mutex<Option<Arc<Frame>>>>,
        active: Arc<AtomicBool>,
    ) {
        while let Some(frame) = from_agent.recv().await {
            // Cache important frames for late-joining clients
            if frame.channel == ChannelId::Control && !frame.payload.is_empty() {
                match frame.payload[0] {
                    0x02 if frame.payload.len() >= 5 => {
                        // ResolutionAck — update current resolution
                        let w = u16::from_le_bytes([frame.payload[1], frame.payload[2]]);
                        let h = u16::from_le_bytes([frame.payload[3], frame.payload[4]]);
                        *resolution.lock().await = (w, h);
                    }
                    0x03 => {
                        // SessionReady — cache for late joiners
                        *cached_session_ready.lock().await = Some(Arc::new(frame.clone()));
                    }
                    _ => {}
                }
            }

            // Cache GridConfig (Tiles channel, tag 0x01) for late joiners.
            // Without this, the tile compositor never initializes → black screen.
            if frame.channel == ChannelId::Tiles
                && !frame.payload.is_empty()
                && frame.payload[0] == 0x01
            {
                *cached_grid_config.lock().await = Some(Arc::new(frame.clone()));
            }

            // Cache keyframes for late joiners (is_keyframe at byte offset 8)
            if frame.channel == ChannelId::Video && frame.payload.len() > 8 {
                let is_keyframe = frame.payload[8] != 0;
                if is_keyframe {
                    *cached_keyframe.lock().await = Some(Arc::new(frame.clone()));
                }
            }

            let arc_frame = Arc::new(frame);
            // broadcast::send returns Err only if there are no receivers — that's OK.
            let _ = broadcast_tx.send(arc_frame);
        }

        active.store(false, Ordering::Relaxed);
        info!("session hub pump ended (agent disconnected)");
    }

    /// Subscribe a new client to the hub.
    /// The first subscriber becomes the session owner.
    pub async fn subscribe(&self) -> Result<ClientHandle, SubscribeError> {
        let join_started = Instant::now();
        let client_id = self.client_counter.fetch_add(1, Ordering::Relaxed) + 1;
        let mcp_is_owner = self.mcp_is_owner.load(Ordering::Relaxed);
        let mut connected_clients = self.connected_clients.lock().await;
        let prev_count = connected_clients.len() as u32;
        let current_owner_id = self.owner_id.load(Ordering::Relaxed);
        let exclusive_browser_owner = self.exclusive_browser_owner;

        // MCP owner takes precedence: all browser clients are viewers.
        let is_owner = if mcp_is_owner {
            false
        } else if !exclusive_browser_owner {
            true
        } else {
            current_owner_id == 0
        };

        if !is_owner {
            let owner_connected = !mcp_is_owner && current_owner_id != 0 && prev_count > 0;
            let current_viewers = if owner_connected {
                prev_count.saturating_sub(1)
            } else {
                prev_count
            };
            if current_viewers >= self.max_viewers {
                self.joins_rejected_viewer_cap
                    .fetch_add(1, Ordering::Relaxed);
                return Err(SubscribeError::ViewerLimitReached {
                    max_viewers: self.max_viewers,
                });
            }
        }

        connected_clients.push(client_id);
        if is_owner {
            self.owner_id.store(client_id, Ordering::Relaxed);
        }
        self.client_count
            .store(connected_clients.len() as u32, Ordering::Relaxed);
        drop(connected_clients);

        // Gather initial frames for late-joining clients.
        // Order matters: SessionReady first, then GridConfig, then keyframe.
        let mut initial_frames = Vec::new();
        if let Some(sr) = self.cached_session_ready.lock().await.as_ref() {
            initial_frames.push(sr.clone());
        }
        if let Some(gc) = self.cached_grid_config.lock().await.as_ref() {
            initial_frames.push(gc.clone());
        }
        if let Some(kf) = self.cached_keyframe.lock().await.as_ref() {
            initial_frames.push(kf.clone());
        }

        let locked_resolution = if is_owner {
            None
        } else if mcp_is_owner {
            // Use MCP's resolution if MCP owns the session
            *self.mcp_resolution.lock().await
        } else if exclusive_browser_owner {
            let (w, h) = *self.current_resolution.lock().await;
            if w > 0 && h > 0 {
                Some((w, h))
            } else {
                None
            }
        } else {
            None
        };

        // Any late joiner needs a full tile refresh so it sees the current
        // screen state immediately, regardless of interaction policy.
        if prev_count > 0 {
            let tiles_requested = self.request_full_refresh().await;
            if tiles_requested > 0 {
                self.record_refresh_burst(tiles_requested);
            }
        }

        self.record_join_latency(join_started.elapsed());
        self.joins_accepted.fetch_add(1, Ordering::Relaxed);

        Ok(ClientHandle {
            from_host: self.broadcast_tx.subscribe(),
            to_host: self.to_agent.clone(),
            client_id,
            is_owner,
            initial_frames,
            locked_resolution,
        })
    }

    /// Called when a client disconnects.
    pub async fn unsubscribe(&self, client_id: u64) {
        let mut connected_clients = self.connected_clients.lock().await;
        connected_clients.retain(|id| *id != client_id);
        self.client_count
            .store(connected_clients.len() as u32, Ordering::Relaxed);

        if self.owner_id.load(Ordering::Relaxed) == client_id {
            let next_owner = if self.mcp_is_owner.load(Ordering::Relaxed) {
                0
            } else {
                connected_clients.first().copied().unwrap_or(0)
            };
            self.owner_id.store(next_owner, Ordering::Relaxed);
            if next_owner != 0 {
                debug!(
                    client_id,
                    next_owner, "promoted existing viewer to session owner"
                );
            }
        } else if self.owner_id.load(Ordering::Relaxed) == 0
            && !self.mcp_is_owner.load(Ordering::Relaxed)
        {
            if let Some(next_owner) = connected_clients.first().copied() {
                self.owner_id.store(next_owner, Ordering::Relaxed);
                debug!(client_id, next_owner, "restored missing session owner");
            }
        }
    }

    pub fn is_browser_owner(&self, client_id: u64) -> bool {
        if self.mcp_is_owner() {
            return false;
        }
        if !self.exclusive_browser_owner {
            return true;
        }
        let owner_id = self.owner_id.load(Ordering::Relaxed);
        owner_id == 0 || owner_id == client_id
    }

    /// Handle a resize request from a client.
    /// Only the owner's requests are forwarded to the host.
    /// Non-owner requests are denied and the current resolution is returned.
    pub async fn request_resize(&self, client_id: u64, width: u16, height: u16) -> ResizeResult {
        // When MCP owns the session, deny all browser resize requests.
        if self.mcp_is_owner.load(Ordering::Relaxed) {
            if let Some((w, h)) = *self.mcp_resolution.lock().await {
                return ResizeResult::Locked(w, h);
            }
        }

        if !self.exclusive_browser_owner {
            let msg = ControlMessage::ResolutionRequest { width, height };
            if self.to_agent.send(msg.to_frame()).await.is_err() {
                warn!("failed to forward collaborative resize to agent");
            }
            return ResizeResult::Applied;
        }

        let owner = self.owner_id.load(Ordering::Relaxed);
        if client_id == owner {
            let msg = ControlMessage::ResolutionRequest { width, height };
            if self.to_agent.send(msg.to_frame()).await.is_err() {
                warn!("failed to forward resize to agent");
            }
            ResizeResult::Applied
        } else {
            let (cur_w, cur_h) = *self.current_resolution.lock().await;
            if cur_w > 0 && cur_h > 0 {
                ResizeResult::Locked(cur_w, cur_h)
            } else {
                ResizeResult::Locked(0, 0)
            }
        }
    }

    /// Get the current session resolution.
    pub async fn current_resolution(&self) -> (u16, u16) {
        *self.current_resolution.lock().await
    }

    /// Send CacheMiss for every tile in the grid to trigger a full refresh.
    /// The host will resend all tiles, which get broadcast to all subscribers.
    async fn request_full_refresh(&self) -> u64 {
        let gc = self.cached_grid_config.lock().await;
        let Some(gc_frame) = gc.as_ref() else {
            warn!("no cached GridConfig — cannot request full refresh");
            return 0;
        };

        // GridConfig payload: tag(1) + tile_size(2) + cols(2) + rows(2) + screen_w(2) + screen_h(2) = 11 bytes
        if gc_frame.payload.len() < 7 {
            warn!("cached GridConfig too short");
            return 0;
        }
        let cols = u16::from_le_bytes([gc_frame.payload[3], gc_frame.payload[4]]);
        let rows = u16::from_le_bytes([gc_frame.payload[5], gc_frame.payload[6]]);
        drop(gc); // release lock before sending

        debug!(cols, rows, "requesting full tile refresh for late joiner");

        for row in 0..rows {
            for col in 0..cols {
                let msg = TileMessage::CacheMiss {
                    frame_seq: 0,
                    col,
                    row,
                    hash: 0,
                };
                if self.to_agent.send(msg.to_frame()).await.is_err() {
                    warn!("failed to send CacheMiss to agent");
                    return u64::from(row) * u64::from(cols) + u64::from(col);
                }
            }
        }
        u64::from(cols) * u64::from(rows)
    }

    /// Register the MCP agent as session owner with the given resolution.
    /// Sends a ResolutionRequest to the host agent.
    /// All browser clients will be treated as viewers with locked resolution.
    pub async fn set_mcp_owner(&self, width: u16, height: u16) {
        *self.mcp_resolution.lock().await = Some((width, height));
        self.mcp_is_owner.store(true, Ordering::Relaxed);

        // Send resolution request to the host agent
        let msg = ControlMessage::ResolutionRequest { width, height };
        if self.to_agent.send(msg.to_frame()).await.is_err() {
            warn!("failed to send MCP resolution request to agent");
        }

        // Broadcast ResolutionLocked to all currently connected browser clients
        let locked = ControlMessage::ResolutionLocked { width, height };
        let _ = self.broadcast_tx.send(Arc::new(locked.to_frame()));

        info!(width, height, "MCP agent registered as session owner");
    }

    /// Remove MCP agent ownership. Next browser client to subscribe becomes owner.
    pub async fn clear_mcp_owner(&self) {
        self.mcp_is_owner.store(false, Ordering::Relaxed);
        *self.mcp_resolution.lock().await = None;
        info!("MCP agent ownership cleared");
    }

    /// Whether the MCP agent currently owns the session.
    pub fn mcp_is_owner(&self) -> bool {
        self.mcp_is_owner.load(Ordering::Relaxed)
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }

    pub fn client_count(&self) -> u32 {
        self.client_count.load(Ordering::Relaxed)
    }

    pub fn viewer_count(&self) -> u32 {
        let clients = self.client_count();
        if self.mcp_is_owner() {
            clients
        } else if self.exclusive_browser_owner
            && self.owner_id.load(Ordering::Relaxed) != 0
            && clients > 0
        {
            clients.saturating_sub(1)
        } else if self.exclusive_browser_owner {
            clients
        } else {
            0
        }
    }

    pub async fn telemetry_snapshot(&self) -> SessionTelemetrySnapshot {
        let resolution = self.current_resolution().await;
        let browser_clients = self.client_count();
        let viewer_clients = self.viewer_count();
        let joins_accepted = self.joins_accepted.load(Ordering::Relaxed);
        let total_join_latency_ms = self.total_join_latency_ms.load(Ordering::Relaxed);

        SessionTelemetrySnapshot {
            browser_clients,
            viewer_clients,
            max_viewers: self.max_viewers,
            viewer_slots_remaining: self.max_viewers.saturating_sub(viewer_clients),
            exclusive_browser_owner: self.exclusive_browser_owner,
            mcp_owner: self.mcp_is_owner(),
            resolution,
            joins_accepted,
            joins_rejected_viewer_cap: self.joins_rejected_viewer_cap.load(Ordering::Relaxed),
            last_join_latency_ms: self.last_join_latency_ms.load(Ordering::Relaxed),
            average_join_latency_ms: if joins_accepted == 0 {
                0.0
            } else {
                total_join_latency_ms as f64 / joins_accepted as f64
            },
            max_join_latency_ms: self.max_join_latency_ms.load(Ordering::Relaxed),
            full_refresh_requests: self.full_refresh_requests.load(Ordering::Relaxed),
            full_refresh_tiles_requested: self.full_refresh_tiles_requested.load(Ordering::Relaxed),
            last_full_refresh_tiles: self.last_full_refresh_tiles.load(Ordering::Relaxed),
            max_full_refresh_tiles: self.max_full_refresh_tiles.load(Ordering::Relaxed),
        }
    }

    fn record_join_latency(&self, elapsed: std::time::Duration) {
        let join_ms = elapsed.as_millis().min(u128::from(u64::MAX)) as u64;
        self.total_join_latency_ms
            .fetch_add(join_ms, Ordering::Relaxed);
        self.last_join_latency_ms.store(join_ms, Ordering::Relaxed);
        update_max(&self.max_join_latency_ms, join_ms);
    }

    fn record_refresh_burst(&self, tiles_requested: u64) {
        self.full_refresh_requests.fetch_add(1, Ordering::Relaxed);
        self.full_refresh_tiles_requested
            .fetch_add(tiles_requested, Ordering::Relaxed);
        self.last_full_refresh_tiles
            .store(tiles_requested, Ordering::Relaxed);
        update_max(&self.max_full_refresh_tiles, tiles_requested);
    }
}

fn update_max(target: &AtomicU64, value: u64) {
    let mut current = target.load(Ordering::Relaxed);
    while value > current {
        match target.compare_exchange(current, value, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => break,
            Err(next) => current = next,
        }
    }
}

/// Result of a resize request.
pub enum ResizeResult {
    /// Request was forwarded to the host agent.
    Applied,
    /// Resolution is locked by the owner — use these dimensions.
    Locked(u16, u16),
}

#[cfg(test)]
mod tests {
    use super::*;
    use bpane_protocol::frame::FrameDecoder;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::UnixListener;

    /// Create a mock agent that echoes frames back and responds to
    /// ResolutionRequest with ResolutionAck.
    async fn mock_agent(sock_path: &str) -> tokio::task::JoinHandle<()> {
        let listener = UnixListener::bind(sock_path).unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 64 * 1024];
            let mut decoder = FrameDecoder::new();

            // Send a SessionReady immediately
            let ready = ControlMessage::SessionReady {
                version: 1,
                flags: bpane_protocol::SessionFlags::new(0x20),
            };
            stream.write_all(&ready.to_frame().encode()).await.unwrap();

            loop {
                let n = match stream.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => n,
                    Err(_) => break,
                };
                decoder.push(&buf[..n]).unwrap();
                loop {
                    match decoder.next_frame() {
                        Ok(Some(frame)) => {
                            // If it's a ResolutionRequest, respond with ResolutionAck
                            if frame.channel == ChannelId::Control
                                && !frame.payload.is_empty()
                                && frame.payload[0] == 0x01
                                && frame.payload.len() >= 5
                            {
                                let ack = ControlMessage::ResolutionAck {
                                    width: u16::from_le_bytes([frame.payload[1], frame.payload[2]]),
                                    height: u16::from_le_bytes([
                                        frame.payload[3],
                                        frame.payload[4],
                                    ]),
                                };
                                stream.write_all(&ack.to_frame().encode()).await.unwrap();
                            }
                        }
                        Ok(None) => break,
                        Err(e) => panic!("decode error: {e}"),
                    }
                }
            }
        })
    }

    #[tokio::test]
    async fn first_subscriber_is_owner() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("test.sock");
        let sock_str = sock.to_str().unwrap();

        let _agent = mock_agent(sock_str).await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let hub = SessionHub::new(sock_str, 10, true).await.unwrap();
        let c1 = hub.subscribe().await.unwrap();
        assert!(c1.is_owner);
        assert_eq!(c1.client_id, 1);

        let c2 = hub.subscribe().await.unwrap();
        assert!(!c2.is_owner);
        assert_eq!(c2.client_id, 2);
    }

    #[tokio::test]
    async fn collaborative_mode_keeps_late_joiners_interactive() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("test.sock");
        let sock_str = sock.to_str().unwrap();

        let _agent = mock_agent(sock_str).await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let hub = Arc::new(SessionHub::new(sock_str, 10, false).await.unwrap());
        let c1 = hub.subscribe().await.unwrap();
        let c2 = hub.subscribe().await.unwrap();

        assert!(c1.is_owner);
        assert!(c2.is_owner);
        assert!(hub.is_browser_owner(c1.client_id));
        assert!(hub.is_browser_owner(c2.client_id));

        let result = hub.request_resize(c2.client_id, 1440, 900).await;
        assert!(matches!(result, ResizeResult::Applied));

        let snapshot = hub.telemetry_snapshot().await;
        assert_eq!(snapshot.viewer_clients, 0);
        assert!(!snapshot.exclusive_browser_owner);
    }

    #[tokio::test]
    async fn non_owner_resize_is_denied() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("test.sock");
        let sock_str = sock.to_str().unwrap();

        let _agent = mock_agent(sock_str).await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let hub = Arc::new(SessionHub::new(sock_str, 10, true).await.unwrap());

        // Owner sets resolution
        let c1 = hub.subscribe().await.unwrap();
        let result = hub.request_resize(c1.client_id, 1920, 1080).await;
        assert!(matches!(result, ResizeResult::Applied));

        // Wait for ResolutionAck to propagate
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Non-owner resize is denied with current resolution
        let c2 = hub.subscribe().await.unwrap();
        let result = hub.request_resize(c2.client_id, 800, 600).await;
        match result {
            ResizeResult::Locked(w, h) => {
                assert_eq!(w, 1920);
                assert_eq!(h, 1080);
            }
            _ => panic!("expected Locked"),
        }
    }

    #[tokio::test]
    async fn broadcast_reaches_all_subscribers() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("test.sock");
        let sock_str = sock.to_str().unwrap();

        let _agent = mock_agent(sock_str).await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let hub = SessionHub::new(sock_str, 10, false).await.unwrap();
        let mut c1 = hub.subscribe().await.unwrap();
        let mut c2 = hub.subscribe().await.unwrap();

        // Both should receive the SessionReady that the mock agent sent
        let timeout = std::time::Duration::from_secs(2);
        let f1 = tokio::time::timeout(timeout, c1.from_host.recv()).await;
        let f2 = tokio::time::timeout(timeout, c2.from_host.recv()).await;

        assert!(f1.is_ok());
        assert!(f2.is_ok());
    }

    #[tokio::test]
    async fn mcp_owner_blocks_browser_resize() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("test.sock");
        let sock_str = sock.to_str().unwrap();

        let _agent = mock_agent(sock_str).await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let hub = Arc::new(SessionHub::new(sock_str, 10, false).await.unwrap());

        // First subscriber would normally be owner
        let c1 = hub.subscribe().await.unwrap();

        // Set MCP as owner
        hub.set_mcp_owner(1920, 1080).await;
        assert!(hub.mcp_is_owner());

        // Even the first subscriber's resize should be denied
        let result = hub.request_resize(c1.client_id, 800, 600).await;
        match result {
            ResizeResult::Locked(w, h) => {
                assert_eq!(w, 1920);
                assert_eq!(h, 1080);
            }
            _ => panic!("expected Locked when MCP is owner"),
        }
    }

    #[tokio::test]
    async fn clear_mcp_owner_restores_normal_behavior() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("test.sock");
        let sock_str = sock.to_str().unwrap();

        let _agent = mock_agent(sock_str).await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let hub = Arc::new(SessionHub::new(sock_str, 10, false).await.unwrap());

        let c1 = hub.subscribe().await.unwrap();

        // Set MCP, then clear
        hub.set_mcp_owner(1920, 1080).await;
        assert!(hub.mcp_is_owner());

        hub.clear_mcp_owner().await;
        assert!(!hub.mcp_is_owner());

        // Owner should now be able to resize again
        let result = hub.request_resize(c1.client_id, 800, 600).await;
        assert!(matches!(result, ResizeResult::Applied));
    }

    #[tokio::test]
    async fn subscriber_under_mcp_is_not_owner() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("test.sock");
        let sock_str = sock.to_str().unwrap();

        let _agent = mock_agent(sock_str).await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let hub = SessionHub::new(sock_str, 10, false).await.unwrap();

        // Set MCP as owner before anyone subscribes
        hub.set_mcp_owner(1280, 720).await;

        // First subscriber should NOT be owner when MCP owns session
        let c1 = hub.subscribe().await.unwrap();
        assert!(!c1.is_owner);

        // Should get locked resolution
        assert_eq!(c1.locked_resolution, Some((1280, 720)));
    }

    #[tokio::test]
    async fn late_joiner_gets_cached_session_ready() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("test.sock");
        let sock_str = sock.to_str().unwrap();

        let _agent = mock_agent(sock_str).await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let hub = SessionHub::new(sock_str, 10, false).await.unwrap();

        // First subscriber triggers caching
        let _c1 = hub.subscribe().await.unwrap();

        // Wait for SessionReady to be cached
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Late joiner should get initial frames
        let c2 = hub.subscribe().await.unwrap();
        // At minimum, SessionReady should be in initial_frames
        assert!(
            !c2.initial_frames.is_empty(),
            "late joiner should get cached initial frames"
        );

        // Verify the first cached frame is on Control channel (SessionReady)
        let first = &c2.initial_frames[0];
        assert_eq!(first.channel, ChannelId::Control);
    }

    #[tokio::test]
    async fn hub_reports_active_while_agent_connected() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("test.sock");
        let sock_str = sock.to_str().unwrap();

        let _agent = mock_agent(sock_str).await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let hub = SessionHub::new(sock_str, 10, false).await.unwrap();
        assert!(hub.is_active());
    }

    #[tokio::test]
    async fn current_resolution_updates_after_resize() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("test.sock");
        let sock_str = sock.to_str().unwrap();

        let _agent = mock_agent(sock_str).await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let hub = Arc::new(SessionHub::new(sock_str, 10, false).await.unwrap());

        // Initial resolution is (0, 0)
        assert_eq!(hub.current_resolution().await, (0, 0));

        let c1 = hub.subscribe().await.unwrap();
        let result = hub.request_resize(c1.client_id, 1280, 720).await;
        assert!(matches!(result, ResizeResult::Applied));

        // Wait for ack to propagate from mock agent
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let (w, h) = hub.current_resolution().await;
        assert_eq!((w, h), (1280, 720));
    }

    #[tokio::test]
    async fn client_count_tracks_subscribe_unsubscribe() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("test.sock");
        let sock_str = sock.to_str().unwrap();

        let _agent = mock_agent(sock_str).await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let hub = SessionHub::new(sock_str, 10, false).await.unwrap();
        assert_eq!(hub.client_count(), 0);

        let c1 = hub.subscribe().await.unwrap();
        assert_eq!(hub.client_count(), 1);

        let c2 = hub.subscribe().await.unwrap();
        assert_eq!(hub.client_count(), 2);

        hub.unsubscribe(c1.client_id).await;
        assert_eq!(hub.client_count(), 1);

        hub.unsubscribe(c2.client_id).await;
        assert_eq!(hub.client_count(), 0);
    }

    #[tokio::test]
    async fn owner_disconnect_promotes_existing_viewer() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("test.sock");
        let sock_str = sock.to_str().unwrap();

        let _agent = mock_agent(sock_str).await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let hub = Arc::new(SessionHub::new(sock_str, 10, true).await.unwrap());
        let owner = hub.subscribe().await.unwrap();
        let viewer = hub.subscribe().await.unwrap();

        assert!(owner.is_owner);
        assert!(!viewer.is_owner);
        assert!(!hub.is_browser_owner(viewer.client_id));

        hub.unsubscribe(owner.client_id).await;

        assert_eq!(hub.client_count(), 1);
        assert!(hub.is_browser_owner(viewer.client_id));
        let result = hub.request_resize(viewer.client_id, 1024, 768).await;
        assert!(matches!(result, ResizeResult::Applied));
    }

    #[tokio::test]
    async fn viewer_cap_blocks_extra_viewers() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("test.sock");
        let sock_str = sock.to_str().unwrap();

        let _agent = mock_agent(sock_str).await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let hub = SessionHub::new(sock_str, 1, true).await.unwrap();
        let owner = hub.subscribe().await.unwrap();
        assert!(owner.is_owner);

        let viewer = hub.subscribe().await.unwrap();
        assert!(!viewer.is_owner);

        let err = hub.subscribe().await.unwrap_err();
        assert_eq!(err, SubscribeError::ViewerLimitReached { max_viewers: 1 });
        assert_eq!(hub.client_count(), 2);

        let snapshot = hub.telemetry_snapshot().await;
        assert_eq!(snapshot.viewer_clients, 1);
        assert_eq!(snapshot.joins_rejected_viewer_cap, 1);
        assert_eq!(snapshot.viewer_slots_remaining, 0);
    }

    #[tokio::test]
    async fn telemetry_tracks_join_latency_and_refresh_bursts() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("test.sock");
        let sock_str = sock.to_str().unwrap();

        let _agent = mock_agent(sock_str).await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let hub = SessionHub::new(sock_str, 10, true).await.unwrap();

        let _owner = hub.subscribe().await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let grid = TileMessage::GridConfig {
            tile_size: 64,
            cols: 3,
            rows: 2,
            screen_w: 192,
            screen_h: 128,
        }
        .to_frame();
        *hub.cached_grid_config.lock().await = Some(Arc::new(grid));

        let _viewer = hub.subscribe().await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let snapshot = hub.telemetry_snapshot().await;
        assert_eq!(snapshot.browser_clients, 2);
        assert_eq!(snapshot.viewer_clients, 1);
        assert_eq!(snapshot.joins_accepted, 2);
        assert!(snapshot.max_join_latency_ms >= snapshot.last_join_latency_ms);
        assert_eq!(snapshot.full_refresh_requests, 1);
        assert_eq!(snapshot.full_refresh_tiles_requested, 6);
        assert_eq!(snapshot.last_full_refresh_tiles, 6);
        assert_eq!(snapshot.max_full_refresh_tiles, 6);
    }
}
