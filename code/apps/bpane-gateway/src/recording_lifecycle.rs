use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::time::{sleep, Instant};
use tracing::{info, warn};
use uuid::Uuid;

use crate::auth::AuthValidator;
use crate::session_control::{
    FailSessionRecordingRequest, PersistedSessionRecordingWorkerAssignment, SessionRecordingMode,
    SessionRecordingTerminationReason, SessionRecordingWorkerAssignmentStatus, SessionStore,
    SessionStoreError, StoredSession,
};

#[derive(Debug, Clone)]
pub struct RecordingWorkerConfig {
    pub bin: PathBuf,
    pub args: Vec<String>,
    pub chrome_executable: PathBuf,
    pub gateway_api_url: String,
    pub page_url: String,
    pub output_root: PathBuf,
    pub cert_spki: Option<String>,
    pub headless: bool,
    pub connect_timeout: Duration,
    pub poll_interval: Duration,
    pub finalize_timeout: Duration,
    pub bearer_token: Option<String>,
    pub oidc_token_url: Option<String>,
    pub oidc_client_id: Option<String>,
    pub oidc_client_secret: Option<String>,
    pub oidc_scopes: Option<String>,
}

#[derive(Debug, Clone)]
pub enum RecordingLifecycleError {
    Disabled(String),
    InvalidConfiguration(String),
    LaunchFailed(String),
    Store(String),
}

