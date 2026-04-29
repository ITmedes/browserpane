use super::*;

impl PostgresSessionStore {
    pub(in crate::session_control) async fn create_workflow_event_subscription(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistWorkflowEventSubscriptionRequest,
    ) -> Result<StoredWorkflowEventSubscription, SessionStoreError> {
        let now = Utc::now();
        let row = self
            .db
            .client()
            .await?
            .query_one(
                r#"
                INSERT INTO control_workflow_event_subscriptions (
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    target_url,
                    event_types,
                    signing_secret,
                    created_at,
                    updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6::jsonb, $7, $8, $8)
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    target_url,
                    event_types,
                    signing_secret,
                    created_at,
                    updated_at
                "#,
                &[
                    &Uuid::now_v7(),
                    &principal.subject,
                    &principal.issuer,
                    &request.name,
                    &request.target_url,
                    &json_string_array(&request.event_types),
                    &request.signing_secret,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to insert workflow event subscription: {error}"
                ))
            })?;
        row_to_stored_workflow_event_subscription(&row)
    }

    pub(in crate::session_control) async fn list_workflow_event_subscriptions_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredWorkflowEventSubscription>, SessionStoreError> {
        let rows = self
            .db
            .client()
            .await?
            .query(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    target_url,
                    event_types,
                    signing_secret,
                    created_at,
                    updated_at
                FROM control_workflow_event_subscriptions
                WHERE owner_subject = $1
                  AND owner_issuer = $2
                ORDER BY created_at DESC, id DESC
                "#,
                &[&principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to list workflow event subscriptions: {error}"
                ))
            })?;
        rows.iter()
            .map(row_to_stored_workflow_event_subscription)
            .collect()
    }

    pub(in crate::session_control) async fn get_workflow_event_subscription_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowEventSubscription>, SessionStoreError> {
        let row = self
            .db
            .client()
            .await?
            .query_opt(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    target_url,
                    event_types,
                    signing_secret,
                    created_at,
                    updated_at
                FROM control_workflow_event_subscriptions
                WHERE id = $1
                  AND owner_subject = $2
                  AND owner_issuer = $3
                "#,
                &[&id, &principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to load workflow event subscription: {error}"
                ))
            })?;
        row.as_ref()
            .map(row_to_stored_workflow_event_subscription)
            .transpose()
    }

    pub(in crate::session_control) async fn delete_workflow_event_subscription_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowEventSubscription>, SessionStoreError> {
        let row = self
            .db
            .client()
            .await?
            .query_opt(
                r#"
                DELETE FROM control_workflow_event_subscriptions
                WHERE id = $1
                  AND owner_subject = $2
                  AND owner_issuer = $3
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    target_url,
                    event_types,
                    signing_secret,
                    created_at,
                    updated_at
                "#,
                &[&id, &principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to delete workflow event subscription: {error}"
                ))
            })?;
        row.as_ref()
            .map(row_to_stored_workflow_event_subscription)
            .transpose()
    }

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
        let rows = self
            .db
            .client()
            .await?
            .query(
                r#"
                SELECT
                    id,
                    subscription_id,
                    run_id,
                    event_id,
                    event_type,
                    target_url,
                    signing_secret,
                    payload,
                    state,
                    attempt_count,
                    next_attempt_at,
                    last_attempt_at,
                    delivered_at,
                    last_response_status,
                    last_error,
                    created_at,
                    updated_at
                FROM control_workflow_event_deliveries
                WHERE subscription_id = $1
                ORDER BY created_at ASC, event_id ASC, id ASC
                "#,
                &[&subscription_id],
            )
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
        self.db
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
        let rows = self
            .db
            .client()
            .await?
            .query(
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
                    delivery.id,
                    delivery.subscription_id,
                    delivery.run_id,
                    delivery.event_id,
                    delivery.event_type,
                    delivery.target_url,
                    delivery.signing_secret,
                    delivery.payload,
                    delivery.state,
                    delivery.attempt_count,
                    delivery.next_attempt_at,
                    delivery.last_attempt_at,
                    delivery.delivered_at,
                    delivery.last_response_status,
                    delivery.last_error,
                    delivery.created_at,
                    delivery.updated_at
                "#,
                &[&limit, &now],
            )
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
        let mut client = self.db.client().await?;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;
        let row = transaction
            .query_opt(
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
                    id,
                    subscription_id,
                    run_id,
                    event_id,
                    event_type,
                    target_url,
                    signing_secret,
                    payload,
                    state,
                    attempt_count,
                    next_attempt_at,
                    last_attempt_at,
                    delivered_at,
                    last_response_status,
                    last_error,
                    created_at,
                    updated_at
                "#,
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
