use super::super::*;

impl InMemorySessionStore {
    pub(in crate::session_control) async fn list_workflow_run_log_retention_candidates(
        &self,
        now: DateTime<Utc>,
        retention: ChronoDuration,
    ) -> Result<Vec<WorkflowRunLogRetentionCandidate>, SessionStoreError> {
        let task_logs = self.automation_task_logs.lock().await;
        let run_logs = self.workflow_run_logs.lock().await;
        let mut candidates = self
            .workflow_runs
            .lock()
            .await
            .iter()
            .filter_map(|run| {
                let completed_at = run.completed_at?;
                if completed_at + retention > now {
                    return None;
                }
                let has_logs = run_logs.iter().any(|log| log.run_id == run.id)
                    || task_logs
                        .iter()
                        .any(|log| log.task_id == run.automation_task_id);
                if !has_logs {
                    return None;
                }
                Some(WorkflowRunLogRetentionCandidate {
                    run_id: run.id,
                    automation_task_id: run.automation_task_id,
                    session_id: run.session_id,
                    expires_at: completed_at + retention,
                })
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            left.expires_at
                .cmp(&right.expires_at)
                .then_with(|| left.run_id.cmp(&right.run_id))
        });
        Ok(candidates)
    }

    pub(in crate::session_control) async fn delete_workflow_run_logs(
        &self,
        run_id: Uuid,
        automation_task_id: Uuid,
    ) -> Result<usize, SessionStoreError> {
        let mut deleted = 0usize;
        {
            let mut logs = self.workflow_run_logs.lock().await;
            let before = logs.len();
            logs.retain(|log| log.run_id != run_id);
            deleted += before - logs.len();
        }
        {
            let mut logs = self.automation_task_logs.lock().await;
            let before = logs.len();
            logs.retain(|log| log.task_id != automation_task_id);
            deleted += before - logs.len();
        }
        if let Some(run) = self
            .workflow_runs
            .lock()
            .await
            .iter_mut()
            .find(|run| run.id == run_id)
        {
            run.updated_at = Utc::now();
        }
        Ok(deleted)
    }

    pub(in crate::session_control) async fn list_workflow_run_output_retention_candidates(
        &self,
        now: DateTime<Utc>,
        retention: ChronoDuration,
    ) -> Result<Vec<WorkflowRunOutputRetentionCandidate>, SessionStoreError> {
        let mut candidates = self
            .workflow_runs
            .lock()
            .await
            .iter()
            .filter_map(|run| {
                let completed_at = run.completed_at?;
                if run.output.is_none() || completed_at + retention > now {
                    return None;
                }
                Some(WorkflowRunOutputRetentionCandidate {
                    run_id: run.id,
                    session_id: run.session_id,
                    expires_at: completed_at + retention,
                })
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            left.expires_at
                .cmp(&right.expires_at)
                .then_with(|| left.run_id.cmp(&right.run_id))
        });
        Ok(candidates)
    }

    pub(in crate::session_control) async fn clear_workflow_run_output(
        &self,
        run_id: Uuid,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        let mut runs = self.workflow_runs.lock().await;
        let Some(run) = runs.iter_mut().find(|run| run.id == run_id) else {
            return Ok(None);
        };
        run.output = None;
        run.updated_at = Utc::now();
        Ok(Some(run.clone()))
    }
}
