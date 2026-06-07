use tracing::warn;

use super::*;

#[derive(Debug, Clone, Copy)]
pub(super) struct WorkflowWorkerCapacity {
    pub(super) available: bool,
    pub(super) active_workers: usize,
    pub(super) max_active_workers: Option<usize>,
}

#[derive(Debug, Clone, Copy)]
struct WorkflowProjectCapacity {
    available: bool,
    should_queue: bool,
    active_workflow_runs: u32,
    max_active_workflow_runs: Option<u32>,
}

impl WorkflowLifecycleInner {
    pub(super) async fn dispatch_waiting_runs_serialized(
        self: &Arc<Self>,
    ) -> Result<(), WorkflowLifecycleError> {
        let _guard = self.dispatch_lock.lock().await;
        self.dispatch_waiting_runs().await
    }

    pub(super) async fn dispatch_waiting_runs(
        self: &Arc<Self>,
    ) -> Result<(), WorkflowLifecycleError> {
        let runs = self.session_store.list_dispatchable_workflow_runs().await?;
        for run in runs {
            if run.state.is_terminal() {
                continue;
            }
            if let Some(task) = self
                .session_store
                .get_automation_task_by_id(run.automation_task_id)
                .await?
            {
                if task.state.is_terminal() {
                    let _ = self
                        .session_store
                        .reconcile_workflow_run_from_task(run.id)
                        .await?;
                    continue;
                }
            }
            if self
                .session_store
                .get_workflow_run_worker_assignment(run.id)
                .await?
                .is_some()
            {
                continue;
            }

            let Some(version) = self
                .session_store
                .get_workflow_definition_version_by_id(run.workflow_definition_version_id)
                .await?
            else {
                warn!(
                    run_id = %run.id,
                    workflow_definition_version_id = %run.workflow_definition_version_id,
                    "skipping workflow run dispatch because the definition version is missing"
                );
                continue;
            };
            if !supports_executor(&version.executor) {
                continue;
            }

            let project_capacity = self.workflow_project_capacity(&run).await?;
            if !project_capacity.available {
                if project_capacity.should_queue {
                    self.queue_run_for_project_quota(
                        &run,
                        project_capacity.active_workflow_runs,
                        project_capacity.max_active_workflow_runs,
                    )
                    .await?;
                }
                continue;
            }

            let capacity = self.workflow_worker_capacity().await?;
            if !capacity.available {
                self.queue_run_for_worker_capacity(&run, &capacity).await?;
                continue;
            }

            if let Err(error) = self.spawn_worker(&run).await {
                self.fail_run_if_active(
                    run.id,
                    format!("failed to launch workflow worker: {error}"),
                )
                .await?;
                return Err(error);
            }
        }
        Ok(())
    }

    async fn workflow_worker_capacity(
        &self,
    ) -> Result<WorkflowWorkerCapacity, WorkflowLifecycleError> {
        if self.config.max_active_workers == 0 {
            return Ok(WorkflowWorkerCapacity {
                available: true,
                active_workers: self.active_worker_count().await?,
                max_active_workers: None,
            });
        }

        let active_workers = self.active_worker_count().await?;
        Ok(WorkflowWorkerCapacity {
            available: active_workers < self.config.max_active_workers,
            active_workers,
            max_active_workers: Some(self.config.max_active_workers),
        })
    }

    async fn active_worker_count(&self) -> Result<usize, WorkflowLifecycleError> {
        Ok(self
            .session_store
            .list_workflow_run_worker_assignments()
            .await?
            .into_iter()
            .filter(|assignment| {
                matches!(
                    assignment.status,
                    WorkflowRunWorkerAssignmentStatus::Starting
                        | WorkflowRunWorkerAssignmentStatus::Running
                        | WorkflowRunWorkerAssignmentStatus::Stopping
                )
            })
            .count())
    }

