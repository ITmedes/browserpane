use std::sync::Arc;

use bpane_protocol::frame::Frame;
use tokio::sync::{broadcast, mpsc};

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

/// Result of a resize request.
pub enum ResizeResult {
    /// Request was forwarded to the host agent.
    Applied,
    /// Resolution is locked by the owner — use these dimensions.
    Locked(u16, u16),
}
