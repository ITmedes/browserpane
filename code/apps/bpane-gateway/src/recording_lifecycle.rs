use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tracing::info;
use uuid::Uuid;

use crate::auth::AuthValidator;
use crate::session_control::{
    FailSessionRecordingRequest, PersistedSessionRecordingWorkerAssignment, SessionRecordingMode,
    SessionRecordingTerminationReason, SessionRecordingWorkerAssignmentStatus, SessionStore,
    SessionStoreError, StoredSession,
};

mod control;
mod workers;

use workers::LaunchedRecordingWorker;

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
        inner
            .request_stop_and_wait(session_id, termination_reason)
            .await
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
    use tokio::time::sleep;

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
                    extension_ids: Vec::new(),
                    extensions: Vec::new(),
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

        for _ in 0..200 {
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

        for _ in 0..200 {
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
