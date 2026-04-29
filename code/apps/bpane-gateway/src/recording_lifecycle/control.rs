use tokio::time::{sleep, Instant};
use tracing::{info, warn};

use super::*;

impl RecordingLifecycleInner {
    pub(super) async fn reconcile_assignment(
        self: &Arc<Self>,
        assignment: PersistedSessionRecordingWorkerAssignment,
    ) -> Result<(), RecordingLifecycleError> {
        info!(
            session_id = %assignment.session_id,
            recording_id = %assignment.recording_id,
            "reconciling persisted recorder worker assignment after gateway restart"
        );

        let stale_recording = self
            .session_store
            .get_recording_for_session(assignment.session_id, assignment.recording_id)
            .await?;
        if let Some(recording) = &stale_recording {
            if recording.state.is_active() {
                let _ = self
                    .session_store
                    .fail_recording_for_session(
                        assignment.session_id,
                        assignment.recording_id,
                        FailSessionRecordingRequest {
                            error: "gateway restarted while recorder worker was active".to_string(),
                            termination_reason: Some(
                                SessionRecordingTerminationReason::GatewayRestart,
                            ),
                        },
                    )
                    .await?;
            }
        }

        self.session_store
            .clear_recording_worker_assignment(assignment.session_id)
            .await?;

        let Some(session) = self
            .session_store
            .get_session_by_id(assignment.session_id)
            .await?
        else {
            return Ok(());
        };
        if session.recording.mode != SessionRecordingMode::Always
            || !session.state.is_runtime_candidate()
        {
            return Ok(());
        }

        let recording = self
            .session_store
            .create_recording_for_session(
                session.id,
                session.recording.format,
                stale_recording.as_ref().map(|recording| recording.id),
            )
            .await?;
        self.spawn_worker(session.id, recording.id).await
    }

    pub(super) async fn request_stop_and_wait(
        &self,
        session_id: Uuid,
        termination_reason: SessionRecordingTerminationReason,
    ) -> Result<(), RecordingLifecycleError> {
        let Some(recording) = self
            .session_store
            .get_latest_recording_for_session(session_id)
            .await?
        else {
            let _ = self
                .session_store
                .clear_recording_worker_assignment(session_id)
                .await;
            return Ok(());
        };

        if let Some(mut assignment) = self
            .session_store
            .get_recording_worker_assignment(session_id)
            .await?
        {
            assignment.status = SessionRecordingWorkerAssignmentStatus::Stopping;
            let _ = self
                .session_store
                .upsert_recording_worker_assignment(assignment)
                .await;
        }

        if recording.state.is_active() {
            self.session_store
                .stop_recording_for_session(session_id, recording.id, termination_reason)
                .await?;
        } else if recording.state.is_terminal() {
            let _ = self
                .session_store
                .clear_recording_worker_assignment(session_id)
                .await;
            return Ok(());
        }

        let deadline = Instant::now() + self.config.finalize_timeout;
        loop {
            let Some(current) = self
                .session_store
                .get_recording_for_session(session_id, recording.id)
                .await?
            else {
                let _ = self
                    .session_store
                    .clear_recording_worker_assignment(session_id)
                    .await;
                return Ok(());
            };
            if current.state.is_terminal() {
                let _ = self
                    .session_store
                    .clear_recording_worker_assignment(session_id)
                    .await;
                return Ok(());
            }
            if Instant::now() >= deadline {
                warn!(
                    session_id = %session_id,
                    recording_id = %recording.id,
                    "timed out waiting for recording finalization during session teardown"
                );
                return Ok(());
            }
            sleep(self.config.poll_interval).await;
        }
    }
}
