use std::sync::Arc;
use std::time::Duration;

use tracing::{info, warn};
use uuid::Uuid;

use crate::recording_lifecycle::RecordingLifecycleManager;
use crate::session_control::{
    SessionLifecycleState, SessionRecordingTerminationReason, SessionStore,
};
use crate::session_manager::SessionManager;
use crate::session_registry::SessionRegistry;

pub fn schedule_idle_session_stop(
    session_id: Uuid,
    default_timeout: Duration,
    registry: Arc<SessionRegistry>,
    session_store: SessionStore,
    session_manager: Arc<SessionManager>,
    recording_lifecycle: Arc<RecordingLifecycleManager>,
) {
    tokio::spawn(async move {
        let timeout = match session_store.get_session_by_id(session_id).await {
            Ok(Some(session)) => session
                .idle_timeout_sec
                .map(|seconds| Duration::from_secs(u64::from(seconds)))
                .unwrap_or(default_timeout),
            Ok(None) => default_timeout,
            Err(error) => {
                warn!(%session_id, "failed to load session idle timeout: {error}");
                default_timeout
            }
        };

        tokio::time::sleep(timeout).await;

        if let Some(snapshot) = registry.telemetry_snapshot_if_live(session_id).await {
            if snapshot.browser_clients > 0 || snapshot.viewer_clients > 0 || snapshot.mcp_owner {
                return;
            }
        }

        if let Err(error) = recording_lifecycle
            .request_stop_and_wait(session_id, SessionRecordingTerminationReason::IdleStop)
            .await
        {
            warn!(%session_id, "failed to finalize recording before idle stop: {error}");
        }

        match session_store.stop_session_if_idle(session_id).await {
            Ok(Some(session)) if session.state == SessionLifecycleState::Stopped => {
                session_manager.release(session_id).await;
                registry.remove_session(session_id).await;
                info!(%session_id, "stopped idle session after {:?}", timeout);
            }
            Ok(_) => {}
            Err(error) => {
                warn!(%session_id, "failed to stop idle session: {error}");
            }
        }
    });
}
