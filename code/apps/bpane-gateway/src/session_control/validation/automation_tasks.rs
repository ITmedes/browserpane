use super::*;

pub(in crate::session_control) fn validate_persist_automation_task_request(
    request: &PersistAutomationTaskRequest,
) -> Result<(), SessionStoreError> {
    if let Some(display_name) = &request.display_name {
        if display_name.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "display_name must not be empty when provided".to_string(),
            ));
        }
    }
    if request.executor.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "executor must not be empty".to_string(),
        ));
    }
    for task_label in &request.labels {
        if task_label.0.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "task label keys must not be empty".to_string(),
            ));
        }
    }
    Ok(())
}

pub(in crate::session_control) fn validate_automation_task_transition_request(
    request: &AutomationTaskTransitionRequest,
) -> Result<(), SessionStoreError> {
    if request.event_type.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "event_type must not be empty".to_string(),
        ));
    }
    if request.event_message.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "event_message must not be empty".to_string(),
        ));
    }
    match request.state {
        AutomationTaskState::Succeeded => {
            if request.error.is_some() {
                return Err(SessionStoreError::InvalidRequest(
                    "succeeded automation tasks must not carry an error".to_string(),
                ));
            }
        }
        AutomationTaskState::Failed | AutomationTaskState::TimedOut => {
            let Some(error) = request.error.as_deref() else {
                return Err(SessionStoreError::InvalidRequest(
                    "failed or timed_out automation tasks require an error".to_string(),
                ));
            };
            if error.trim().is_empty() {
                return Err(SessionStoreError::InvalidRequest(
                    "automation task error must not be empty".to_string(),
                ));
            }
        }
        AutomationTaskState::Cancelled => {
            if request.error.is_some() {
                return Err(SessionStoreError::InvalidRequest(
                    "cancelled automation tasks must not carry an error".to_string(),
                ));
            }
        }
        _ => {}
    }
    for artifact_ref in &request.artifact_refs {
        if artifact_ref.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "artifact_refs entries must not be empty".to_string(),
            ));
        }
    }
    Ok(())
}

pub(in crate::session_control) fn validate_automation_task_log_message(
    message: &str,
) -> Result<(), SessionStoreError> {
    if message.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "automation task log message must not be empty".to_string(),
        ));
    }
    Ok(())
}
