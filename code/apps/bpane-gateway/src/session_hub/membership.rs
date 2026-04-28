use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;

use bpane_protocol::frame::Frame;
use bpane_protocol::ControlMessage;
use tokio::sync::mpsc;
use tracing::debug;

use super::{BrowserClientRole, ClientHandle, SessionHub, SubscribeError};

pub(super) async fn subscribe(
    hub: &SessionHub,
    client_role: BrowserClientRole,
) -> Result<ClientHandle, SubscribeError> {
    let join_started = Instant::now();
    let client_id = hub.client_counter.fetch_add(1, Ordering::Relaxed) + 1;
    let mcp_is_owner = hub.mcp_is_owner.load(Ordering::Relaxed);
    let mut connected_clients = hub.connected_clients.lock().await;
    let prev_count = connected_clients.len() as u32;
    let current_owner_id = hub.owner_id.load(Ordering::Relaxed);
    let exclusive_browser_owner = hub.exclusive_browser_owner;
    let is_resize_owner =
        client_role == BrowserClientRole::Interactive && !mcp_is_owner && current_owner_id == 0;
    let (control_tx, control_rx) = mpsc::channel::<ControlMessage>(8);

    let is_owner = if client_role == BrowserClientRole::Recorder || mcp_is_owner {
        false
    } else if !exclusive_browser_owner {
        true
    } else {
        current_owner_id == 0
    };

    if client_role == BrowserClientRole::Interactive && !is_owner {
        let owner_connected = !mcp_is_owner && current_owner_id != 0 && prev_count > 0;
        let current_viewers = {
            let roles = hub.client_roles.read().unwrap();
            connected_clients
                .iter()
                .filter(|id| roles.get(id) == Some(&BrowserClientRole::Interactive))
                .count() as u32
        };
        let current_viewers = if owner_connected {
            current_viewers.saturating_sub(1)
        } else {
            current_viewers
        };
        if current_viewers >= hub.max_viewers {
            hub.joins_rejected_viewer_cap
                .fetch_add(1, Ordering::Relaxed);
            return Err(SubscribeError::ViewerLimitReached {
                max_viewers: hub.max_viewers,
            });
        }
    }

    connected_clients.push(client_id);
    if is_resize_owner {
        hub.owner_id.store(client_id, Ordering::Relaxed);
    }
    hub.client_count
        .store(connected_clients.len() as u32, Ordering::Relaxed);
    drop(connected_clients);

    hub.client_roles
        .write()
        .unwrap()
        .insert(client_id, client_role);

    hub.client_control_txs
        .lock()
        .await
        .insert(client_id, control_tx);

    let initial_frames = gather_initial_frames(hub).await;
    let initial_access_state = super::policy::initial_access_state(hub, client_id).await;

    if prev_count > 0 {
        let tiles_requested = hub.request_full_refresh().await;
        if tiles_requested > 0 {
            hub.record_refresh_burst(tiles_requested);
        }
    }

    hub.record_join_latency(join_started.elapsed());
    hub.joins_accepted.fetch_add(1, Ordering::Relaxed);

    Ok(ClientHandle {
        from_host: hub.broadcast_tx.subscribe(),
        to_host: hub.to_agent.clone(),
        client_id,
        is_owner,
        client_role,
        initial_frames,
        initial_access_state,
        control_rx,
    })
}

pub(super) async fn unsubscribe(hub: &SessionHub, client_id: u64) {
    let mut connected_clients = hub.connected_clients.lock().await;
    connected_clients.retain(|id| *id != client_id);
    hub.client_count
        .store(connected_clients.len() as u32, Ordering::Relaxed);

    let remaining_clients = connected_clients.clone();
    let next_owner = if hub.mcp_is_owner.load(Ordering::Relaxed) {
        0
    } else {
        let roles = hub.client_roles.read().unwrap();
        connected_clients
            .iter()
            .copied()
            .find(|id| roles.get(id) == Some(&BrowserClientRole::Interactive))
            .unwrap_or(0)
    };
    if hub.owner_id.load(Ordering::Relaxed) == client_id {
        hub.owner_id.store(next_owner, Ordering::Relaxed);
        if next_owner != 0 {
            debug!(
                client_id,
                next_owner, "promoted existing viewer to session owner"
            );
        }
    } else if hub.owner_id.load(Ordering::Relaxed) == 0
        && !hub.mcp_is_owner.load(Ordering::Relaxed)
        && next_owner != 0
    {
        hub.owner_id.store(next_owner, Ordering::Relaxed);
        debug!(client_id, next_owner, "restored missing session owner");
    }
    drop(connected_clients);

    hub.client_roles.write().unwrap().remove(&client_id);
    hub.client_control_txs.lock().await.remove(&client_id);
    super::policy::notify_client_access_states(hub, &remaining_clients, None).await;
}

async fn gather_initial_frames(hub: &SessionHub) -> Vec<Arc<Frame>> {
    let mut initial_frames = Vec::new();
    if let Some(sr) = hub.cached_session_ready.lock().await.as_ref() {
        initial_frames.push(sr.clone());
    }
    if let Some(gc) = hub.cached_grid_config.lock().await.as_ref() {
        initial_frames.push(gc.clone());
    }
    if let Some(kf) = hub.cached_keyframe.lock().await.as_ref() {
        initial_frames.push(kf.clone());
    }
    initial_frames
}
