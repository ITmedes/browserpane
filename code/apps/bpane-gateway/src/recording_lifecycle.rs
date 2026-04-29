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
mod tests;
