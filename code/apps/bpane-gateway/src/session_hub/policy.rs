use std::sync::atomic::Ordering;

use bpane_protocol::{ClientAccessFlags, ControlMessage};
use tracing::{info, warn};

use super::{BrowserClientRole, ResizeResult, SessionHub};

fn client_role(hub: &SessionHub, client_id: u64) -> BrowserClientRole {
    hub.client_roles
        .read()
        .unwrap()
        .get(&client_id)
        .copied()
        .unwrap_or(BrowserClientRole::Interactive)
}

pub(super) fn is_browser_owner(hub: &SessionHub, client_id: u64) -> bool {
    if client_role(hub, client_id) == BrowserClientRole::Recorder {
        return false;
    }
    if hub.mcp_is_owner() {
        return false;
    }
    if !hub.exclusive_browser_owner {
        return true;
    }
    let owner_id = hub.owner_id.load(Ordering::Relaxed);
    owner_id == 0 || owner_id == client_id
}

pub(super) fn is_resize_owner(hub: &SessionHub, client_id: u64) -> bool {
    if client_role(hub, client_id) == BrowserClientRole::Recorder {
        return false;
    }
    if hub.mcp_is_owner() {
        return false;
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
        notify_client_access_states(hub, &[client_id], None).await;
        if let Some((w, h)) = *hub.mcp_resolution.lock().await {
            return ResizeResult::Locked(w, h);
        }
    }

    if is_resize_owner(hub, client_id) {
        let result = forward_resize_request(
            &hub.to_agent,
            width,
            height,
            "failed to forward resize to agent",
        )
        .await;
        if matches!(result, ResizeResult::Applied) {
            let client_ids = hub.connected_clients.lock().await.clone();
            notify_client_access_states(hub, &client_ids, Some((width, height))).await;
        }
        return result;
    }

    notify_client_access_states(hub, &[client_id], None).await;
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

    let client_ids = hub.connected_clients.lock().await.clone();
    notify_client_access_states(hub, &client_ids, Some((width, height))).await;

    info!(width, height, "MCP agent registered as session owner");
}

pub(super) async fn clear_mcp_owner(hub: &SessionHub) {
    hub.mcp_is_owner.store(false, Ordering::Relaxed);
    *hub.mcp_resolution.lock().await = None;
    let client_ids = hub.connected_clients.lock().await.clone();
    if hub.owner_id.load(Ordering::Relaxed) == 0 {
        if let Some(next_owner) = client_ids.first().copied() {
            hub.owner_id.store(next_owner, Ordering::Relaxed);
        }
    }
    notify_client_access_states(hub, &client_ids, None).await;
    info!("MCP agent ownership cleared");
}

pub(super) fn viewer_count(hub: &SessionHub) -> u32 {
    let roles = hub.client_roles.read().unwrap();
    roles
        .iter()
        .filter(|(_, role)| **role == BrowserClientRole::Interactive)
        .filter(|(client_id, _)| !is_browser_owner(hub, **client_id))
        .count() as u32
}

pub(super) fn recorder_count(hub: &SessionHub) -> u32 {
    hub.client_roles
        .read()
        .unwrap()
        .values()
        .filter(|role| **role == BrowserClientRole::Recorder)
        .count() as u32
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

pub(super) async fn initial_access_state(
    hub: &SessionHub,
    client_id: u64,
) -> Option<ControlMessage> {
    let state = client_access_state(hub, client_id, None).await;
    match state {
        ControlMessage::ClientAccessState { flags, .. } if flags.is_empty() => None,
        _ => Some(state),
    }
}

pub(super) async fn notify_client_access_states(
    hub: &SessionHub,
    client_ids: &[u64],
    locked_resolution_override: Option<(u16, u16)>,
) {
    let senders = {
        let senders = hub.client_control_txs.lock().await;
        client_ids
            .iter()
            .filter_map(|client_id| {
                senders
                    .get(client_id)
                    .cloned()
                    .map(|sender| (*client_id, sender))
            })
            .collect::<Vec<_>>()
    };

    for (client_id, sender) in senders {
        if is_browser_owner(hub, client_id) {
            if let Some(ready) = cached_session_ready_message(hub).await {
                let _ = sender.send(ready).await;
            }
        }

        let state = client_access_state(hub, client_id, locked_resolution_override).await;
        let _ = sender.send(state).await;
    }
}

async fn client_access_state(
    hub: &SessionHub,
    client_id: u64,
    locked_resolution_override: Option<(u16, u16)>,
) -> ControlMessage {
    let mut flags = ClientAccessFlags::empty();
    if !is_browser_owner(hub, client_id) {
        flags |= ClientAccessFlags::VIEW_ONLY;
    }

    let locked_resolution = if is_resize_owner(hub, client_id) {
        None
    } else {
        resolve_locked_resolution(hub, locked_resolution_override).await
    };

    if locked_resolution.is_some() {
        flags |= ClientAccessFlags::RESIZE_LOCKED;
    }

    let (width, height) = locked_resolution.unwrap_or((0, 0));
    ControlMessage::ClientAccessState {
        flags,
        width,
        height,
    }
}

async fn resolve_locked_resolution(
    hub: &SessionHub,
    locked_resolution_override: Option<(u16, u16)>,
) -> Option<(u16, u16)> {
    if hub.mcp_is_owner() {
        return *hub.mcp_resolution.lock().await;
    }

    if let Some((width, height)) = locked_resolution_override {
        return Some((width, height));
    }

    let (width, height) = *hub.current_resolution.lock().await;
    if width > 0 && height > 0 {
        Some((width, height))
    } else {
        None
    }
}

async fn cached_session_ready_message(hub: &SessionHub) -> Option<ControlMessage> {
    let frame = hub.cached_session_ready.lock().await.clone()?;
    ControlMessage::decode(&frame.payload).ok()
}
