use super::*;

impl AutomationTaskRepository<'_> {
    pub(in crate::session_control) async fn create_automation_task(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistAutomationTaskRequest,
    ) -> Result<StoredAutomationTask, SessionStoreError> {
        let mut client = self.store.db.client().await?;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;

        let now = Utc::now();
        let task_id = Uuid::now_v7();
        let labels_value = json_labels(&request.labels);
        let artifact_refs_value = Value::Array(Vec::new());
        let row = transaction
            .query_one(
                r#"
                INSERT INTO control_automation_tasks (
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
                )
                VALUES (
                    $1, $2, $3, $4, $5, $6, 'pending', $7, $8, $9::jsonb, NULL, NULL,
                    $10::jsonb, $11::jsonb, NULL, NULL, NULL, $12, $12
                )
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
                    &task_id,
                    &principal.subject,
                    &principal.issuer,
                    &principal.display_name,
                    &request.display_name,
                    &request.executor,
                    &request.session_id,
                    &request.session_source.as_str(),
                    &request.input,
                    &artifact_refs_value,
                    &labels_value,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to insert automation task: {error}"))
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
                    &task_id,
                    &"automation_task.created",
                    &"automation task created",
                    &Some(serde_json::json!({
                        "session_id": request.session_id,
                        "session_source": request.session_source.as_str(),
                        "executor": request.executor,
                    })),
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to insert automation task event: {error}"
                ))
            })?;

        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;

        row_to_stored_automation_task(&row)
    }

    pub(in crate::session_control) async fn list_automation_tasks_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredAutomationTask>, SessionStoreError> {
        let rows = self
            .store
            .db
            .client()
            .await?
            .query(
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
                WHERE owner_subject = $1
                  AND owner_issuer = $2
                ORDER BY created_at DESC
                "#,
                &[&principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to list automation tasks: {error}"))
            })?;

        rows.iter().map(row_to_stored_automation_task).collect()
    }

    pub(in crate::session_control) async fn get_automation_task_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredAutomationTask>, SessionStoreError> {
        let row = self
            .store
            .db
            .client()
            .await?
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
                  AND owner_subject = $2
                  AND owner_issuer = $3
                "#,
                &[&id, &principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to load automation task: {error}"))
            })?;
        row.as_ref().map(row_to_stored_automation_task).transpose()
    }

    pub(in crate::session_control) async fn get_automation_task_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredAutomationTask>, SessionStoreError> {
        let row = self
            .store
            .db
            .client()
            .await?
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
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to load automation task by id: {error}"))
            })?;
        row.as_ref().map(row_to_stored_automation_task).transpose()
    }
}
