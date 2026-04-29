use std::collections::HashSet;

use super::*;

pub(in crate::session_control) fn validate_create_request(
    request: &CreateSessionRequest,
) -> Result<(), SessionStoreError> {
    if let Some(viewport) = &request.viewport {
        if viewport.width == 0 || viewport.height == 0 {
            return Err(SessionStoreError::InvalidRequest(
                "viewport width and height must be greater than zero".to_string(),
            ));
        }
    }
    if let Some(idle_timeout_sec) = request.idle_timeout_sec {
        if idle_timeout_sec == 0 {
            return Err(SessionStoreError::InvalidRequest(
                "idle_timeout_sec must be greater than zero when provided".to_string(),
            ));
        }
    }
    if let Some(integration_context) = &request.integration_context {
        if !integration_context.is_object() {
            return Err(SessionStoreError::InvalidRequest(
                "integration_context must be a JSON object when provided".to_string(),
            ));
        }
    }
    if let Some(retention_sec) = request.recording.retention_sec {
        if retention_sec == 0 {
            return Err(SessionStoreError::InvalidRequest(
                "recording.retention_sec must be greater than zero when provided".to_string(),
            ));
        }
    }
    let mut requested_extension_ids = HashSet::new();
    for extension_id in &request.extension_ids {
        if !requested_extension_ids.insert(*extension_id) {
            return Err(SessionStoreError::InvalidRequest(
                "extension_ids must not contain duplicates".to_string(),
            ));
        }
    }
    for extension in &request.extensions {
        if extension.name.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "session extensions must not contain an empty name".to_string(),
            ));
        }
        if extension.version.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "session extensions must not contain an empty version".to_string(),
            ));
        }
        if extension.install_path.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "session extensions must not contain an empty install_path".to_string(),
            ));
        }
    }
    Ok(())
}

pub(in crate::session_control) fn validate_automation_delegate_request(
    request: &SetAutomationDelegateRequest,
) -> Result<(), SessionStoreError> {
    if request.client_id.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "client_id must not be empty".to_string(),
        ));
    }
    if let Some(issuer) = &request.issuer {
        if issuer.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "issuer must not be empty when provided".to_string(),
            ));
        }
    }
    Ok(())
}
