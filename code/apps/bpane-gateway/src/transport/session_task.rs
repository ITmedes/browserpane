use std::sync::Arc;
use std::time::Duration;

use tracing::debug;

use super::bitrate::DatagramStats;
use super::bootstrap::send_initial_frames;
use super::egress::{spawn_agent_to_browser_task, EgressTaskContext};
use super::ingress::spawn_browser_to_agent_task;
use super::request::ValidatedConnectRequest;
use super::tasks::{
    recorder_role_suppresses_bitrate_feedback, spawn_bitrate_hint_task, spawn_direct_control_task,
    spawn_gateway_pinger,
};
use crate::idle_stop::schedule_idle_session_stop;
use crate::recording_lifecycle::RecordingLifecycleManager;
use crate::session_control::SessionRecordingTerminationReason;
use crate::session_control::SessionStore;
use crate::session_files::{SessionFileRecorder, SessionFileSource};
use crate::session_hub::{BrowserClientRole, SessionTerminationReason};
use crate::session_manager::SessionManager;
use crate::session_registry::SessionRegistry;
use crate::workspaces::WorkspaceFileStore;

use super::session::Session;

pub(super) struct SessionTaskContext {
    pub connection: wtransport::Connection,
    pub session_id: u64,
    pub connect_request: ValidatedConnectRequest,
    pub session_manager: Arc<SessionManager>,
    pub session_store: SessionStore,
    pub workspace_file_store: Arc<WorkspaceFileStore>,
    pub idle_stop_timeout: Duration,
    pub agent_socket_path: String,
    pub heartbeat_timeout: Duration,
    pub registry: Arc<SessionRegistry>,
    pub recording_lifecycle: Arc<RecordingLifecycleManager>,
}

pub(super) async fn handle_session(context: SessionTaskContext) -> anyhow::Result<()> {
    let SessionTaskContext {
        connection,
        session_id,
        connect_request,
        session_manager,
        session_store,
        workspace_file_store,
        idle_stop_timeout,
        agent_socket_path,
        heartbeat_timeout,
        registry,
        recording_lifecycle,
    } = context;
    let routed_session_id = connect_request.session_id;
    let transport_policy = connect_request.transport_policy;
    let (client_handle, hub) = registry
        .join_with_role(
            routed_session_id,
            &agent_socket_path,
            connect_request.client_role,
            transport_policy.allow_browser_downloads(),
        )
        .await?;
    let client_id = client_handle.client_id;
    let joined_as_owner = client_handle.is_owner;
    let client_role = client_handle.client_role;
    let initial_access_state = client_handle.initial_access_state;
    let control_rx = client_handle.control_rx;
    let termination_rx = client_handle.termination_rx;
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
        transport_policy.clone(),
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
            transport_policy: transport_policy.clone(),
        },
        from_host,
    );

    let browser_to_agent = spawn_browser_to_agent_task(
        session.clone(),
        hub.clone(),
        client_id,
        recv_stream,
        to_host,
        SessionFileRecorder::new(
            routed_session_id,
            SessionFileSource::BrowserUpload,
            session_store.clone(),
            workspace_file_store,
        ),
        transport_policy.clone(),
    );

    let direct_control_task = spawn_direct_control_task(
        session.clone(),
        send_stream.clone(),
        control_rx,
        transport_policy,
    );

    let gateway_pinger = spawn_gateway_pinger(session.clone(), send_stream.clone());
    let mut close_reason: &[u8] = b"session ended";
    let mut should_transition_to_idle = true;
    let mut recording_termination_reason =
        Some(SessionRecordingTerminationReason::ClientDisconnect);
    let mut termination_rx = termination_rx;

    if recorder_role_suppresses_bitrate_feedback(client_role) {
        tokio::select! {
            _ = agent_to_browser => {}
            _ = browser_to_agent => {}
            _ = direct_control_task => {}
            _ = gateway_pinger => {}
            reason = &mut termination_rx => {
                if let Ok(reason) = reason {
                    close_reason = reason.close_reason_bytes();
                    should_transition_to_idle = reason.transitions_to_idle();
                    recording_termination_reason = recording_reason_for_termination(reason);
                    session.deactivate();
                }
            }
        }
    } else {
        let bitrate_hint_task = spawn_bitrate_hint_task(
            session_id,
            client_id,
            session.clone(),
            dgram_stats.clone(),
            send_stream.clone(),
        );
        tokio::select! {
            _ = agent_to_browser => {}
            _ = browser_to_agent => {}
            _ = direct_control_task => {}
            _ = gateway_pinger => {}
            _ = bitrate_hint_task => {}
            reason = &mut termination_rx => {
                if let Ok(reason) = reason {
                    close_reason = reason.close_reason_bytes();
                    should_transition_to_idle = reason.transitions_to_idle();
                    recording_termination_reason = recording_reason_for_termination(reason);
                    session.deactivate();
                }
            }
        }
    }

    session.deactivate();
    registry.leave(routed_session_id, client_id).await;
    if should_transition_to_idle {
        if let Some(snapshot) = registry.telemetry_snapshot_if_live(routed_session_id).await {
            if !snapshot.has_interactive_session_activity() {
                if client_role == BrowserClientRole::Interactive {
                    if let Some(reason) = recording_termination_reason {
                        let _ = recording_lifecycle
                            .request_stop_and_wait(routed_session_id, reason)
                            .await;
                    }
                }
                let _ = session_store.mark_session_idle(routed_session_id).await;
                session_manager.mark_session_idle(routed_session_id).await;
                schedule_idle_session_stop(
                    routed_session_id,
                    idle_stop_timeout,
                    registry.clone(),
                    session_store.clone(),
                    session_manager.clone(),
                    recording_lifecycle.clone(),
                );
            }
        } else {
            if client_role == BrowserClientRole::Interactive {
                if let Some(reason) = recording_termination_reason {
                    let _ = recording_lifecycle
                        .request_stop_and_wait(routed_session_id, reason)
                        .await;
                }
            }
            let _ = session_store.mark_session_idle(routed_session_id).await;
            session_manager.mark_session_idle(routed_session_id).await;
            schedule_idle_session_stop(
                routed_session_id,
                idle_stop_timeout,
                registry.clone(),
                session_store,
                session_manager.clone(),
                recording_lifecycle,
            );
        }
    }

    connection.close(wtransport::VarInt::from_u32(0), close_reason);

    Ok(())
}

fn recording_reason_for_termination(
    reason: SessionTerminationReason,
) -> Option<SessionRecordingTerminationReason> {
    match reason {
        SessionTerminationReason::DisconnectedByOwner => {
            Some(SessionRecordingTerminationReason::ClientDisconnect)
        }
        SessionTerminationReason::DisconnectedAllByOwner => {
            Some(SessionRecordingTerminationReason::DisconnectAll)
        }
        SessionTerminationReason::SessionKilled => None,
    }
}
