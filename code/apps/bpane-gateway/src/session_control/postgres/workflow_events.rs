use super::*;

mod deliveries;
mod subscriptions;

const WORKFLOW_EVENT_SUBSCRIPTION_COLUMNS: &str = r#"
    id,
    owner_subject,
    owner_issuer,
    name,
    target_url,
    event_types,
    signing_secret,
    created_at,
    updated_at
"#;

const WORKFLOW_EVENT_DELIVERY_COLUMNS: &str = r#"
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
"#;

pub(super) struct WorkflowEventRepository<'a> {
    store: &'a PostgresSessionStore,
}

impl PostgresSessionStore {
    fn workflow_event_repository(&self) -> WorkflowEventRepository<'_> {
        WorkflowEventRepository { store: self }
    }

    pub(in crate::session_control) async fn create_workflow_event_subscription(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistWorkflowEventSubscriptionRequest,
    ) -> Result<StoredWorkflowEventSubscription, SessionStoreError> {
        self.workflow_event_repository()
            .create_workflow_event_subscription(principal, request)
            .await
    }

    pub(in crate::session_control) async fn list_workflow_event_subscriptions_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredWorkflowEventSubscription>, SessionStoreError> {
        self.workflow_event_repository()
            .list_workflow_event_subscriptions_for_owner(principal)
            .await
    }

    pub(in crate::session_control) async fn get_workflow_event_subscription_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowEventSubscription>, SessionStoreError> {
        self.workflow_event_repository()
            .get_workflow_event_subscription_for_owner(principal, id)
            .await
    }

    pub(in crate::session_control) async fn delete_workflow_event_subscription_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowEventSubscription>, SessionStoreError> {
        self.workflow_event_repository()
            .delete_workflow_event_subscription_for_owner(principal, id)
            .await
    }

    pub(in crate::session_control) async fn list_workflow_event_deliveries_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        subscription_id: Uuid,
    ) -> Result<Vec<StoredWorkflowEventDelivery>, SessionStoreError> {
        self.workflow_event_repository()
            .list_workflow_event_deliveries_for_owner(principal, subscription_id)
            .await
    }

    pub(in crate::session_control) async fn list_workflow_event_delivery_attempts_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        subscription_id: Uuid,
    ) -> Result<Vec<StoredWorkflowEventDeliveryAttempt>, SessionStoreError> {
        self.workflow_event_repository()
            .list_workflow_event_delivery_attempts_for_owner(principal, subscription_id)
            .await
    }

    pub(in crate::session_control) async fn requeue_inflight_workflow_event_deliveries(
        &self,
    ) -> Result<(), SessionStoreError> {
        self.workflow_event_repository()
            .requeue_inflight_workflow_event_deliveries()
            .await
    }

    pub(in crate::session_control) async fn claim_due_workflow_event_deliveries(
        &self,
        limit: usize,
        now: DateTime<Utc>,
    ) -> Result<Vec<StoredWorkflowEventDelivery>, SessionStoreError> {
        self.workflow_event_repository()
            .claim_due_workflow_event_deliveries(limit, now)
            .await
    }

    pub(in crate::session_control) async fn record_workflow_event_delivery_attempt(
        &self,
        delivery_id: Uuid,
        request: RecordWorkflowEventDeliveryAttemptRequest,
    ) -> Result<Option<StoredWorkflowEventDelivery>, SessionStoreError> {
        self.workflow_event_repository()
            .record_workflow_event_delivery_attempt(delivery_id, request)
            .await
    }
}
