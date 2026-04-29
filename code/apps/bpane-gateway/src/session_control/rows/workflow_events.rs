use super::super::*;
use super::encoding::row_to_json_string_array;

pub(in crate::session_control) fn row_to_stored_workflow_event_subscription(
    row: &Row,
) -> Result<StoredWorkflowEventSubscription, SessionStoreError> {
    Ok(StoredWorkflowEventSubscription {
        id: row.get("id"),
        owner_subject: row.get("owner_subject"),
        owner_issuer: row.get("owner_issuer"),
        name: row.get("name"),
        target_url: row.get("target_url"),
        event_types: row_to_json_string_array(row.get("event_types"), "event_types")?,
        signing_secret: row.get("signing_secret"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub(in crate::session_control) fn row_to_stored_workflow_event_delivery(
    row: &Row,
) -> Result<StoredWorkflowEventDelivery, SessionStoreError> {
    let state = row
        .get::<_, String>("state")
        .parse::<WorkflowEventDeliveryState>()
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?;
    let last_response_status = row
        .get::<_, Option<i32>>("last_response_status")
        .map(u16::try_from)
        .transpose()
        .map_err(|error| {
            SessionStoreError::Backend(format!(
                "workflow event delivery last_response_status is out of range: {error}"
            ))
        })?;
    let attempt_count = row
        .get::<_, i32>("attempt_count")
        .try_into()
        .map_err(|error| {
            SessionStoreError::Backend(format!(
                "workflow event delivery attempt_count is out of range: {error}"
            ))
        })?;
    Ok(StoredWorkflowEventDelivery {
        id: row.get("id"),
        subscription_id: row.get("subscription_id"),
        run_id: row.get("run_id"),
        event_id: row.get("event_id"),
        event_type: row.get("event_type"),
        target_url: row.get("target_url"),
        signing_secret: row.get("signing_secret"),
        payload: row.get("payload"),
        state,
        attempt_count,
        next_attempt_at: row.get("next_attempt_at"),
        last_attempt_at: row.get("last_attempt_at"),
        delivered_at: row.get("delivered_at"),
        last_response_status,
        last_error: row.get("last_error"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub(in crate::session_control) fn row_to_stored_workflow_event_delivery_attempt(
    row: &Row,
) -> Result<StoredWorkflowEventDeliveryAttempt, SessionStoreError> {
    let attempt_number = row
        .get::<_, i32>("attempt_number")
        .try_into()
        .map_err(|error| {
            SessionStoreError::Backend(format!(
                "workflow event delivery attempt_number is out of range: {error}"
            ))
        })?;
    let response_status = row
        .get::<_, Option<i32>>("response_status")
        .map(u16::try_from)
        .transpose()
        .map_err(|error| {
            SessionStoreError::Backend(format!(
                "workflow event delivery response_status is out of range: {error}"
            ))
        })?;
    Ok(StoredWorkflowEventDeliveryAttempt {
        id: row.get("id"),
        delivery_id: row.get("delivery_id"),
        attempt_number,
        response_status,
        error: row.get("error"),
        created_at: row.get("created_at"),
    })
}
