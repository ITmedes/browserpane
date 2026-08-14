use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::time::{sleep, Instant};
use tracing::info;
use uuid::Uuid;

use crate::auth::{AuthValidator, AuthenticatedPrincipal};
use crate::session_access::{
    RecordingWorkerAccessTokenManager, SessionAutomationAccessTokenManager,
    SessionConnectTicketManager,
};
use crate::session_control::{
    FailSessionRecordingRequest, PersistedSessionRecordingWorkerAssignment, SessionRecordingMode,
    SessionRecordingTerminationReason, SessionRecordingWorkerAssignmentStatus, SessionStore,
    SessionStoreError, StoredSession,
};
use crate::session_registry::SessionRegistry;
use crate::worker_runtime_control::WorkerRuntimeControl;

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
    pub request_timeout: Duration,
    pub output_limit_bytes: usize,
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
    connect_ticket_manager: Arc<SessionConnectTicketManager>,
    automation_access_token_manager: Arc<SessionAutomationAccessTokenManager>,
    recording_worker_access_token_manager: Arc<RecordingWorkerAccessTokenManager>,
    session_store: SessionStore,
    worker_control: WorkerRuntimeControl,
    launched: Mutex<HashMap<Uuid, LaunchedRecordingWorker>>,
}

impl RecordingLifecycleManager {
    pub fn disabled() -> Self {
        Self { inner: None }
    }

    #[cfg(test)]
    pub fn new(
        config: Option<RecordingWorkerConfig>,
        auth_validator: Arc<AuthValidator>,
        connect_ticket_manager: Arc<SessionConnectTicketManager>,
        automation_access_token_manager: Arc<SessionAutomationAccessTokenManager>,
        recording_worker_access_token_manager: Arc<RecordingWorkerAccessTokenManager>,
        session_store: SessionStore,
    ) -> Result<Self, RecordingLifecycleError> {
        Self::new_with_worker_control(
            config,
            auth_validator,
            connect_ticket_manager,
            automation_access_token_manager,
            recording_worker_access_token_manager,
            session_store,
            WorkerRuntimeControl::direct(),
        )
    }

    pub(crate) fn new_with_worker_control(
        config: Option<RecordingWorkerConfig>,
        auth_validator: Arc<AuthValidator>,
        connect_ticket_manager: Arc<SessionConnectTicketManager>,
        automation_access_token_manager: Arc<SessionAutomationAccessTokenManager>,
        recording_worker_access_token_manager: Arc<RecordingWorkerAccessTokenManager>,
        session_store: SessionStore,
        worker_control: WorkerRuntimeControl,
    ) -> Result<Self, RecordingLifecycleError> {
        let Some(config) = config else {
            return Ok(Self::disabled());
        };
        validate_config(
            &config,
            auth_validator.is_oidc(),
            worker_control.is_broker(),
        )?;
        Ok(Self {
            inner: Some(Arc::new(RecordingLifecycleInner {
                config,
                auth_validator,
                connect_ticket_manager,
                automation_access_token_manager,
                recording_worker_access_token_manager,
                session_store,
                worker_control,
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

    pub async fn ensure_auto_recording_ready(
        &self,
        session: &StoredSession,
        registry: &SessionRegistry,
    ) -> Result<(), RecordingLifecycleError> {
        self.ensure_auto_recording(session).await?;
        if session.recording.mode != SessionRecordingMode::Always {
            return Ok(());
        }
        let Some(inner) = &self.inner else {
            return Ok(());
        };

        inner
            .wait_for_recorder_attachment(session.id, registry)
            .await
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

impl RecordingLifecycleInner {
    fn owner_principal(&self, session: &StoredSession) -> AuthenticatedPrincipal {
        AuthenticatedPrincipal {
            subject: session.owner.subject.clone(),
            issuer: session.owner.issuer.clone(),
            display_name: session.owner.display_name.clone(),
            client_id: None,
            safe_claims: Default::default(),
        }
    }

    async fn wait_for_recorder_attachment(
        &self,
        session_id: Uuid,
        registry: &SessionRegistry,
    ) -> Result<(), RecordingLifecycleError> {
        let deadline = Instant::now() + self.config.connect_timeout;
        loop {
            match self
                .session_store
                .get_latest_recording_for_session(session_id)
                .await?
            {
                Some(recording) if recording.state.is_terminal() => {
                    return Err(RecordingLifecycleError::LaunchFailed(format!(
                        "recording worker for session {session_id} reached terminal state {} before attaching{}",
                        recording.state.as_str(),
                        recording
                            .error
                            .as_deref()
                            .map(|error| format!(": {error}"))
                            .unwrap_or_default()
                    )));
                }
                Some(recording) if recording.state.is_active() => {
                    if let Some(snapshot) = registry.telemetry_snapshot_if_live(session_id).await {
                        if snapshot.recorder_clients > 0 {
                            info!(
                                session_id = %session_id,
                                recording_id = %recording.id,
                                "recorder worker attached before issuing browser access"
                            );
                            return Ok(());
                        }
                    }
                }
                Some(recording) => {
                    return Err(RecordingLifecycleError::LaunchFailed(format!(
                        "recording worker for session {session_id} is not active; latest recording {} is {}",
                        recording.id,
                        recording.state.as_str()
                    )));
                }
                None => {
                    return Err(RecordingLifecycleError::LaunchFailed(format!(
                        "recording worker for session {session_id} did not create a recording"
                    )));
                }
            }

            if Instant::now() >= deadline {
                return Err(RecordingLifecycleError::LaunchFailed(format!(
                    "timed out waiting for recorder worker to attach to session {session_id}"
                )));
            }
            sleep(self.config.poll_interval).await;
        }
    }
}

fn validate_config(
    config: &RecordingWorkerConfig,
    oidc_enabled: bool,
    broker_managed: bool,
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
    if config.request_timeout.is_zero() {
        return Err(RecordingLifecycleError::InvalidConfiguration(
            "recording worker request timeout must be greater than zero".to_string(),
        ));
    }
    if config.output_limit_bytes == 0 {
        return Err(RecordingLifecycleError::InvalidConfiguration(
            "recording worker output limit must be greater than zero".to_string(),
        ));
    }
    if oidc_enabled
        && !broker_managed
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
