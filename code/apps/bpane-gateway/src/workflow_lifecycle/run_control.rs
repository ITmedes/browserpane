use tracing::{info, warn};

use super::*;

impl WorkflowLifecycleInner {
    pub(super) async fn reconcile_assignment(
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

    pub(super) async fn ensure_run_started(
        self: &Arc<Self>,
        run_id: Uuid,
    ) -> Result<(), WorkflowLifecycleError> {
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

        if let Some(task) = self
            .session_store
            .get_automation_task_by_id(run.automation_task_id)
            .await?
        {
            if task.state.is_terminal() {
                let _ = self
                    .session_store
                    .reconcile_workflow_run_from_task(run_id)
                    .await?;
                return Ok(());
            }
        }

        {
            let launched = self
                .launched
                .lock()
                .expect("workflow launched mutex poisoned");
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

        self.dispatch_waiting_runs_serialized().await
    }

    pub(super) async fn cancel_run(&self, run_id: Uuid) -> Result<(), WorkflowLifecycleError> {
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
}
