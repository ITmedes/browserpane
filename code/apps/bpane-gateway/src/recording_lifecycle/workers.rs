use std::process::Stdio;

use tokio::process::Command;
use tracing::{info, warn};

use super::*;

#[derive(Debug, Clone, Copy)]
pub(super) struct LaunchedRecordingWorker {
    pub(super) recording_id: Uuid,
}

impl RecordingLifecycleInner {
    pub(super) async fn spawn_worker(
        self: &Arc<Self>,
        session_id: Uuid,
        recording_id: Uuid,
    ) -> Result<(), RecordingLifecycleError> {
        let mut command = Command::new(&self.config.bin);
        command.args(&self.config.args);
        command.stdin(Stdio::null());
        command.stdout(Stdio::null());
        command.stderr(Stdio::null());
        command.env("BPANE_RECORDING_SESSION_ID", session_id.to_string());
        command.env("BPANE_RECORDING_ID", recording_id.to_string());
        command.env("BPANE_RECORDING_CHROME", &self.config.chrome_executable);
        command.env("BPANE_GATEWAY_API_URL", &self.config.gateway_api_url);
        command.env("BPANE_RECORDING_PAGE_URL", &self.config.page_url);
        command.env("BPANE_RECORDING_OUTPUT_ROOT", &self.config.output_root);
        command.env(
            "BPANE_RECORDING_CONNECT_TIMEOUT_MS",
            self.config.connect_timeout.as_millis().to_string(),
        );
        command.env(
            "BPANE_RECORDING_POLL_INTERVAL_MS",
            self.config.poll_interval.as_millis().to_string(),
        );
        command.env(
            "BPANE_RECORDING_HEADLESS",
            if self.config.headless {
                "true"
            } else {
                "false"
            },
        );

        if let Some(cert_spki) = &self.config.cert_spki {
            command.env("BPANE_RECORDING_CERT_SPKI", cert_spki);
        }
        if let Some(bearer_token) = self.resolve_bearer_token() {
            command.env("BPANE_RECORDING_BEARER_TOKEN", bearer_token);
        }
        if let Some(token_url) = &self.config.oidc_token_url {
            command.env("BPANE_GATEWAY_OIDC_TOKEN_URL", token_url);
        }
        if let Some(client_id) = &self.config.oidc_client_id {
            command.env("BPANE_GATEWAY_OIDC_CLIENT_ID", client_id);
        }
        if let Some(client_secret) = &self.config.oidc_client_secret {
            command.env("BPANE_GATEWAY_OIDC_CLIENT_SECRET", client_secret);
        }
        if let Some(scopes) = &self.config.oidc_scopes {
            command.env("BPANE_GATEWAY_OIDC_SCOPES", scopes);
        }

        let mut child = command.spawn().map_err(|error| {
            RecordingLifecycleError::LaunchFailed(format!(
                "failed to spawn recording worker for session {session_id}: {error}"
            ))
        })?;
        let process_id = child.id();

        if let Err(error) = self
            .session_store
            .upsert_recording_worker_assignment(PersistedSessionRecordingWorkerAssignment {
                session_id,
                recording_id,
                status: SessionRecordingWorkerAssignmentStatus::Running,
                process_id,
            })
            .await
        {
            let _ = child.start_kill();
            return Err(error.into());
        }

        self.launched
            .lock()
            .await
            .insert(session_id, LaunchedRecordingWorker { recording_id });

        let manager = Arc::clone(self);
        tokio::spawn(async move {
            let status = child.wait_with_output().await;
            manager
                .handle_worker_exit(session_id, recording_id, status)
                .await;
        });

        info!(
            session_id = %session_id,
            recording_id = %recording_id,
            "launched recorder worker for always-on session"
        );
        Ok(())
    }

    fn resolve_bearer_token(&self) -> Option<String> {
        self.config
            .bearer_token
            .clone()
            .or_else(|| self.auth_validator.generate_token())
    }

    async fn handle_worker_exit(
        self: Arc<Self>,
        session_id: Uuid,
        recording_id: Uuid,
        status: std::io::Result<std::process::Output>,
    ) {
        self.launched.lock().await.remove(&session_id);

        let exit_message = match status {
            Ok(output) if output.status.success() => {
                format!("recording worker exited before finalizing recording {recording_id}")
            }
            Ok(output) => format!(
                "recording worker exited with status {:?} before finalizing recording {recording_id}",
                output.status.code()
            ),
            Err(error) => format!(
                "recording worker failed while waiting for session {session_id}: {error}"
            ),
        };

        let Ok(Some(recording)) = self
            .session_store
            .get_recording_for_session(session_id, recording_id)
            .await
        else {
            let _ = self
                .session_store
                .clear_recording_worker_assignment(session_id)
                .await;
            return;
        };
        if recording.state.is_terminal() {
            let _ = self
                .session_store
                .clear_recording_worker_assignment(session_id)
                .await;
            return;
        }

        warn!(
            session_id = %session_id,
            recording_id = %recording_id,
            "{exit_message}"
        );
        let _ = self
            .session_store
            .fail_recording_for_session(
                session_id,
                recording_id,
                FailSessionRecordingRequest {
                    error: exit_message,
                    termination_reason: Some(SessionRecordingTerminationReason::WorkerExit),
                },
            )
            .await;
        let _ = self
            .session_store
            .clear_recording_worker_assignment(session_id)
            .await;
    }
}
