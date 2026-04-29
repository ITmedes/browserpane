use super::*;

impl WorkflowRunRepository<'_> {
    pub(in crate::session_control) async fn list_workflow_run_events_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Vec<StoredWorkflowRunEvent>, SessionStoreError> {
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
                    event_type,
                    message,
                    data,
                    created_at
                FROM control_workflow_run_events
                WHERE run_id = $1
                ORDER BY created_at ASC, id ASC
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to list workflow run events: {error}"))
            })?;
        rows.iter().map(row_to_stored_workflow_run_event).collect()
    }

    pub(in crate::session_control) async fn list_workflow_run_events(
        &self,
        id: Uuid,
    ) -> Result<Vec<StoredWorkflowRunEvent>, SessionStoreError> {
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
                    event_type,
                    message,
                    data,
                    created_at
                FROM control_workflow_run_events
                WHERE run_id = $1
                ORDER BY created_at ASC, id ASC
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to list workflow run events: {error}"))
            })?;
        rows.iter().map(row_to_stored_workflow_run_event).collect()
    }
    pub(in crate::session_control) async fn append_workflow_run_event_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        request: PersistWorkflowRunEventRequest,
    ) -> Result<Option<StoredWorkflowRunEvent>, SessionStoreError> {
        let Some(run) = self.get_workflow_run_for_owner(principal, id).await? else {
            return Ok(None);
        };
        let now = Utc::now();
        let mut client = self.store.db.client().await?;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;
        let event_id = Uuid::now_v7();
        let event = StoredWorkflowRunEvent {
            id: event_id,
            run_id: id,
            event_type: request.event_type,
            message: request.message,
            data: request.data,
            created_at: now,
        };
        let row = transaction
            .query_opt(
                r#"
                WITH inserted AS (
                    INSERT INTO control_workflow_run_events (
                        id,
                        run_id,
                        event_type,
                        message,
                        data,
                        created_at
                    )
                    VALUES ($2, $1, $3, $4, $5::jsonb, $6)
                    RETURNING
                        id,
                        run_id,
                        event_type,
                        message,
                        data,
                        created_at
                )
                UPDATE control_workflow_runs
                SET updated_at = $6
                WHERE id = $1
                RETURNING (SELECT id FROM inserted) AS inserted_id
                "#,
                &[
                    &id,
                    &event_id,
                    &event.event_type,
                    &event.message,
                    &event.data,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to append workflow run event: {error}"))
            })?;
        let Some(row) = row else {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Ok(None);
        };
        let inserted_id: Uuid = row.get("inserted_id");
        if inserted_id != event.id {
            return Err(SessionStoreError::Backend(
                "workflow run event insert returned unexpected id".to_string(),
            ));
        }
        PostgresSessionStore::enqueue_workflow_event_deliveries(&transaction, &run, &event).await?;
        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;
        Ok(Some(event))
    }

    pub(in crate::session_control) async fn append_workflow_run_event(
        &self,
        id: Uuid,
        request: PersistWorkflowRunEventRequest,
    ) -> Result<Option<StoredWorkflowRunEvent>, SessionStoreError> {
        let Some(run) = self.get_workflow_run_by_id(id).await? else {
            return Ok(None);
        };
        let now = Utc::now();
        let mut client = self.store.db.client().await?;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;
        let event_id = Uuid::now_v7();
        let event = StoredWorkflowRunEvent {
            id: event_id,
            run_id: id,
            event_type: request.event_type,
            message: request.message,
            data: request.data,
            created_at: now,
        };
        let row = transaction
            .query_opt(
                r#"
                WITH inserted AS (
                    INSERT INTO control_workflow_run_events (
                        id,
                        run_id,
                        event_type,
                        message,
                        data,
                        created_at
                    )
                    VALUES ($2, $1, $3, $4, $5::jsonb, $6)
                    RETURNING
                        id,
                        run_id,
                        event_type,
                        message,
                        data,
                        created_at
                )
                UPDATE control_workflow_runs
                SET updated_at = $6
                WHERE id = $1
                RETURNING (SELECT id FROM inserted) AS inserted_id
                "#,
                &[
                    &id,
                    &event_id,
                    &event.event_type,
                    &event.message,
                    &event.data,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to append workflow run event: {error}"))
            })?;
        let Some(row) = row else {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Ok(None);
        };
        let inserted_id: Uuid = row.get("inserted_id");
        if inserted_id != event.id {
            return Err(SessionStoreError::Backend(
                "workflow run event insert returned unexpected id".to_string(),
            ));
        }
        PostgresSessionStore::enqueue_workflow_event_deliveries(&transaction, &run, &event).await?;
        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;
        Ok(Some(event))
    }
}
