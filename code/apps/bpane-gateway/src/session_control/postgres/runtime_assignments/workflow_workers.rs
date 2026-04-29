use super::*;

impl RuntimeAssignmentRepository<'_> {
    pub(in crate::session_control) async fn upsert_workflow_run_worker_assignment(
        &self,
        assignment: PersistedWorkflowRunWorkerAssignment,
    ) -> Result<(), SessionStoreError> {
        let process_id = assignment.process_id.map(i64::from);
        self.store
            .db
            .client()
            .await?
            .execute(
                r#"
                INSERT INTO control_workflow_run_workers (
                    run_id,
                    session_id,
                    automation_task_id,
                    status,
                    process_id,
                    container_name,
                    created_at,
                    updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, NOW(), NOW())
                ON CONFLICT (run_id)
                DO UPDATE SET
                    session_id = EXCLUDED.session_id,
                    automation_task_id = EXCLUDED.automation_task_id,
                    status = EXCLUDED.status,
                    process_id = EXCLUDED.process_id,
                    container_name = EXCLUDED.container_name,
                    updated_at = NOW()
                "#,
                &[
                    &assignment.run_id,
                    &assignment.session_id,
                    &assignment.automation_task_id,
                    &assignment.status.as_str(),
                    &process_id,
                    &assignment.container_name,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to upsert workflow run worker assignment: {error}"
                ))
            })?;
        Ok(())
    }

    pub(in crate::session_control) async fn clear_workflow_run_worker_assignment(
        &self,
        run_id: Uuid,
    ) -> Result<(), SessionStoreError> {
        self.store
            .db
            .client()
            .await?
            .execute(
                "DELETE FROM control_workflow_run_workers WHERE run_id = $1",
                &[&run_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to clear workflow run worker assignment: {error}"
                ))
            })?;
        Ok(())
    }

    pub(in crate::session_control) async fn get_workflow_run_worker_assignment(
        &self,
        run_id: Uuid,
    ) -> Result<Option<PersistedWorkflowRunWorkerAssignment>, SessionStoreError> {
        let query = format!(
            r#"
            SELECT
                {WORKFLOW_RUN_WORKER_COLUMNS}
            FROM control_workflow_run_workers
            WHERE run_id = $1
            "#
        );
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(&query, &[&run_id])
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to load workflow run worker assignment: {error}"
                ))
            })?;
        row.as_ref()
            .map(row_to_workflow_run_worker_assignment)
            .transpose()
    }

    pub(in crate::session_control) async fn list_workflow_run_worker_assignments(
        &self,
    ) -> Result<Vec<PersistedWorkflowRunWorkerAssignment>, SessionStoreError> {
        let query = format!(
            r#"
            SELECT
                {WORKFLOW_RUN_WORKER_COLUMNS}
            FROM control_workflow_run_workers
            ORDER BY updated_at DESC, created_at DESC
            "#
        );
        let rows = self
            .store
            .db
            .client()
            .await?
            .query(&query, &[])
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to list workflow run worker assignments: {error}"
                ))
            })?;

        rows.iter()
            .map(row_to_workflow_run_worker_assignment)
            .collect()
    }
}
