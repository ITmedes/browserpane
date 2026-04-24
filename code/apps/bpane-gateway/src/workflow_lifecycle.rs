use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use tokio::process::Command;
use tokio::sync::Mutex;
use tracing::{info, warn};
use uuid::Uuid;

use crate::automation_access_token::SessionAutomationAccessTokenManager;
use crate::auth::AuthenticatedPrincipal;
use crate::auth::AuthValidator;
use crate::session_control::{
    PersistedWorkflowRunWorkerAssignment, SessionStore, SessionStoreError,
    WorkflowRunWorkerAssignmentStatus,
};
use crate::workflow::{WorkflowRunState, WorkflowRunTransitionRequest};

#[derive(Debug, Clone)]
pub struct WorkflowWorkerConfig {
    pub docker_bin: PathBuf,
    pub image: String,
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
    launched: Mutex<HashMap<Uuid, LaunchedWorkflowWorker>>,
}

#[derive(Debug, Clone)]
struct LaunchedWorkflowWorker {
    container_name: String,
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
                launched: Mutex::new(HashMap::new()),
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
}

impl WorkflowLifecycleInner {
    async fn reconcile_assignment(
        self: &Arc<Self>,
        assignment: PersistedWorkflowRunWorkerAssignment,
    ) -> Result<(), WorkflowLifecycleError> {
        info!(
            run_id = %assignment.run_id,
            session_id = %assignment.session_id,
            automation_task_id = %assignment.automation_task_id,
            "reconciling persisted workflow worker assignment after gateway restart"
        );

        if let Some(container_name) = assignment.container_name.as_deref() {
            if let Err(error) = self.remove_container(container_name).await {
                warn!(
                    run_id = %assignment.run_id,
                    container_name,
                    "failed to remove stale workflow worker container during reconcile: {error}"
                );
            }
        }

        self.session_store
            .clear_workflow_run_worker_assignment(assignment.run_id)
            .await?;
        self.fail_run_if_active(
            assignment.run_id,
            "gateway restarted while workflow worker was active".to_string(),
        )
        .await?;
        Ok(())
    }

    async fn ensure_run_started(self: &Arc<Self>, run_id: Uuid) -> Result<(), WorkflowLifecycleError> {
        let Some(run) = self.session_store.get_workflow_run_by_id(run_id).await? else {
            return Ok(());
        };
        if run.state.is_terminal() {
            let _ = self
                .session_store
                .clear_workflow_run_worker_assignment(run_id)
                .await;
            return Ok(());
        }

        {
            let launched = self.launched.lock().await;
            if launched.contains_key(&run_id) {
                return Ok(());
            }
        }

        if self
            .session_store
            .get_workflow_run_worker_assignment(run_id)
            .await?
            .is_some()
        {
            return Ok(());
        }

        if let Err(error) = self.spawn_worker(&run).await {
            self.fail_run_if_active(
                run_id,
                format!("failed to launch workflow worker: {error}"),
            )
            .await?;
            return Err(error);
        }
        Ok(())
    }

    async fn cancel_run(&self, run_id: Uuid) -> Result<(), WorkflowLifecycleError> {
        let Some(mut assignment) = self
            .session_store
            .get_workflow_run_worker_assignment(run_id)
            .await?
        else {
            return Ok(());
        };

        assignment.status = WorkflowRunWorkerAssignmentStatus::Stopping;
        self.session_store
            .upsert_workflow_run_worker_assignment(assignment.clone())
            .await?;
        if let Some(container_name) = assignment.container_name.as_deref() {
            self.remove_container(container_name).await?;
        }
        Ok(())
    }

