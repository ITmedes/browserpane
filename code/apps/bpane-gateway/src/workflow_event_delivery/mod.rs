mod destination_policy;
mod helpers;
mod manager;
mod model;

pub(crate) use destination_policy::{parse_target_url, WorkflowEventDestinationPolicy};

pub use helpers::{
    build_workflow_event_delivery_payload, group_attempts_by_delivery,
    validate_workflow_event_subscription_request, workflow_event_type_matches,
};
pub use manager::{WorkflowEventDeliveryConfig, WorkflowEventDeliveryManager};
pub use model::{
    PersistWorkflowEventSubscriptionRequest, RecordWorkflowEventDeliveryAttemptRequest,
    StoredWorkflowEventDelivery, StoredWorkflowEventDeliveryAttempt,
    StoredWorkflowEventSubscription, WorkflowEventDeliveryListResponse, WorkflowEventDeliveryState,
    WorkflowEventSubscriptionListResponse, WorkflowEventSubscriptionResource,
};

#[cfg(test)]
pub(crate) use helpers::{
    backoff_for_attempt, sign_workflow_event_delivery, sort_workflow_event_deliveries,
};

#[cfg(test)]
mod tests;
