use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use chrono::Utc;
use tokio::sync::Mutex as AsyncMutex;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::auth::AuthValidator;
use crate::auth::AuthenticatedPrincipal;
use crate::automation_access_token::SessionAutomationAccessTokenManager;
use crate::session_control::{
    PersistedWorkflowRunWorkerAssignment, SessionLifecycleState, SessionStore, SessionStoreError,
    WorkflowRunWorkerAssignmentStatus,
};
use crate::session_manager::SessionManager;
use crate::session_registry::SessionRegistry;
use crate::workflow::{
    parse_workflow_run_runtime_hold_request, WorkflowRunRuntimeHoldRequest, WorkflowRunState,
    WorkflowRunTransitionRequest,
};

mod admission;
mod run_control;
mod runtime_holds;
mod workers;

use workers::LaunchedWorkflowWorker;

#[derive(Debug, Clone)]
pub struct WorkflowWorkerConfig {
    pub docker_bin: PathBuf,
    pub image: String,
    pub max_active_workers: usize,
    pub network: Option<String>,
    pub container_name_prefix: String,
    pub gateway_api_url: String,
    pub work_root: PathBuf,
    pub bearer_token: Option<String>,
    pub oidc_token_url: Option<String>,
    pub oidc_client_id: Option<String>,
    pub oidc_client_secret: Option<String>,
    pub oidc_scopes: Option<String>,
}

#[derive(Debug, Clone)]
pub enum WorkflowLifecycleError {
    InvalidConfiguration(String),
    LaunchFailed(String),
    Store(String),
}

impl std::fmt::Display for WorkflowLifecycleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidConfiguration(message)
            | Self::LaunchFailed(message)
            | Self::Store(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for WorkflowLifecycleError {}

impl From<SessionStoreError> for WorkflowLifecycleError {
    fn from(value: SessionStoreError) -> Self {
        Self::Store(value.to_string())
    }
}

#[derive(Clone, Default)]
pub struct WorkflowLifecycleManager {
    inner: Option<Arc<WorkflowLifecycleInner>>,
}

struct WorkflowLifecycleInner {
    config: WorkflowWorkerConfig,
    auth_validator: Arc<AuthValidator>,
    automation_access_token_manager: Arc<SessionAutomationAccessTokenManager>,
    session_store: SessionStore,
    session_manager: Arc<SessionManager>,
    registry: Arc<SessionRegistry>,
    launched: StdMutex<HashMap<Uuid, LaunchedWorkflowWorker>>,
    dispatch_lock: AsyncMutex<()>,
    runtime_hold_tasks: AsyncMutex<HashMap<Uuid, JoinHandle<()>>>,
}

impl WorkflowLifecycleManager {
    pub fn disabled() -> Self {
        Self { inner: None }
    }

    pub fn new(
        config: Option<WorkflowWorkerConfig>,
        auth_validator: Arc<AuthValidator>,
        automation_access_token_manager: Arc<SessionAutomationAccessTokenManager>,
        session_store: SessionStore,
        session_manager: Arc<SessionManager>,
        registry: Arc<SessionRegistry>,
    ) -> Result<Self, WorkflowLifecycleError> {
        let Some(config) = config else {
            return Ok(Self::disabled());
        };
        validate_config(&config, &auth_validator)?;
        Ok(Self {
            inner: Some(Arc::new(WorkflowLifecycleInner {
                config,
                auth_validator,
                automation_access_token_manager,
                session_store,
                session_manager,
                registry,
                launched: StdMutex::new(HashMap::new()),
                dispatch_lock: AsyncMutex::new(()),
                runtime_hold_tasks: AsyncMutex::new(HashMap::new()),
            })),
        })
    }

    pub async fn reconcile_persisted_state(&self) -> Result<(), WorkflowLifecycleError> {
        let Some(inner) = &self.inner else {
            return Ok(());
        };

        let assignments = inner
            .session_store
            .list_workflow_run_worker_assignments()
            .await?;
        for assignment in assignments {
            inner.reconcile_assignment(assignment).await?;
        }
        inner.dispatch_waiting_runs_serialized().await?;
        inner.reconcile_runtime_holds().await?;
        Ok(())
    }

    pub async fn ensure_run_started(
        &self,
        executor: &str,
        run_id: Uuid,
    ) -> Result<(), WorkflowLifecycleError> {
        let Some(inner) = &self.inner else {
            return Ok(());
        };
        if !supports_executor(executor) {
            return Ok(());
        }
        inner.ensure_run_started(run_id).await
    }

    pub async fn cancel_run(&self, run_id: Uuid) -> Result<(), WorkflowLifecycleError> {
        let Some(inner) = &self.inner else {
            return Ok(());
        };
        inner.cancel_run(run_id).await
    }

    pub async fn reconcile_waiting_runs(&self) -> Result<(), WorkflowLifecycleError> {
        let Some(inner) = &self.inner else {
            return Ok(());
        };
        inner.dispatch_waiting_runs_serialized().await
    }

    pub async fn reconcile_runtime_hold(&self, run_id: Uuid) -> Result<(), WorkflowLifecycleError> {
        let Some(inner) = &self.inner else {
            return Ok(());
        };
        inner.reconcile_runtime_hold(run_id).await
    }
}

fn validate_config(
    config: &WorkflowWorkerConfig,
    auth_validator: &AuthValidator,
) -> Result<(), WorkflowLifecycleError> {
    if config.docker_bin.as_os_str().is_empty() {
        return Err(WorkflowLifecycleError::InvalidConfiguration(
            "workflow worker docker binary path must not be empty".to_string(),
        ));
    }
    if config.image.trim().is_empty() {
        return Err(WorkflowLifecycleError::InvalidConfiguration(
            "workflow worker image must not be empty".to_string(),
        ));
    }
    if config.container_name_prefix.trim().is_empty() {
        return Err(WorkflowLifecycleError::InvalidConfiguration(
            "workflow worker container name prefix must not be empty".to_string(),
        ));
    }
    if config.gateway_api_url.trim().is_empty() {
        return Err(WorkflowLifecycleError::InvalidConfiguration(
            "workflow worker gateway api url must not be empty".to_string(),
        ));
    }
    if auth_validator.is_oidc()
        && config.bearer_token.is_none()
        && (config.oidc_token_url.is_none()
            || config.oidc_client_id.is_none()
            || config.oidc_client_secret.is_none())
    {
        return Err(WorkflowLifecycleError::InvalidConfiguration(
            "workflow worker auth is not configured for OIDC mode".to_string(),
        ));
    }
    Ok(())
}

fn supports_executor(executor: &str) -> bool {
    executor == "playwright"
}

#[cfg(test)]
mod tests;
