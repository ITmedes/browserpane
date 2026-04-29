use super::*;

impl AutomationTaskRepository<'_> {
    pub(in crate::session_control) async fn transition_automation_task(
        &self,
        id: Uuid,
        request: AutomationTaskTransitionRequest,
    ) -> Result<Option<StoredAutomationTask>, SessionStoreError> {
        let mut client = self.store.db.client().await?;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;

        let current_row = transaction
            .query_opt(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    display_name,
                    executor,
                    state,
                    session_id,
                    session_source,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    cancel_requested_at,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                FROM control_automation_tasks
                WHERE id = $1
                FOR UPDATE
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to lock automation task for transition: {error}"
                ))
            })?;
        let Some(current_row) = current_row else {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Ok(None);
        };
        let current = row_to_stored_automation_task(&current_row)?;
        let transition_plan = plan_automation_task_transition(&current, &request, Utc::now())
            .map_err(|error| SessionStoreError::Conflict(error.to_string()))?;
        let now = transition_plan.task_updated_at;
        let artifact_refs = json_string_array(&transition_plan.task_artifact_refs);
        let row = transaction
            .query_one(
                r#"
                UPDATE control_automation_tasks
                SET
                    state = $2,
                    output = $3::jsonb,
                    error = $4,
                    artifact_refs = $5::jsonb,
                    started_at = $6,
                    completed_at = $7,
                    updated_at = $8
                WHERE id = $1
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    display_name,
                    executor,
                    state,
                    session_id,
                    session_source,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    cancel_requested_at,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                "#,
                &[
                    &id,
                    &transition_plan.task_state.as_str(),
                    &transition_plan.task_output,
                    &transition_plan.task_error,
                    &artifact_refs,
                    &transition_plan.task_started_at,
                    &transition_plan.task_completed_at,
                    &transition_plan.task_updated_at,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to update automation task state: {error}"
                ))
            })?;

        transaction
            .execute(
                r#"
                INSERT INTO control_automation_task_events (
                    id,
                    task_id,
                    event_type,
                    message,
                    data,
                    created_at
                )
                VALUES ($1, $2, $3, $4, $5::jsonb, $6)
                "#,
                &[
                    &Uuid::now_v7(),
                    &id,
                    &transition_plan.task_event_type,
                    &transition_plan.task_event_message,
                    &transition_plan.task_event_data,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to insert automation task transition event: {error}"
                ))
            })?;

        let task = row_to_stored_automation_task(&row)?;
        transaction
            .execute(
                r#"
                UPDATE control_workflow_runs
                SET
                    state = $2,
                    output = $3::jsonb,
                    error = $4,
                    artifact_refs = $5::jsonb,
                    started_at = $6,
                    completed_at = $7,
                    updated_at = $8
                WHERE automation_task_id = $1
                "#,
                &[
                    &task.id,
                    &WorkflowRunState::from(task.state).as_str(),
                    &task.output,
                    &task.error,
                    &json_string_array(&task.artifact_refs),
                    &task.started_at,
                    &task.completed_at,
                    &task.updated_at,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to sync workflow run after automation task transition: {error}"
                ))
            })?;

        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;

        Ok(Some(task))
    }
}
