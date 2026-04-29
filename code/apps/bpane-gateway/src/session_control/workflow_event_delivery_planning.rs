use super::*;

pub fn plan_workflow_event_deliveries(
    subscriptions: &[StoredWorkflowEventSubscription],
    run: &StoredWorkflowRun,
    event: &StoredWorkflowRunEvent,
) -> Vec<StoredWorkflowEventDelivery> {
    subscriptions
        .iter()
        .filter(|subscription| {
            subscription.owner_subject == run.owner_subject
                && subscription.owner_issuer == run.owner_issuer
                && workflow_event_type_matches(&subscription.event_types, &event.event_type)
        })
        .map(|subscription| {
            let delivery_id = Uuid::now_v7();
            StoredWorkflowEventDelivery {
                id: delivery_id,
                subscription_id: subscription.id,
                run_id: run.id,
                event_id: event.id,
                event_type: event.event_type.clone(),
                target_url: subscription.target_url.clone(),
                signing_secret: subscription.signing_secret.clone(),
                payload: build_workflow_event_delivery_payload(
                    subscription.id,
                    delivery_id,
                    run,
                    event,
                ),
                state: WorkflowEventDeliveryState::Pending,
                attempt_count: 0,
                next_attempt_at: Some(event.created_at),
                last_attempt_at: None,
                delivered_at: None,
                last_response_status: None,
                last_error: None,
                created_at: event.created_at,
                updated_at: event.created_at,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
