use std::collections::HashMap;
use std::time::Duration;

use hmac::{Hmac, Mac};
use serde_json::Value;
use sha2::Sha256;
use uuid::Uuid;

use crate::session_control::SessionStoreError;
use crate::workflow::{StoredWorkflowRun, StoredWorkflowRunEvent};

use super::model::{
    PersistWorkflowEventSubscriptionRequest, StoredWorkflowEventDelivery,
    StoredWorkflowEventDeliveryAttempt, WorkflowEventDeliveryAttemptResource,
};

type HmacSha256 = Hmac<Sha256>;

const SIGNATURE_SCHEME: &str = "v1";

pub fn workflow_event_type_matches(subscription_event_types: &[String], event_type: &str) -> bool {
    subscription_event_types.iter().any(|candidate| {
        if let Some(prefix) = candidate.strip_suffix(".*") {
            event_type.starts_with(prefix) && event_type[prefix.len()..].starts_with('.')
        } else {
            candidate == event_type
        }
    })
}

pub fn build_workflow_event_delivery_payload(
    subscription_id: Uuid,
    delivery_id: Uuid,
    run: &StoredWorkflowRun,
    event: &StoredWorkflowRunEvent,
) -> Value {
    serde_json::json!({
        "subscription_id": subscription_id,
        "delivery_id": delivery_id,
        "event_id": event.id,
        "run_id": run.id,
        "session_id": run.session_id,
        "automation_task_id": run.automation_task_id,
        "workflow_definition_id": run.workflow_definition_id,
        "workflow_definition_version_id": run.workflow_definition_version_id,
        "workflow_version": run.workflow_version,
        "run_state": run.state.as_str(),
        "source_system": run.source_system,
        "source_reference": run.source_reference,
        "client_request_id": run.client_request_id,
        "event_type": event.event_type,
        "message": event.message,
        "data": event.data,
        "created_at": event.created_at,
    })
}

pub fn validate_workflow_event_subscription_request(
    request: &PersistWorkflowEventSubscriptionRequest,
) -> Result<(), SessionStoreError> {
    if request.name.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "workflow event subscription name must not be empty".to_string(),
        ));
    }
    if !(request.target_url.starts_with("http://") || request.target_url.starts_with("https://")) {
        return Err(SessionStoreError::InvalidRequest(
            "workflow event subscription target_url must be http or https".to_string(),
        ));
    }
    if request.signing_secret.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "workflow event subscription signing_secret must not be empty".to_string(),
        ));
    }
    if request.event_types.is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "workflow event subscription event_types must not be empty".to_string(),
        ));
    }
    for event_type in &request.event_types {
        let trimmed = event_type.trim();
        if trimmed.is_empty() || trimmed.contains(' ') {
            return Err(SessionStoreError::InvalidRequest(format!(
                "workflow event subscription event type {event_type:?} is invalid"
            )));
        }
    }
    Ok(())
}

pub fn sign_workflow_event_delivery(
    signing_secret: &str,
    timestamp: &str,
    body: &[u8],
) -> Result<String, SessionStoreError> {
    let mut mac = HmacSha256::new_from_slice(signing_secret.as_bytes()).map_err(|error| {
        SessionStoreError::Backend(format!(
            "failed to initialize workflow event delivery HMAC signer: {error}"
        ))
    })?;
    mac.update(timestamp.as_bytes());
    mac.update(b".");
    mac.update(body);
    let signature = mac.finalize().into_bytes();
    Ok(format!("{SIGNATURE_SCHEME}={}", hex::encode(signature)))
}

pub fn backoff_for_attempt(base_backoff: Duration, attempt_number: u32) -> Duration {
    let exponent = attempt_number.saturating_sub(1).min(8);
    let multiplier = 1u32 << exponent;
    let backoff = base_backoff.saturating_mul(multiplier);
    std::cmp::min(backoff, Duration::from_secs(300))
}

pub(super) fn should_retry_http_status(status: u16) -> bool {
    status == 429 || (500..=599).contains(&status)
}

pub fn sort_workflow_event_deliveries(deliveries: &mut [StoredWorkflowEventDelivery]) {
    deliveries.sort_by(|left, right| {
        left.created_at
            .cmp(&right.created_at)
            .then_with(|| left.event_id.cmp(&right.event_id))
            .then_with(|| left.id.cmp(&right.id))
    });
}

pub fn group_attempts_by_delivery(
    attempts: Vec<StoredWorkflowEventDeliveryAttempt>,
) -> HashMap<Uuid, Vec<WorkflowEventDeliveryAttemptResource>> {
    let mut grouped = HashMap::<Uuid, Vec<WorkflowEventDeliveryAttemptResource>>::new();
    for attempt in attempts {
        grouped
            .entry(attempt.delivery_id)
            .or_default()
            .push(attempt.to_resource());
    }
    for attempts in grouped.values_mut() {
        attempts.sort_by(|left, right| {
            left.attempt_number
                .cmp(&right.attempt_number)
                .then_with(|| left.created_at.cmp(&right.created_at))
        });
    }
    grouped
}