    async fn spawn_worker(
        self: &Arc<Self>,
        run: &crate::workflow::StoredWorkflowRun,
    ) -> Result<(), WorkflowLifecycleError> {
        let session = self
            .session_store
            .get_session_by_id(run.session_id)
            .await?
            .ok_or_else(|| {
                WorkflowLifecycleError::LaunchFailed(format!(
                    "workflow run {} references missing session {}",
                    run.id, run.session_id
                ))
            })?;
        let automation_access_token = self
            .automation_access_token_manager
            .issue_token(
                run.session_id,
                &AuthenticatedPrincipal {
                    subject: session.owner.subject.clone(),
                    issuer: session.owner.issuer.clone(),
                    display_name: session.owner.display_name.clone(),
                    client_id: None,
                },
            )
            .map_err(|error| {
                WorkflowLifecycleError::LaunchFailed(format!(
                    "failed to issue automation access token for workflow run {}: {error}",
                    run.id
                ))
            })?;
        let container_name = format!(
            "{}-{}",
            self.config.container_name_prefix,
            run.id.simple()
        );

        let mut command = Command::new(&self.config.docker_bin);
        command.arg("run");
        command.arg("--rm");
        command.arg("--name");
        command.arg(&container_name);
        if let Some(network) = self.config.network.as_deref() {
            command.arg("--network");
            command.arg(network);
        }
        append_container_env(
            &mut command,
            "BPANE_WORKFLOW_RUN_ID",
            run.id.to_string(),
        );
        append_container_env(
            &mut command,
            "BPANE_GATEWAY_API_URL",
            self.config.gateway_api_url.clone(),
        );
        append_container_env(
            &mut command,
            "BPANE_WORKFLOW_WORK_ROOT",
            self.config.work_root.to_string_lossy().into_owned(),
        );
        append_container_env(
            &mut command,
            "BPANE_SESSION_AUTOMATION_ACCESS_TOKEN",
            automation_access_token.token,
        );
        if let Some(bearer_token) = self.resolve_bearer_token() {
            append_container_env(&mut command, "BPANE_WORKFLOW_BEARER_TOKEN", bearer_token);
        }
        if let Some(token_url) = self.config.oidc_token_url.as_deref() {
            append_container_env(
                &mut command,
                "BPANE_GATEWAY_OIDC_TOKEN_URL",
                token_url.to_string(),
            );
        }
        if let Some(client_id) = self.config.oidc_client_id.as_deref() {
            append_container_env(
                &mut command,
                "BPANE_GATEWAY_OIDC_CLIENT_ID",
                client_id.to_string(),
            );
        }
        if let Some(client_secret) = self.config.oidc_client_secret.as_deref() {
            append_container_env(
                &mut command,
                "BPANE_GATEWAY_OIDC_CLIENT_SECRET",
                client_secret.to_string(),
            );
        }
        if let Some(scopes) = self.config.oidc_scopes.as_deref() {
            append_container_env(
                &mut command,
                "BPANE_GATEWAY_OIDC_SCOPES",
                scopes.to_string(),
            );
        }
        command.arg(&self.config.image);
        command.stdin(Stdio::null());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        let mut child = command.spawn().map_err(|error| {
            WorkflowLifecycleError::LaunchFailed(format!(
                "failed to spawn workflow worker for run {}: {error}",
                run.id
            ))
        })?;
        let process_id = child.id();

        if let Err(error) = self
            .session_store
            .upsert_workflow_run_worker_assignment(PersistedWorkflowRunWorkerAssignment {
                run_id: run.id,
                session_id: run.session_id,
                automation_task_id: run.automation_task_id,
                status: WorkflowRunWorkerAssignmentStatus::Running,
                process_id,
                container_name: Some(container_name.clone()),
            })
            .await
        {
            let _ = child.start_kill();
            let _ = self.remove_container(&container_name).await;
            return Err(error.into());
        }

        self.launched.lock().await.insert(
            run.id,
            LaunchedWorkflowWorker {
                container_name: container_name.clone(),
            },
        );

        let manager = Arc::clone(self);
        let run_id = run.id;
        tokio::spawn(async move {
            let status = child.wait_with_output().await;
            manager.handle_worker_exit(run_id, status).await;
        });

        info!(
            run_id = %run.id,
            session_id = %run.session_id,
            automation_task_id = %run.automation_task_id,
            container_name,
            "launched workflow worker for run"
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
        run_id: Uuid,
        status: std::io::Result<std::process::Output>,
    ) {
        let container_name = self
            .launched
            .lock()
            .await
            .remove(&run_id)
            .map(|worker| worker.container_name);

        if let Some(container_name) = container_name.as_deref() {
            let _ = self.remove_container(container_name).await;
        }

        let exit_message = match status {
            Ok(output) if output.status.success() => {
                format!("workflow worker exited before completing workflow run {run_id}")
            }
            Ok(output) => {
                let detail = last_non_empty_line(&output.stderr)
                    .or_else(|| last_non_empty_line(&output.stdout))
                    .unwrap_or_else(|| {
                        format!("workflow worker exited with status {:?}", output.status.code())
                    });
                format!("workflow worker exited before completing workflow run {run_id}: {detail}")
            }
            Err(error) => format!("workflow worker failed while waiting for run {run_id}: {error}"),
        };

        let Ok(Some(run)) = self.session_store.get_workflow_run_by_id(run_id).await else {
            let _ = self
                .session_store
                .clear_workflow_run_worker_assignment(run_id)
                .await;
            return;
        };
        if run.state.is_terminal() {
            let _ = self
                .session_store
                .clear_workflow_run_worker_assignment(run_id)
                .await;
            return;
        }

        warn!(run_id = %run_id, "{exit_message}");
        let _ = self.fail_run_if_active(run_id, exit_message).await;
        let _ = self
            .session_store
            .clear_workflow_run_worker_assignment(run_id)
            .await;
    }

    async fn fail_run_if_active(
        &self,
        run_id: Uuid,
        error: String,
    ) -> Result<(), WorkflowLifecycleError> {
        let Some(run) = self.session_store.get_workflow_run_by_id(run_id).await? else {
            let _ = self
                .session_store
                .clear_workflow_run_worker_assignment(run_id)
                .await;
            return Ok(());
        };
        if run.state.is_terminal() {
            let _ = self
                .session_store
                .clear_workflow_run_worker_assignment(run_id)
                .await;
            return Ok(());
        }

        let _ = self
            .session_store
            .append_workflow_run_log(
                run_id,
                crate::workflow::PersistWorkflowRunLogRequest {
                    stream: crate::automation_task::AutomationTaskLogStream::System,
                    message: error.clone(),
                },
            )
            .await;
        let _ = self
            .session_store
            .transition_workflow_run(
                run_id,
                WorkflowRunTransitionRequest {
                    state: WorkflowRunState::Failed,
                    output: None,
                    error: Some(error),
                    artifact_refs: Vec::new(),
                    message: Some("workflow worker failed".to_string()),
                    data: None,
                },
            )
            .await?;
        Ok(())
    }

    async fn remove_container(&self, container_name: &str) -> Result<(), WorkflowLifecycleError> {
        let output = Command::new(&self.config.docker_bin)
            .arg("rm")
            .arg("-f")
            .arg(container_name)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|error| {
                WorkflowLifecycleError::LaunchFailed(format!(
                    "failed to remove workflow worker container {container_name}: {error}"
                ))
            })?;
        if output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.contains("No such container") {
            return Ok(());
        }

        Err(WorkflowLifecycleError::LaunchFailed(format!(
            "failed to remove workflow worker container {container_name}: {}",
            if stderr.is_empty() {
                format!("exit status {:?}", output.status.code())
            } else {
                stderr
            }
        )))
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

fn append_container_env(command: &mut Command, key: &str, value: String) {
    command.arg("-e");
    command.arg(format!("{key}={value}"));
}

fn last_non_empty_line(bytes: &[u8]) -> Option<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .map(|line| line.trim().to_string())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Duration;

    use tempfile::tempdir;
    use tokio::time::sleep;

    use super::*;
    use crate::automation_access_token::SessionAutomationAccessTokenManager;
    use crate::auth::{AuthValidator, AuthenticatedPrincipal};
    use crate::automation_task::{AutomationTaskSessionSource, PersistAutomationTaskRequest};
    use crate::session_control::{
        CreateSessionRequest, SessionOwnerMode, SessionRecordingPolicy, SessionStore,
    };
    use crate::workflow::{
        PersistWorkflowDefinitionRequest, PersistWorkflowDefinitionVersionRequest,
        PersistWorkflowRunRequest,
    };

    fn test_principal() -> AuthenticatedPrincipal {
        AuthenticatedPrincipal {
            subject: "owner".to_string(),
            issuer: "issuer".to_string(),
            display_name: Some("Owner".to_string()),
            client_id: None,
        }
    }

    fn test_config(script: PathBuf) -> WorkflowWorkerConfig {
        WorkflowWorkerConfig {
            docker_bin: script,
            image: "deploy-workflow-worker:test".to_string(),
            network: Some("deploy_bpane-internal".to_string()),
            container_name_prefix: "bpane-workflow".to_string(),
            gateway_api_url: "http://gateway:8932".to_string(),
            work_root: PathBuf::from("/tmp/bpane-workflows"),
            bearer_token: Some("token".to_string()),
            oidc_token_url: None,
            oidc_client_id: None,
            oidc_client_secret: None,
            oidc_scopes: None,
        }
    }

    async fn create_workflow_run(store: &SessionStore) -> crate::workflow::StoredWorkflowRun {
        let principal = test_principal();
        let session = store
            .create_session(
                &principal,
                CreateSessionRequest {
                    template_id: None,
                    owner_mode: None,
                    viewport: None,
                    idle_timeout_sec: None,
                    labels: HashMap::new(),
                    integration_context: None,
                    recording: SessionRecordingPolicy::default(),
                },
                SessionOwnerMode::Collaborative,
            )
            .await
            .unwrap();
        let task = store
            .create_automation_task(
                &principal,
                PersistAutomationTaskRequest {
                    display_name: Some("Workflow Task".to_string()),
                    executor: "playwright".to_string(),
                    session_id: session.id,
                    session_source: AutomationTaskSessionSource::CreatedSession,
                    input: None,
                    labels: HashMap::new(),
                },
            )
            .await
            .unwrap();
        let workflow = store
            .create_workflow_definition(
                &principal,
                PersistWorkflowDefinitionRequest {
                    name: "Smoke Workflow".to_string(),
                    description: None,
                    labels: HashMap::new(),
                },
            )
            .await
            .unwrap();
        let version = store
            .create_workflow_definition_version(
                &principal,
                PersistWorkflowDefinitionVersionRequest {
                    workflow_definition_id: workflow.id,
                    version: "v1".to_string(),
                    executor: "playwright".to_string(),
                    entrypoint: "workflows/smoke/run.mjs".to_string(),
                    source: None,
                    input_schema: None,
                    output_schema: None,
                    default_session: None,
                    allowed_credential_binding_ids: Vec::new(),
                    allowed_extension_ids: Vec::new(),
                    allowed_file_workspace_ids: Vec::new(),
                },
            )
            .await
            .unwrap();
        store
            .create_workflow_run(
                &principal,
                PersistWorkflowRunRequest {
                    workflow_definition_id: workflow.id,
                    workflow_definition_version_id: version.id,
                    workflow_version: version.version.clone(),
                    session_id: session.id,
                    automation_task_id: task.id,
                    source_snapshot: None,
                    workspace_inputs: Vec::new(),
                    input: None,
                    labels: HashMap::new(),
                },
            )
            .await
            .unwrap()
    }

    fn create_capture_script(dir: &tempfile::TempDir, capture_file: &std::path::Path) -> PathBuf {
        let script_path = dir.path().join("capture-docker.sh");
        fs::write(
            &script_path,
            format!(
                r#"#!/bin/sh
printf '%s\n' "$@" >> "{}"
"#,
                capture_file.display()
            ),
        )
        .unwrap();
        let mut permissions = fs::metadata(&script_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script_path, permissions).unwrap();
        script_path
    }

    #[tokio::test]
    async fn launches_worker_and_marks_unfinished_run_failed() {
        let temp_dir = tempdir().unwrap();
        let capture_file = temp_dir.path().join("capture.txt");
        let script = create_capture_script(&temp_dir, &capture_file);
        let store = SessionStore::in_memory();
        let auth = Arc::new(AuthValidator::from_hmac_secret(vec![9; 32]));
        let automation_access_token_manager = Arc::new(SessionAutomationAccessTokenManager::new(
            vec![7; 32],
            Duration::from_secs(300),
        ));
        let manager = WorkflowLifecycleManager::new(
            Some(WorkflowWorkerConfig {
                docker_bin: script,
                image: "deploy-workflow-worker:test".to_string(),
                network: Some("deploy_bpane-internal".to_string()),
                container_name_prefix: "bpane-workflow".to_string(),
                gateway_api_url: "http://gateway:8932".to_string(),
                work_root: PathBuf::from("/tmp/bpane-workflows"),
                bearer_token: Some("token".to_string()),
                oidc_token_url: None,
                oidc_client_id: None,
                oidc_client_secret: None,
                oidc_scopes: None,
            }),
            auth,
            automation_access_token_manager,
            store.clone(),
        )
        .unwrap();
        let run = create_workflow_run(&store).await;

        manager.ensure_run_started("playwright", run.id).await.unwrap();

        for _ in 0..50 {
            if capture_file.exists() {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
        assert!(capture_file.exists());

        let capture = fs::read_to_string(&capture_file).unwrap();
        assert!(capture.contains("run"));
        assert!(capture.contains("BPANE_WORKFLOW_RUN_ID"));
        assert!(capture.contains(&run.id.to_string()));
        assert!(capture.contains("BPANE_SESSION_AUTOMATION_ACCESS_TOKEN"));
        assert!(capture.contains("deploy-workflow-worker:test"));

        let mut latest = None;
        for _ in 0..50 {
            latest = store.get_workflow_run_by_id(run.id).await.unwrap();
            if latest.as_ref().is_some_and(|run| run.state.is_terminal()) {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }

        let failed = latest.expect("workflow run should exist");
        assert!(matches!(failed.state, WorkflowRunState::Failed));
    }

    #[tokio::test]
    async fn reconcile_fails_stale_run_assignment() {
        let temp_dir = tempdir().unwrap();
        let capture_file = temp_dir.path().join("capture.txt");
        let script = create_capture_script(&temp_dir, &capture_file);
        let store = SessionStore::in_memory();
        let auth = Arc::new(AuthValidator::from_hmac_secret(vec![9; 32]));
        let automation_access_token_manager = Arc::new(SessionAutomationAccessTokenManager::new(
            vec![7; 32],
            Duration::from_secs(300),
        ));
        let manager = WorkflowLifecycleManager::new(
            Some(test_config(script)),
            auth,
            automation_access_token_manager,
            store.clone(),
        )
        .unwrap();
        let run = create_workflow_run(&store).await;
        store
            .upsert_workflow_run_worker_assignment(PersistedWorkflowRunWorkerAssignment {
                run_id: run.id,
                session_id: run.session_id,
                automation_task_id: run.automation_task_id,
                status: WorkflowRunWorkerAssignmentStatus::Running,
                process_id: Some(7777),
                container_name: Some("bpane-workflow-stale".to_string()),
            })
            .await
            .unwrap();

        manager.reconcile_persisted_state().await.unwrap();

        let failed = store.get_workflow_run_by_id(run.id).await.unwrap().unwrap();
        assert!(matches!(failed.state, WorkflowRunState::Failed));
        assert!(store
            .get_workflow_run_worker_assignment(run.id)
            .await
            .unwrap()
            .is_none());
    }
}