    async fn workflow_project_capacity(
        &self,
        run: &crate::workflow::StoredWorkflowRun,
    ) -> Result<WorkflowProjectCapacity, WorkflowLifecycleError> {
        let Some(project_id) = run.project_id else {
            return Ok(WorkflowProjectCapacity {
                available: true,
                should_queue: false,
                active_workflow_runs: 0,
                max_active_workflow_runs: None,
            });
        };
        let owner = AuthenticatedPrincipal {
            subject: run.owner_subject.clone(),
            issuer: run.owner_issuer.clone(),
            display_name: None,
            client_id: None,
            safe_claims: Default::default(),
        };
        let Some(project) = self
            .session_store
            .get_project_for_owner(&owner, project_id)
            .await?
        else {
            self.fail_run_if_active(
                run.id,
                format!("workflow run project {project_id} is no longer visible"),
            )
            .await?;
            return Ok(WorkflowProjectCapacity {
                available: false,
                should_queue: false,
                active_workflow_runs: 0,
                max_active_workflow_runs: Some(0),
            });
        };
        if project.state == ProjectState::Archived {
            self.fail_run_if_active(
                run.id,
                format!("workflow run project {project_id} is archived"),
            )
            .await?;
            return Ok(WorkflowProjectCapacity {
                available: false,
                should_queue: false,
                active_workflow_runs: 0,
                max_active_workflow_runs: Some(0),
            });
        }

        let active_workflow_runs = self
            .session_store
            .count_active_workflow_runs_for_project(&owner, project_id)
            .await?;
        let active_competing_workflow_runs = if run.state.consumes_project_active_quota() {
            active_workflow_runs.saturating_sub(1)
        } else {
            active_workflow_runs
        };
        let max_active_workflow_runs = project.quotas.max_active_workflow_runs;
        let available = max_active_workflow_runs
            .map(|max| active_competing_workflow_runs < max)
            .unwrap_or(true);

        Ok(WorkflowProjectCapacity {
            available,
            should_queue: !available,
            active_workflow_runs: active_competing_workflow_runs,
            max_active_workflow_runs,
        })
    }

    async fn queue_run_for_worker_capacity(
        &self,
        run: &crate::workflow::StoredWorkflowRun,
        capacity: &WorkflowWorkerCapacity,
    ) -> Result<(), WorkflowLifecycleError> {
        if run.state == WorkflowRunState::Queued {
            return Ok(());
        }

        let admission_data = serde_json::json!({
            "admission": {
                "reason": "workflow_worker_capacity",
                "details": {
                    "active_workers": capacity.active_workers,
                    "max_active_workers": capacity.max_active_workers,
                }
            }
        });
        let _ = self
            .session_store
            .append_workflow_run_log(
                run.id,
                crate::workflow::PersistWorkflowRunLogRequest {
                    stream: crate::automation_tasks::AutomationTaskLogStream::System,
                    message: "workflow run queued until worker capacity is available".to_string(),
                },
            )
            .await;
        self.session_store
            .transition_workflow_run(
                run.id,
                WorkflowRunTransitionRequest {
                    state: WorkflowRunState::Queued,
                    output: None,
                    error: None,
                    artifact_refs: Vec::new(),
                    message: Some(
                        "workflow run queued until worker capacity is available".to_string(),
                    ),
                    data: Some(admission_data),
                },
            )
            .await?;
        Ok(())
    }

    async fn queue_run_for_project_quota(
        &self,
        run: &crate::workflow::StoredWorkflowRun,
        active_workflow_runs: u32,
        max_active_workflow_runs: Option<u32>,
    ) -> Result<(), WorkflowLifecycleError> {
        if run.state == WorkflowRunState::Queued {
            return Ok(());
        }

        let message = "workflow run queued until project workflow quota is available".to_string();
        let admission_data = serde_json::json!({
            "admission": {
                "reason": "project_active_workflow_quota_exhausted",
                "details": {
                    "project_id": run.project_id,
                    "active_workflow_runs": active_workflow_runs,
                    "max_active_workflow_runs": max_active_workflow_runs,
                }
            }
        });
        let _ = self
            .session_store
            .append_workflow_run_log(
                run.id,
                crate::workflow::PersistWorkflowRunLogRequest {
                    stream: crate::automation_tasks::AutomationTaskLogStream::System,
                    message: message.clone(),
                },
            )
            .await;
        self.session_store
            .transition_workflow_run(
                run.id,
                WorkflowRunTransitionRequest {
                    state: WorkflowRunState::Queued,
                    output: None,
                    error: None,
                    artifact_refs: Vec::new(),
                    message: Some(message),
                    data: Some(admission_data),
                },
            )
            .await?;
        Ok(())
    }
}
