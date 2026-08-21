use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use tracing::{debug, warn};
use uuid::Uuid;

use super::bitrate::DatagramStats;
use super::bootstrap::{send_initial_frames, InitialFramesContext};
use super::egress::{spawn_agent_to_browser_task, EgressTaskContext};
use super::ingress::{spawn_browser_to_agent_task, IngressTaskContext};
use super::negotiation::{
    negotiate_connection, reject_active_connection, ProtocolNegotiationConfig,
};
use super::request::ValidatedConnectRequest;
use super::tasks::{
    recorder_role_suppresses_bitrate_feedback, spawn_bitrate_hint_task, spawn_direct_control_task,
    spawn_gateway_pinger,
};
use crate::idle_stop::schedule_idle_session_stop;
use crate::metrics::GatewayMetrics;
use crate::recording_lifecycle::RecordingLifecycleManager;
use crate::session_control::SessionRecordingTerminationReason;
use crate::session_control::SessionStore;
use crate::session_files::{SessionFileRecorder, SessionFileSource};
use crate::session_hub::{BrowserClientRole, SessionTerminationReason};
use crate::session_manager::{SessionManager, SessionManagerError, SessionRuntime};
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
    pub heartbeat_timeout: Duration,
    pub protocol_handshake_timeout: Duration,
    pub protocol_legacy_compatibility: bool,
    pub runtime_startup_capacity_wait: Duration,
    pub registry: Arc<SessionRegistry>,
    pub recording_lifecycle: Arc<RecordingLifecycleManager>,
    pub metrics: Arc<GatewayMetrics>,
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
        heartbeat_timeout,
        protocol_handshake_timeout,
        protocol_legacy_compatibility,
        runtime_startup_capacity_wait,
        registry,
        recording_lifecycle,
        metrics,
    } = context;
    let routed_session_id = connect_request.session_id;
    let transport_policy = connect_request.transport_policy;
    let negotiation_config = ProtocolNegotiationConfig {
        timeout: protocol_handshake_timeout,
        legacy_compatibility: protocol_legacy_compatibility,
    };
    let Some(negotiated) = negotiate_connection(&connection, &negotiation_config, &metrics).await?
    else {
        return Ok(());
    };
    let Some(runtime) = resolve_runtime_after_negotiation(
        &connection,
        &session_manager,
        routed_session_id,
        runtime_startup_capacity_wait,
    )
    .await?
    else {
        return Ok(());
    };
    session_manager.mark_session_active(routed_session_id).await;
    if let Err(error) = session_store.mark_session_active(routed_session_id).await {
        warn!("failed to mark negotiated session active in store: {error}");
    }
    let (client_handle, hub) = registry
        .join_with_role(
            routed_session_id,
            &runtime.agent_socket_path,
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

    let send_stream = negotiated.send_stream;
    let recv_stream = negotiated.recv_stream;
    let protocol = negotiated.protocol;

    send_initial_frames(
        &send_stream,
        &initial_frames,
        InitialFramesContext {
            joined_as_owner,
            initial_access_state,
            policy: transport_policy.clone(),
            protocol: protocol.clone(),
            session_id,
            client_id,
        },
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
            protocol: protocol.clone(),
        },
        from_host,
    );

    let browser_to_agent = spawn_browser_to_agent_task(IngressTaskContext {
        session: session.clone(),
        hub: hub.clone(),
        client_id,
        recv_stream,
        initial_frames: negotiated.initial_client_frames,
        to_host,
        file_recorder: SessionFileRecorder::new(
            routed_session_id,
            SessionFileSource::BrowserUpload,
            session_store.clone(),
            workspace_file_store,
        ),
        transport_policy: transport_policy.clone(),
        protocol: protocol.clone(),
    });

    let direct_control_task = spawn_direct_control_task(
        session.clone(),
        send_stream.clone(),
        control_rx,
        transport_policy,
        protocol,
    );

    let gateway_pinger = spawn_gateway_pinger(session.clone(), send_stream.clone());
    let mut close_reason: &[u8] = b"session ended";
    let mut should_transition_to_idle = true;
    let mut recording_termination_reason =
        Some(SessionRecordingTerminationReason::ClientDisconnect);
    let mut protocol_failure = None;
    let mut termination_rx = termination_rx;

    if recorder_role_suppresses_bitrate_feedback(client_role) {
        tokio::select! {
            result = agent_to_browser => {
                protocol_failure = joined_protocol_failure(result);
            }
            result = browser_to_agent => {
                protocol_failure = joined_protocol_failure(result);
            }
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
            result = agent_to_browser => {
                protocol_failure = joined_protocol_failure(result);
            }
            result = browser_to_agent => {
                protocol_failure = joined_protocol_failure(result);
            }
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
    if let Some(failure) = protocol_failure {
        metrics.record_protocol_violation(failure);
        reject_active_connection(&connection, &send_stream, failure).await;
    }
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

    if protocol_failure.is_none() {
        connection.close(wtransport::VarInt::from_u32(0), close_reason);
    }

    Ok(())
}

async fn resolve_runtime_after_negotiation(
    connection: &wtransport::Connection,
    session_manager: &SessionManager,
    session_id: Uuid,
    startup_capacity_wait: Duration,
) -> anyhow::Result<Option<SessionRuntime>> {
    const RETRY_INTERVAL: Duration = Duration::from_millis(100);

    let deadline = tokio::time::Instant::now() + startup_capacity_wait;
    loop {
        match session_manager.resolve(session_id).await {
            Ok(runtime) => return Ok(Some(runtime)),
            Err(error @ SessionManagerError::RuntimeStartupCapacityReached { .. }) => {
                let now = tokio::time::Instant::now();
                if now >= deadline {
                    return Err(error).context("runtime startup-capacity wait expired");
                }
                tokio::select! {
                    _ = connection.closed() => return Ok(None),
                    _ = tokio::time::sleep_until((now + RETRY_INTERVAL).min(deadline)) => {}
                }
            }
            Err(error) => return Err(error.into()),
        }
    }
}

fn joined_protocol_failure(
    result: Result<Option<bpane_protocol::frame::ProtocolFailure>, tokio::task::JoinError>,
) -> Option<bpane_protocol::frame::ProtocolFailure> {
    result.ok().flatten()
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
