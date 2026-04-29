use super::*;

impl WorkflowEventRepository<'_> {
    pub(in crate::session_control) async fn list_workflow_event_deliveries_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        subscription_id: Uuid,
    ) -> Result<Vec<StoredWorkflowEventDelivery>, SessionStoreError> {
        if self
            .get_workflow_event_subscription_for_owner(principal, subscription_id)
            .await?
            .is_none()
        {
            return Ok(Vec::new());
        }

        let query = format!(
            r#"
            SELECT
                {WORKFLOW_EVENT_DELIVERY_COLUMNS}
            FROM control_workflow_event_deliveries
            WHERE subscription_id = $1
            ORDER BY created_at ASC, event_id ASC, id ASC
            "#
        );
        let rows = self
            .store
            .db
            .client()
            .await?
            .query(&query, &[&subscription_id])
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to list workflow event deliveries: {error}"
                ))
            })?;
        rows.iter()
            .map(row_to_stored_workflow_event_delivery)
            .collect()
    }

    pub(in crate::session_control) async fn list_workflow_event_delivery_attempts_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        subscription_id: Uuid,
    ) -> Result<Vec<StoredWorkflowEventDeliveryAttempt>, SessionStoreError> {
        if self
            .get_workflow_event_subscription_for_owner(principal, subscription_id)
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
                    attempt.id,
                    attempt.delivery_id,
                    attempt.attempt_number,
                    attempt.response_status,
                    attempt.error,
                    attempt.created_at
                FROM control_workflow_event_delivery_attempts attempt
                JOIN control_workflow_event_deliveries delivery
                  ON delivery.id = attempt.delivery_id
                WHERE delivery.subscription_id = $1
                ORDER BY attempt.created_at ASC, attempt.id ASC
                "#,
                &[&subscription_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to list workflow event delivery attempts: {error}"
                ))
            })?;
        rows.iter()
            .map(row_to_stored_workflow_event_delivery_attempt)
            .collect()
    }

    pub(in crate::session_control) async fn requeue_inflight_workflow_event_deliveries(
        &self,
    ) -> Result<(), SessionStoreError> {
        self.store
            .db
            .client()
            .await?
            .execute(
                r#"
                UPDATE control_workflow_event_deliveries
                SET
                    state = 'pending',
                    next_attempt_at = NOW(),
                    updated_at = NOW()
                WHERE state = 'delivering'
                "#,
                &[],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to requeue inflight workflow event deliveries: {error}"
                ))
            })?;
        Ok(())
    }

    pub(in crate::session_control) async fn claim_due_workflow_event_deliveries(
        &self,
        limit: usize,
        now: DateTime<Utc>,
    ) -> Result<Vec<StoredWorkflowEventDelivery>, SessionStoreError> {
        let limit = i64::try_from(limit).map_err(|error| {
            SessionStoreError::InvalidRequest(format!(
                "workflow event delivery limit is out of range: {error}"
            ))
        })?;
        let claim_query = format!(
            r#"
            WITH claimed AS (
                SELECT id
                FROM control_workflow_event_deliveries
                WHERE state = 'pending'
                  AND (next_attempt_at IS NULL OR next_attempt_at <= $2)
                ORDER BY created_at ASC, event_id ASC, id ASC
                FOR UPDATE SKIP LOCKED
                LIMIT $1
            )
            UPDATE control_workflow_event_deliveries delivery
            SET
                state = 'delivering',
                updated_at = $2
            FROM claimed
            WHERE delivery.id = claimed.id
            RETURNING
                {WORKFLOW_EVENT_DELIVERY_COLUMNS_FROM_DELIVERY_ALIAS}
            "#
        );
        let rows = self
            .store
            .db
            .client()
            .await?
            .query(&claim_query, &[&limit, &now])
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to claim due workflow event deliveries: {error}"
                ))
            })?;
        let mut deliveries = rows
            .iter()
            .map(row_to_stored_workflow_event_delivery)
            .collect::<Result<Vec<_>, _>>()?;
        deliveries.sort_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| left.event_id.cmp(&right.event_id))
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(deliveries)
    }

    pub(in crate::session_control) async fn record_workflow_event_delivery_attempt(
        &self,
        delivery_id: Uuid,
        request: RecordWorkflowEventDeliveryAttemptRequest,
    ) -> Result<Option<StoredWorkflowEventDelivery>, SessionStoreError> {
        let response_status = request.response_status.map(i32::from);
        let attempt_number = i32::try_from(request.attempt_number).map_err(|error| {
            SessionStoreError::InvalidRequest(format!(
                "workflow event delivery attempt_number is out of range: {error}"
            ))
        })?;
        let mut client = self.store.db.client().await?;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;
        let update_query = format!(
            r#"
            UPDATE control_workflow_event_deliveries
            SET
                state = $2,
                attempt_count = $3,
                next_attempt_at = $4,
                last_attempt_at = $5,
                delivered_at = $6,
                last_response_status = $7,
                last_error = $8,
                updated_at = $5
            WHERE id = $1
            RETURNING
                {WORKFLOW_EVENT_DELIVERY_COLUMNS}
            "#
        );
        let row = transaction
            .query_opt(
                &update_query,
                &[
                    &delivery_id,
                    &request.state.as_str(),
                    &attempt_number,
                    &request.next_attempt_at,
                    &request.attempted_at,
                    &request.delivered_at,
                    &response_status,
                    &request.error,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to update workflow event delivery attempt: {error}"
                ))
            })?;
        let Some(row) = row else {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Ok(None);
        };
        transaction
            .execute(
                r#"
                INSERT INTO control_workflow_event_delivery_attempts (
                    id,
                    delivery_id,
                    attempt_number,
                    response_status,
                    error,
                    created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6)
                "#,
                &[
                    &Uuid::now_v7(),
                    &delivery_id,
                    &attempt_number,
                    &response_status,
                    &request.error,
                    &request.attempted_at,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to insert workflow event delivery attempt: {error}"
                ))
            })?;
        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;
        Ok(Some(row_to_stored_workflow_event_delivery(&row)?))
    }
}
