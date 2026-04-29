use super::*;

impl WorkflowRunRepository<'_> {
    pub(in crate::session_control) async fn list_workflow_run_logs_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Vec<StoredWorkflowRunLog>, SessionStoreError> {
        if self
            .get_workflow_run_for_owner(principal, id)
            .await?
            .is_none()
        {
            return Ok(Vec::new());
        }
        let rows = self
            .store
            .db
            .client()
            .await?
            .query(
                r#"
                SELECT
                    id,
                    run_id,
                    stream,
                    message,
                    created_at
                FROM control_workflow_run_logs
                WHERE run_id = $1
                ORDER BY created_at ASC, id ASC
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to list workflow run logs: {error}"))
            })?;
        rows.iter().map(row_to_stored_workflow_run_log).collect()
    }
    pub(in crate::session_control) async fn append_workflow_run_log(
        &self,
        id: Uuid,
        request: PersistWorkflowRunLogRequest,
    ) -> Result<Option<StoredWorkflowRunLog>, SessionStoreError> {
        let now = Utc::now();
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(
                r#"
                WITH inserted AS (
                    INSERT INTO control_workflow_run_logs (
                        id,
                        run_id,
                        stream,
                        message,
                        created_at
                    )
                    SELECT $2, $1, $3, $4, $5
                    WHERE EXISTS (
                        SELECT 1
                        FROM control_workflow_runs
                        WHERE id = $1
                    )
                    RETURNING
                        id,
                        run_id,
                        stream,
                        message,
                        created_at
                )
                UPDATE control_workflow_runs
                SET updated_at = $5
                WHERE id = $1
                  AND EXISTS (SELECT 1 FROM inserted)
                RETURNING (SELECT id FROM inserted) AS inserted_id
                "#,
                &[
                    &id,
                    &Uuid::now_v7(),
                    &request.stream.as_str(),
                    &request.message,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to append workflow run log: {error}"))
            })?;
        let Some(row) = row else {
            return Ok(None);
        };
        let inserted_id: Option<Uuid> = row.get("inserted_id");
        let Some(inserted_id) = inserted_id else {
            return Ok(None);
        };
        let log_row = self
            .store
            .db
            .client()
            .await?
            .query_one(
                r#"
                SELECT
                    id,
                    run_id,
                    stream,
                    message,
                    created_at
                FROM control_workflow_run_logs
                WHERE id = $1
                "#,
                &[&inserted_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to reload workflow run log: {error}"))
            })?;
        row_to_stored_workflow_run_log(&log_row).map(Some)
    }
}
