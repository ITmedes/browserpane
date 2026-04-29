use super::*;

impl WorkflowEventRepository<'_> {
    pub(in crate::session_control) async fn create_workflow_event_subscription(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistWorkflowEventSubscriptionRequest,
    ) -> Result<StoredWorkflowEventSubscription, SessionStoreError> {
        let now = Utc::now();
        let insert_query = format!(
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
                {WORKFLOW_EVENT_SUBSCRIPTION_COLUMNS}
            "#
        );
        let row = self
            .store
            .db
            .client()
            .await?
            .query_one(
                &insert_query,
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
        let query = format!(
            r#"
            SELECT
                {WORKFLOW_EVENT_SUBSCRIPTION_COLUMNS}
            FROM control_workflow_event_subscriptions
            WHERE owner_subject = $1
              AND owner_issuer = $2
            ORDER BY created_at DESC, id DESC
            "#
        );
        let rows = self
            .store
            .db
            .client()
            .await?
            .query(&query, &[&principal.subject, &principal.issuer])
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
        let query = format!(
            r#"
            SELECT
                {WORKFLOW_EVENT_SUBSCRIPTION_COLUMNS}
            FROM control_workflow_event_subscriptions
            WHERE id = $1
              AND owner_subject = $2
              AND owner_issuer = $3
            "#
        );
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(&query, &[&id, &principal.subject, &principal.issuer])
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
        let delete_query = format!(
            r#"
            DELETE FROM control_workflow_event_subscriptions
            WHERE id = $1
              AND owner_subject = $2
              AND owner_issuer = $3
            RETURNING
                {WORKFLOW_EVENT_SUBSCRIPTION_COLUMNS}
            "#
        );
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(&delete_query, &[&id, &principal.subject, &principal.issuer])
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
}
