use std::sync::Arc;
use std::time::Duration;

use tracing::debug;

use super::bitrate::DatagramStats;
use super::bootstrap::send_initial_frames;
use super::egress::{spawn_agent_to_browser_task, EgressTaskContext};
use super::ingress::spawn_browser_to_agent_task;
use super::request::ValidatedConnectRequest;
use super::tasks::{spawn_bitrate_hint_task, spawn_direct_control_task, spawn_gateway_pinger};
use crate::idle_stop::schedule_idle_session_stop;
use crate::session::Session;
use crate::session_control::SessionStore;
use crate::session_manager::SessionManager;
use crate::session_registry::SessionRegistry;

pub(super) async fn handle_session(
    connection: wtransport::Connection,
    session_id: u64,
    connect_request: ValidatedConnectRequest,
    session_manager: Arc<SessionManager>,
    session_store: SessionStore,
    idle_stop_timeout: Duration,
    agent_socket_path: &str,
    heartbeat_timeout: Duration,
    registry: Arc<SessionRegistry>,
) -> anyhow::Result<()> {
    let routed_session_id = connect_request.session_id;
    let (client_handle, hub) = registry.join(routed_session_id, agent_socket_path).await?;
    let client_id = client_handle.client_id;
    let joined_as_owner = client_handle.is_owner;
    let initial_access_state = client_handle.initial_access_state;
    let control_rx = client_handle.control_rx;
    let from_host = client_handle.from_host;
    let to_host = client_handle.to_host;
    let initial_frames = client_handle.initial_frames;

    debug!(
        session_id,
        %routed_session_id,
        client_id,
        is_owner = joined_as_owner,
        "client joined session hub"
    );

    let session = Arc::new(Session::new(session_id, heartbeat_timeout));
    let session_clone = session.clone();
    tokio::spawn(async move {
        session_clone.run_heartbeat_monitor().await;
    });

    let (send_stream, recv_stream) = connection.open_bi().await?.await?;
    let send_stream = Arc::new(tokio::sync::Mutex::new(send_stream));

    send_initial_frames(
        &send_stream,
        &initial_frames,
        joined_as_owner,
        initial_access_state,
        session_id,
        client_id,
    )
    .await?;

    let dgram_stats = Arc::new(DatagramStats::new());
    let agent_to_browser = spawn_agent_to_browser_task(
        EgressTaskContext {
            session: session.clone(),
            hub: hub.clone(),
            session_id,
            client_id,
            send_stream: send_stream.clone(),
            connection: connection.clone(),
            dgram_stats: dgram_stats.clone(),
        },
        from_host,
    );

    let bitrate_hint_task = spawn_bitrate_hint_task(
        session_id,
        client_id,
        session.clone(),
        dgram_stats.clone(),
        send_stream.clone(),
    );

    let browser_to_agent = spawn_browser_to_agent_task(
        session.clone(),
        hub.clone(),
        client_id,
        recv_stream,
        to_host,
    );

    let direct_control_task =
        spawn_direct_control_task(session.clone(), send_stream.clone(), control_rx);

    let gateway_pinger = spawn_gateway_pinger(session.clone(), send_stream.clone());

    tokio::select! {
        _ = agent_to_browser => {}
        _ = browser_to_agent => {}
        _ = direct_control_task => {}
        _ = gateway_pinger => {}
        _ = bitrate_hint_task => {}
    }

    session.deactivate();
    registry.leave(routed_session_id, client_id).await;
    if let Some(snapshot) = registry.telemetry_snapshot_if_live(routed_session_id).await {
        if snapshot.browser_clients == 0 && snapshot.viewer_clients == 0 && !snapshot.mcp_owner {
            let _ = session_store.mark_session_idle(routed_session_id).await;
            session_manager.mark_session_idle(routed_session_id).await;
            schedule_idle_session_stop(
                routed_session_id,
                idle_stop_timeout,
                registry.clone(),
                session_store.clone(),
                session_manager.clone(),
            );
        }
    } else {
        let _ = session_store.mark_session_idle(routed_session_id).await;
        session_manager.mark_session_idle(routed_session_id).await;
        schedule_idle_session_stop(
            routed_session_id,
            idle_stop_timeout,
            registry.clone(),
            session_store,
            session_manager.clone(),
        );
    }

    connection.close(wtransport::VarInt::from_u32(0), b"session ended");

    Ok(())
}
