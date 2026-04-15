use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::{broadcast, mpsc, Mutex};
use tracing::{debug, info, warn};

use bpane_protocol::channel::ChannelId;
use bpane_protocol::frame::Frame;
use bpane_protocol::ControlMessage;

use crate::relay::Relay;

mod refresh;
mod telemetry;

pub use self::telemetry::SessionTelemetrySnapshot;

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
        refresh::request_full_refresh(&self.cached_grid_config, &self.to_agent).await
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
        telemetry::snapshot(self, resolution)
    }

    fn record_join_latency(&self, elapsed: std::time::Duration) {
        telemetry::record_join_latency(self, elapsed);
    }

    fn record_refresh_burst(&self, tiles_requested: u64) {
        telemetry::record_refresh_burst(self, tiles_requested);
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
mod tests;
