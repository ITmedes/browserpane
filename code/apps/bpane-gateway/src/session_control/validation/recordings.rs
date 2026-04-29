use super::*;

pub(in crate::session_control) fn validate_persist_completed_recording_request(
    request: &PersistCompletedSessionRecordingRequest,
) -> Result<(), SessionStoreError> {
    if request.artifact_ref.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "artifact_ref must not be empty".to_string(),
        ));
    }
    if let Some(mime_type) = &request.mime_type {
        if mime_type.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "mime_type must not be empty when provided".to_string(),
            ));
        }
    }
    Ok(())
}

pub(in crate::session_control) fn validate_fail_recording_request(
    request: &FailSessionRecordingRequest,
) -> Result<(), SessionStoreError> {
    if request.error.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "error must not be empty".to_string(),
        ));
    }
    Ok(())
}
