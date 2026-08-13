use std::process::Stdio;
use std::time::Duration;

use bpane_runtime_contract::{
    RuntimeOperationKind, SecretValue, WorkflowWorkerCredentials, WorkflowWorkerLaunchRequest,
};
use tokio::process::Command;
use tracing::{info, warn};

use super::*;
use crate::worker_process_output::{wait_with_bounded_output, BoundedProcessOutput};
use crate::worker_runtime_control::{BrokerWorkerState, WorkerRuntimeControlError};

#[derive(Debug, Clone)]
pub(super) struct LaunchedWorkflowWorker {
    pub(super) container_name: String,
}

enum WorkflowWorkerExit {
    Direct(std::io::Result<BoundedProcessOutput>),
    Broker(Result<BrokerWorkerState, WorkerRuntimeControlError>),
}

impl WorkflowLifecycleInner {
    pub(super) async fn spawn_worker(
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
                    safe_claims: Default::default(),
                },
            )
            .map_err(|error| {
                WorkflowLifecycleError::LaunchFailed(format!(
                    "failed to issue automation access token for workflow run {}: {error}",
                    run.id
                ))
            })?;
        let container_name = format!("{}-{}", self.config.container_name_prefix, run.id.simple());

        self.session_store
            .upsert_workflow_run_worker_assignment(PersistedWorkflowRunWorkerAssignment {
                run_id: run.id,
                session_id: run.session_id,
                automation_task_id: run.automation_task_id,
                status: WorkflowRunWorkerAssignmentStatus::Starting,
                process_id: None,
                container_name: Some(container_name.clone()),
            })
            .await?;

        self.launched
            .lock()
            .expect("workflow launched mutex poisoned")
            .insert(
                run.id,
                LaunchedWorkflowWorker {
                    container_name: container_name.clone(),
                },
            );

        let bearer_token = self.resolve_bearer_token();
        let launch = if self.worker_control.is_broker() {
            self.launch_broker_worker(run, automation_access_token.token, bearer_token)
                .await
        } else {
            self.launch_direct_worker(
                run,
                automation_access_token.token,
                bearer_token,
                &container_name,
            )
            .await
        };
        if let Err(error) = launch {
            self.launched
                .lock()
                .expect("workflow launched mutex poisoned")
                .remove(&run.id);
            let _ = self
                .session_store
                .clear_workflow_run_worker_assignment(run.id)
                .await;
            return Err(error);
        }

        info!(
            run_id = %run.id,
            session_id = %run.session_id,
            automation_task_id = %run.automation_task_id,
            container_name,
            launch_mode = if self.worker_control.is_broker() { "broker" } else { "direct" },
            "launched workflow worker for run"
        );
        Ok(())
    }

    async fn launch_broker_worker(
        self: &Arc<Self>,
        run: &crate::workflow::StoredWorkflowRun,
        automation_access_token: String,
        bearer_token: Option<String>,
    ) -> Result<(), WorkflowLifecycleError> {
        let request = WorkflowWorkerLaunchRequest {
            workflow_run_id: run.id,
            session_id: run.session_id,
            automation_task_id: run.automation_task_id,
            credentials: WorkflowWorkerCredentials {
                session_automation_access_token: worker_secret(automation_access_token, run.id)?,
                gateway_bearer_token: bearer_token
                    .map(|value| worker_secret(value, run.id))
                    .transpose()?,
            },
        };
        if let Err(error) = self.worker_control.launch_workflow(request).await {
            let _ = self
                .session_store
                .clear_workflow_run_worker_assignment(run.id)
                .await;
            return Err(WorkflowLifecycleError::LaunchFailed(format!(
                "runtime broker failed to launch workflow worker for run {}: {error}",
                run.id
            )));
        }

        let manager = Arc::clone(self);
        let run_id = run.id;
        tokio::spawn(async move {
            let status = manager
                .worker_control
                .wait_for_exit(
                    RuntimeOperationKind::WorkflowWorker,
                    run_id,
                    Duration::from_millis(500),
                )
                .await;
            manager
                .handle_worker_exit(run_id, WorkflowWorkerExit::Broker(status))
                .await;
        });
        Ok(())
    }

    async fn launch_direct_worker(
        self: &Arc<Self>,
        run: &crate::workflow::StoredWorkflowRun,
        automation_access_token: String,
        bearer_token: Option<String>,
        container_name: &str,
    ) -> Result<(), WorkflowLifecycleError> {
        let mut command = Command::new(&self.config.docker_bin);
        command.arg("run");
        command.arg("--rm");
        command.arg("--name");
        command.arg(container_name);
        if let Some(network) = self.config.network.as_deref() {
            command.arg("--network");
            command.arg(network);
        }
        append_container_env(&mut command, "BPANE_WORKFLOW_RUN_ID", run.id.to_string());
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
            "BPANE_WORKER_REQUEST_TIMEOUT_MS",
            self.config.request_timeout.as_millis().to_string(),
        );
        append_container_env(
            &mut command,
            "BPANE_WORKER_MAX_OUTPUT_BYTES",
            self.config.output_limit_bytes.to_string(),
        );
        append_container_env(
            &mut command,
            "BPANE_SESSION_AUTOMATION_ACCESS_TOKEN",
            automation_access_token,
        );
        if let Some(bearer_token) = bearer_token {
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

        let child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                let _ = self
                    .session_store
                    .clear_workflow_run_worker_assignment(run.id)
                    .await;
                return Err(WorkflowLifecycleError::LaunchFailed(format!(
                    "failed to spawn workflow worker for run {}: {error}",
                    run.id
                )));
            }
        };

        let manager = Arc::clone(self);
        let run_id = run.id;
        let output_limit_bytes = self.config.output_limit_bytes;
        tokio::spawn(async move {
            let status = wait_with_bounded_output(child, output_limit_bytes).await;
            manager
                .handle_worker_exit(run_id, WorkflowWorkerExit::Direct(status))
                .await;
        });
        Ok(())
    }

    fn resolve_bearer_token(&self) -> Option<String> {
        self.config
            .bearer_token
            .clone()
            .or_else(|| self.auth_validator.generate_token())
    }

    async fn handle_worker_exit(self: Arc<Self>, run_id: Uuid, status: WorkflowWorkerExit) {
        let container_name = self
            .launched
            .lock()
            .expect("workflow launched mutex poisoned")
            .remove(&run_id)
            .map(|worker| worker.container_name);

        if let Some(container_name) = container_name.as_deref() {
            let _ = self.remove_worker(run_id, container_name).await;
        }

        let exit_message = match status {
            WorkflowWorkerExit::Direct(Ok(output)) if output.status.success() => {
                format!("workflow worker exited before completing workflow run {run_id}")
            }
            WorkflowWorkerExit::Direct(Ok(output)) => {
                let detail = last_non_empty_line(&output.stderr)
                    .or_else(|| last_non_empty_line(&output.stdout))
                    .unwrap_or_else(|| {
                        format!(
                            "workflow worker exited with status {:?}",
                            output.status.code()
                        )
                    });
                let truncation = output_truncation_detail(output.omitted_bytes());
                format!(
                    "workflow worker exited before completing workflow run {run_id}: {detail}{truncation}"
                )
            }
            WorkflowWorkerExit::Direct(Err(error)) => {
                format!("workflow worker failed while waiting for run {run_id}: {error}")
            }
            WorkflowWorkerExit::Broker(Ok(BrokerWorkerState::Exited { exit_code })) => format!(
                "workflow worker exited before completing workflow run {run_id} with status {exit_code:?}"
            ),
            WorkflowWorkerExit::Broker(Ok(BrokerWorkerState::Absent)) => {
                format!("workflow worker disappeared before completing workflow run {run_id}")
            }
            WorkflowWorkerExit::Broker(Ok(BrokerWorkerState::Running)) => {
                format!("workflow worker monitor ended unexpectedly for run {run_id}")
            }
            WorkflowWorkerExit::Broker(Err(error)) => format!(
                "workflow worker monitoring failed for run {run_id}: {error}"
            ),
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

    pub(super) async fn fail_run_if_active(
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
                    stream: crate::automation_tasks::AutomationTaskLogStream::System,
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

    pub(super) async fn remove_worker(
        &self,
        run_id: Uuid,
        container_name: &str,
    ) -> Result<(), WorkflowLifecycleError> {
        if self.worker_control.is_broker() {
            return self
                .worker_control
                .remove(RuntimeOperationKind::WorkflowWorker, run_id)
                .await
                .map_err(|error| {
                    WorkflowLifecycleError::LaunchFailed(format!(
                        "runtime broker failed to remove workflow worker for run {run_id}: {error}"
                    ))
                });
        }
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

fn worker_secret(value: String, run_id: Uuid) -> Result<SecretValue, WorkflowLifecycleError> {
    SecretValue::new(value).map_err(|_| {
        WorkflowLifecycleError::LaunchFailed(format!(
            "workflow worker credential for run {run_id} is invalid"
        ))
    })
}

fn output_truncation_detail(omitted_bytes: u64) -> String {
    if omitted_bytes == 0 {
        String::new()
    } else {
        format!(" (omitted {omitted_bytes} earlier output bytes)")
    }
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
