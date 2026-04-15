use std::sync::atomic::Ordering;
use std::sync::Arc;

use bpane_protocol::ControlMessage;
use tracing::{info, warn};

use super::{ResizeResult, SessionHub};

pub(super) fn is_browser_owner(hub: &SessionHub, client_id: u64) -> bool {
    if hub.mcp_is_owner() {
        return false;
    }
    if !hub.exclusive_browser_owner {
        return true;
    }
    let owner_id = hub.owner_id.load(Ordering::Relaxed);
    owner_id == 0 || owner_id == client_id
}

pub(super) async fn request_resize(
    hub: &SessionHub,
    client_id: u64,
    width: u16,
    height: u16,
) -> ResizeResult {
    if hub.mcp_is_owner.load(Ordering::Relaxed) {
        if let Some((w, h)) = *hub.mcp_resolution.lock().await {
            return ResizeResult::Locked(w, h);
        }
    }

    if !hub.exclusive_browser_owner {
        return forward_resize_request(
            &hub.to_agent,
            width,
            height,
            "failed to forward collaborative resize to agent",
        )
        .await;
    }

    let owner = hub.owner_id.load(Ordering::Relaxed);
    if client_id == owner {
        return forward_resize_request(
            &hub.to_agent,
            width,
            height,
            "failed to forward resize to agent",
        )
        .await;
    }

    let (cur_w, cur_h) = *hub.current_resolution.lock().await;
    if cur_w > 0 && cur_h > 0 {
        ResizeResult::Locked(cur_w, cur_h)
    } else {
        ResizeResult::Locked(0, 0)
    }
}

pub(super) async fn set_mcp_owner(hub: &SessionHub, width: u16, height: u16) {
    *hub.mcp_resolution.lock().await = Some((width, height));
    hub.mcp_is_owner.store(true, Ordering::Relaxed);

    let msg = ControlMessage::ResolutionRequest { width, height };
    if hub.to_agent.send(msg.to_frame()).await.is_err() {
        warn!("failed to send MCP resolution request to agent");
    }

    let locked = ControlMessage::ResolutionLocked { width, height };
    let _ = hub.broadcast_tx.send(Arc::new(locked.to_frame()));

    info!(width, height, "MCP agent registered as session owner");
}

pub(super) async fn clear_mcp_owner(hub: &SessionHub) {
    hub.mcp_is_owner.store(false, Ordering::Relaxed);
    *hub.mcp_resolution.lock().await = None;
    info!("MCP agent ownership cleared");
}

pub(super) fn viewer_count(hub: &SessionHub) -> u32 {
    let clients = hub.client_count();
    if hub.mcp_is_owner() {
        clients
    } else if hub.exclusive_browser_owner
        && hub.owner_id.load(Ordering::Relaxed) != 0
        && clients > 0
    {
        clients.saturating_sub(1)
    } else if hub.exclusive_browser_owner {
        clients
    } else {
        0
    }
}

async fn forward_resize_request(
    to_agent: &tokio::sync::mpsc::Sender<bpane_protocol::frame::Frame>,
    width: u16,
    height: u16,
    error_message: &str,
) -> ResizeResult {
    let msg = ControlMessage::ResolutionRequest { width, height };
    if to_agent.send(msg.to_frame()).await.is_err() {
        warn!("{error_message}");
    }
    ResizeResult::Applied
}