impl std::fmt::Display for RecordingLifecycleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Disabled(message)
            | Self::InvalidConfiguration(message)
            | Self::LaunchFailed(message)
            | Self::Store(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for RecordingLifecycleError {}

impl From<SessionStoreError> for RecordingLifecycleError {
    fn from(value: SessionStoreError) -> Self {
        Self::Store(value.to_string())
    }
}

#[derive(Clone, Default)]
pub struct RecordingLifecycleManager {
    inner: Option<Arc<RecordingLifecycleInner>>,
}

struct RecordingLifecycleInner {
    config: RecordingWorkerConfig,
    auth_validator: Arc<AuthValidator>,
    session_store: SessionStore,
    launched: Mutex<HashMap<Uuid, LaunchedRecordingWorker>>,
}

#[derive(Debug, Clone, Copy)]
struct LaunchedRecordingWorker {
    recording_id: Uuid,
}

impl RecordingLifecycleManager {
    pub fn disabled() -> Self {
        Self { inner: None }
    }

    pub fn new(
        config: Option<RecordingWorkerConfig>,
        auth_validator: Arc<AuthValidator>,
        session_store: SessionStore,
    ) -> Result<Self, RecordingLifecycleError> {
        let Some(config) = config else {
            return Ok(Self::disabled());
        };
        validate_config(&config, &auth_validator)?;
        Ok(Self {
            inner: Some(Arc::new(RecordingLifecycleInner {
                config,
                auth_validator,
                session_store,
                launched: Mutex::new(HashMap::new()),
            })),
        })
    }

    pub fn validate_mode(&self, mode: SessionRecordingMode) -> Result<(), RecordingLifecycleError> {
        if mode != SessionRecordingMode::Always {
            return Ok(());
        }
        if self.inner.is_none() {
            return Err(RecordingLifecycleError::Disabled(
                "recording mode=always requires a configured recording worker".to_string(),
            ));
        }
        Ok(())
    }

    pub async fn reconcile_persisted_state(&self) -> Result<(), RecordingLifecycleError> {
        let Some(inner) = &self.inner else {
            return Ok(());
        };

        let assignments = inner
            .session_store
            .list_recording_worker_assignments()
            .await?;
        for assignment in assignments {
            inner.reconcile_assignment(assignment).await?;
        }
        Ok(())
    }

    pub async fn ensure_auto_recording(
        &self,
        session: &StoredSession,
    ) -> Result<(), RecordingLifecycleError> {
        self.validate_mode(session.recording.mode)?;
        if session.recording.mode != SessionRecordingMode::Always {
            return Ok(());
        }
        let Some(inner) = &self.inner else {
            return Ok(());
        };

        {
            let launched = inner.launched.lock().await;
            if let Some(worker) = launched.get(&session.id) {
                info!(
                    session_id = %session.id,
                    recording_id = %worker.recording_id,
                    "recorder worker is already running for always-on session"
                );
                return Ok(());
            }
        }

        if let Some(existing) = inner
            .session_store
            .get_latest_recording_for_session(session.id)
            .await?
        {
            if existing.state.is_active() {
                info!(
                    session_id = %session.id,
                    recording_id = %existing.id,
                    "reusing existing active recording for always-on session"
                );
                return Ok(());
            }
        }

        let recording = inner
            .session_store
            .create_recording_for_session(session.id, session.recording.format, None)
            .await?;

        inner.spawn_worker(session.id, recording.id).await?;
        Ok(())
    }

    pub async fn request_stop_and_wait(
        &self,
        session_id: Uuid,
        termination_reason: SessionRecordingTerminationReason,
    ) -> Result<(), RecordingLifecycleError> {
        let Some(inner) = &self.inner else {
            return Ok(());
        };

        let Some(recording) = inner
            .session_store
            .get_latest_recording_for_session(session_id)
            .await?
        else {
            let _ = inner
                .session_store
                .clear_recording_worker_assignment(session_id)
                .await;
            return Ok(());
        };

        if let Some(mut assignment) = inner
            .session_store
            .get_recording_worker_assignment(session_id)
            .await?
        {
            assignment.status = SessionRecordingWorkerAssignmentStatus::Stopping;
            let _ = inner
                .session_store
                .upsert_recording_worker_assignment(assignment)
                .await;
        }

        if recording.state.is_active() {
            inner
                .session_store
                .stop_recording_for_session(session_id, recording.id, termination_reason)
                .await?;
        } else if recording.state.is_terminal() {
            let _ = inner
                .session_store
                .clear_recording_worker_assignment(session_id)
                .await;
            return Ok(());
        }

        let deadline = Instant::now() + inner.config.finalize_timeout;
        loop {
            let Some(current) = inner
                .session_store
                .get_recording_for_session(session_id, recording.id)
                .await?
            else {
                let _ = inner
                    .session_store
                    .clear_recording_worker_assignment(session_id)
                    .await;
                return Ok(());
            };
            if current.state.is_terminal() {
                let _ = inner
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
            sleep(inner.config.poll_interval).await;
        }
    }
}

impl RecordingLifecycleInner {
    async fn reconcile_assignment(
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

    async fn spawn_worker(
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

fn validate_config(
    config: &RecordingWorkerConfig,
    auth_validator: &AuthValidator,
) -> Result<(), RecordingLifecycleError> {
    if config.bin.as_os_str().is_empty() {
        return Err(RecordingLifecycleError::InvalidConfiguration(
            "recording worker binary path must not be empty".to_string(),
        ));
    }
    if config.chrome_executable.as_os_str().is_empty() {
        return Err(RecordingLifecycleError::InvalidConfiguration(
            "recording worker chrome path must not be empty".to_string(),
        ));
    }
    if auth_validator.is_oidc()
        && config.bearer_token.is_none()
        && (config.oidc_token_url.is_none()
            || config.oidc_client_id.is_none()
            || config.oidc_client_secret.is_none())
    {
        return Err(RecordingLifecycleError::InvalidConfiguration(
            "recording worker auth is not configured for OIDC mode".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::Arc;
    use std::time::Duration;

    use tempfile::tempdir;

    use super::*;
    use crate::auth::{AuthValidator, AuthenticatedPrincipal};
    use crate::session_control::{
        CreateSessionRequest, PersistCompletedSessionRecordingRequest, SessionOwnerMode,
        SessionRecordingFormat, SessionRecordingPolicy,
    };

    fn test_principal() -> AuthenticatedPrincipal {
        AuthenticatedPrincipal {
            subject: "owner".to_string(),
            issuer: "issuer".to_string(),
            display_name: Some("Owner".to_string()),
            client_id: None,
        }
    }

    fn test_config(script: PathBuf, capture_file: PathBuf) -> RecordingWorkerConfig {
        RecordingWorkerConfig {
            bin: script,
            args: vec![capture_file.to_string_lossy().to_string()],
            chrome_executable: PathBuf::from("/tmp/google-chrome"),
            gateway_api_url: "http://127.0.0.1:8932".to_string(),
            page_url: "http://127.0.0.1:8080".to_string(),
            output_root: PathBuf::from("/tmp/bpane-recordings"),
            cert_spki: Some("spki".to_string()),
            headless: true,
            connect_timeout: Duration::from_secs(1),
            poll_interval: Duration::from_millis(10),
            finalize_timeout: Duration::from_millis(100),
            bearer_token: Some("token".to_string()),
            oidc_token_url: None,
            oidc_client_id: None,
            oidc_client_secret: None,
            oidc_scopes: None,
        }
    }

    async fn create_session_with_mode(
        store: &SessionStore,
        mode: SessionRecordingMode,
    ) -> StoredSession {
        store
            .create_session(
                &test_principal(),
                CreateSessionRequest {
                    template_id: None,
                    owner_mode: None,
                    viewport: None,
                    idle_timeout_sec: None,
                    labels: HashMap::new(),
                    integration_context: None,
                    recording: SessionRecordingPolicy {
                        mode,
                        format: SessionRecordingFormat::Webm,
                        retention_sec: None,
                    },
                },
                SessionOwnerMode::Collaborative,
            )
            .await
            .unwrap()
    }

    fn create_capture_script(dir: &tempfile::TempDir) -> PathBuf {
        let script_path = dir.path().join("capture-env.sh");
        fs::write(
            &script_path,
            r#"#!/bin/sh
echo "${BPANE_RECORDING_SESSION_ID} ${BPANE_RECORDING_ID}" > "$1"
"#,
        )
        .unwrap();
        let mut permissions = fs::metadata(&script_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script_path, permissions).unwrap();
        script_path
    }

    #[tokio::test]
    async fn always_mode_launches_worker_and_marks_unfinished_recording_failed() {
        let temp_dir = tempdir().unwrap();
        let capture_file = temp_dir.path().join("capture.txt");
        let script = create_capture_script(&temp_dir);
        let store = SessionStore::in_memory();
        let auth = Arc::new(AuthValidator::from_hmac_secret(vec![9; 32]));
        let manager = RecordingLifecycleManager::new(
            Some(test_config(script, capture_file.clone())),
            auth,
            store.clone(),
        )
        .unwrap();
        let session = create_session_with_mode(&store, SessionRecordingMode::Always).await;

        manager.ensure_auto_recording(&session).await.unwrap();

        for _ in 0..50 {
            if capture_file.exists() {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
        assert!(capture_file.exists());

        let capture = fs::read_to_string(&capture_file).unwrap();
        assert!(capture.contains(&session.id.to_string()));

        let mut latest = None;
        for _ in 0..50 {
            latest = store
                .get_latest_recording_for_session(session.id)
                .await
                .unwrap();
            if latest
                .as_ref()
                .is_some_and(|recording| recording.state.is_terminal())
            {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }

        let recording = latest.expect("recording should exist");
        assert!(matches!(
            recording.state,
            crate::session_control::SessionRecordingState::Failed
        ));
    }

    #[tokio::test]
    async fn request_stop_and_wait_observes_recording_completion() {
        let store = SessionStore::in_memory();
        let auth = Arc::new(AuthValidator::from_hmac_secret(vec![9; 32]));
        let manager = RecordingLifecycleManager::new(
            Some(RecordingWorkerConfig {
                bin: PathBuf::from("/bin/sh"),
                args: vec!["-c".to_string(), "exit 0".to_string()],
                chrome_executable: PathBuf::from("/tmp/google-chrome"),
                gateway_api_url: "http://127.0.0.1:8932".to_string(),
                page_url: "http://127.0.0.1:8080".to_string(),
                output_root: PathBuf::from("/tmp/bpane-recordings"),
                cert_spki: None,
                headless: true,
                connect_timeout: Duration::from_secs(1),
                poll_interval: Duration::from_millis(10),
                finalize_timeout: Duration::from_secs(1),
                bearer_token: Some("token".to_string()),
                oidc_token_url: None,
                oidc_client_id: None,
                oidc_client_secret: None,
                oidc_scopes: None,
            }),
            auth,
            store.clone(),
        )
        .unwrap();
        let session = create_session_with_mode(&store, SessionRecordingMode::Manual).await;
        let recording = store
            .create_recording_for_session(session.id, SessionRecordingFormat::Webm, None)
            .await
            .unwrap();

        let completion_store = store.clone();
        let session_id = session.id;
        let recording_id = recording.id;
        tokio::spawn(async move {
            sleep(Duration::from_millis(20)).await;
            let _ = completion_store
                .complete_recording_for_session(
                    session_id,
                    recording_id,
                    PersistCompletedSessionRecordingRequest {
                        artifact_ref: "local_fs:session/recording.webm".to_string(),
                        mime_type: Some("video/webm".to_string()),
                        bytes: Some(42),
                        duration_ms: Some(1000),
                    },
                )
                .await;
        });

        manager
            .request_stop_and_wait(session.id, SessionRecordingTerminationReason::SessionStop)
            .await
            .unwrap();

        let completed = store
            .get_recording_for_session(session.id, recording.id)
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            completed.state,
            crate::session_control::SessionRecordingState::Ready
        ));
        assert_eq!(
            completed.termination_reason,
            Some(SessionRecordingTerminationReason::SessionStop)
        );
    }

    #[tokio::test]
    async fn reconcile_fails_stale_recording_and_starts_a_fresh_one() {
        let temp_dir = tempdir().unwrap();
        let capture_file = temp_dir.path().join("capture.txt");
        let script = create_capture_script(&temp_dir);
        let store = SessionStore::in_memory();
        let auth = Arc::new(AuthValidator::from_hmac_secret(vec![9; 32]));
        let manager = RecordingLifecycleManager::new(
            Some(test_config(script, capture_file.clone())),
            auth,
            store.clone(),
        )
        .unwrap();
        let session = create_session_with_mode(&store, SessionRecordingMode::Always).await;
        let stale_recording = store
            .create_recording_for_session(session.id, SessionRecordingFormat::Webm, None)
            .await
            .unwrap();
        store
            .upsert_recording_worker_assignment(PersistedSessionRecordingWorkerAssignment {
                session_id: session.id,
                recording_id: stale_recording.id,
                status: SessionRecordingWorkerAssignmentStatus::Running,
                process_id: Some(7777),
            })
            .await
            .unwrap();

        manager.reconcile_persisted_state().await.unwrap();

        for _ in 0..50 {
            if capture_file.exists() {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
        assert!(capture_file.exists());

        let stale = store
            .get_recording_for_session(session.id, stale_recording.id)
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            stale.state,
            crate::session_control::SessionRecordingState::Failed
        ));
        assert_eq!(
            stale.error.as_deref(),
            Some("gateway restarted while recorder worker was active")
        );
        assert_eq!(
            stale.termination_reason,
            Some(SessionRecordingTerminationReason::GatewayRestart)
        );

        let listed = store.list_recordings_for_session(session.id).await.unwrap();
        assert_eq!(listed.len(), 2);
        assert_ne!(listed[0].id, stale_recording.id);
        assert_eq!(listed[0].previous_recording_id, Some(stale_recording.id));
    }
}
