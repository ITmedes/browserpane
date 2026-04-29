use std::process::Stdio;

use tokio::process::Command;
use tracing::{info, warn};

use super::*;

#[derive(Debug, Clone)]
pub(super) struct LaunchedWorkflowWorker {
    pub(super) container_name: String,
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

        let mut command = Command::new(&self.config.docker_bin);
        command.arg("run");
        command.arg("--rm");
        command.arg("--name");
        command.arg(&container_name);
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

        self.launched
            .lock()
            .expect("workflow launched mutex poisoned")
            .insert(
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

    pub(super) async fn handle_worker_exit(
        self: Arc<Self>,
        run_id: Uuid,
        status: std::io::Result<std::process::Output>,
    ) {
        let container_name = self
            .launched
            .lock()
            .expect("workflow launched mutex poisoned")
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
                        format!(
                            "workflow worker exited with status {:?}",
                            output.status.code()
                        )
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

    pub(super) async fn remove_container(
        &self,
        container_name: &str,
    ) -> Result<(), WorkflowLifecycleError> {
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
