use super::*;

impl AutomationTaskRepository<'_> {
    pub(in crate::session_control) async fn list_automation_task_logs_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Vec<StoredAutomationTaskLog>, SessionStoreError> {
        if self
            .get_automation_task_for_owner(principal, id)
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
                    task_id,
                    stream,
                    message,
                    created_at
                FROM control_automation_task_logs
                WHERE task_id = $1
                ORDER BY created_at ASC, id ASC
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to list automation task logs: {error}"))
            })?;
        rows.iter().map(row_to_stored_automation_task_log).collect()
    }

    pub(in crate::session_control) async fn append_automation_task_log(
        &self,
        id: Uuid,
        stream: AutomationTaskLogStream,
        message: String,
    ) -> Result<Option<StoredAutomationTaskLog>, SessionStoreError> {
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(
                r#"
                INSERT INTO control_automation_task_logs (
                    id,
                    task_id,
                    stream,
                    message,
                    created_at
                )
                SELECT $2, $1, $3, $4, $5
                WHERE EXISTS (
                    SELECT 1
                    FROM control_automation_tasks
                    WHERE id = $1
                )
                RETURNING
                    id,
                    task_id,
                    stream,
                    message,
                    created_at
                "#,
                &[
                    &id,
                    &Uuid::now_v7(),
                    &stream.as_str(),
                    &message,
                    &Utc::now(),
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to append automation task log: {error}"))
            })?;
        row.as_ref()
            .map(row_to_stored_automation_task_log)
            .transpose()
    }
}
