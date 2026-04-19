use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::{collections::HashMap};
use tokio::sync::{broadcast, mpsc, Mutex};

use bpane_protocol::frame::Frame;
use bpane_protocol::ControlMessage;

use crate::relay::Relay;

mod membership;
mod policy;
mod pump;
mod refresh;
mod telemetry;
mod types;

pub use self::telemetry::SessionTelemetrySnapshot;
pub use self::types::{ClientHandle, ResizeResult, SubscribeError};

/// Broadcast channel capacity. At 30fps, 1024 frames is ~34 seconds of buffer.
const BROADCAST_CAPACITY: usize = 1024;

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
    client_control_txs: Mutex<HashMap<u64, mpsc::Sender<ControlMessage>>>,
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

        let pump_handle = pump::spawn(
            from_agent_rx,
            broadcast_tx.clone(),
            pump::PumpState {
                active: active.clone(),
                cached_grid_config: cached_grid_config.clone(),
                cached_keyframe: cached_keyframe.clone(),
                cached_session_ready: cached_session_ready.clone(),
                current_resolution: current_resolution.clone(),
            },
        );

        Ok(Self {
            broadcast_tx,
            to_agent: to_agent_tx,
            max_viewers,
            exclusive_browser_owner,
            current_resolution,
            connected_clients: Mutex::new(Vec::new()),
            client_control_txs: Mutex::new(HashMap::new()),
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

    /// Subscribe a new client to the hub.
    /// The first subscriber becomes the session owner.
    pub async fn subscribe(&self) -> Result<ClientHandle, SubscribeError> {
        membership::subscribe(self).await
    }

    /// Called when a client disconnects.
    pub async fn unsubscribe(&self, client_id: u64) {
        membership::unsubscribe(self, client_id).await;
    }

    pub fn is_browser_owner(&self, client_id: u64) -> bool {
        policy::is_browser_owner(self, client_id)
    }

    /// Handle a resize request from a client.
    /// Only the owner's requests are forwarded to the host.
    /// Non-owner requests are denied and the current resolution is returned.
    pub async fn request_resize(&self, client_id: u64, width: u16, height: u16) -> ResizeResult {
        policy::request_resize(self, client_id, width, height).await
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
        policy::set_mcp_owner(self, width, height).await;
    }

    /// Remove MCP agent ownership. Next browser client to subscribe becomes owner.
    pub async fn clear_mcp_owner(&self) {
        policy::clear_mcp_owner(self).await;
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
        policy::viewer_count(self)
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

#[cfg(test)]
mod tests;
