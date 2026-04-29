use std::path::Path;

use super::*;

pub(in crate::session_control) fn validate_credential_binding_request(
    request: &PersistCredentialBindingRequest,
) -> Result<(), SessionStoreError> {
    if request.name.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "credential binding name must not be empty".to_string(),
        ));
    }
    if request.external_ref.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "credential binding external_ref must not be empty".to_string(),
        ));
    }
    if let Some(namespace) = &request.namespace {
        if namespace.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "credential binding namespace must not be empty when provided".to_string(),
            ));
        }
    }
    for origin in &request.allowed_origins {
        if origin.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "credential binding allowed_origins must not contain empty values".to_string(),
            ));
        }
    }
    if let Some(totp) = &request.totp {
        if let Some(issuer) = &totp.issuer {
            if issuer.trim().is_empty() {
                return Err(SessionStoreError::InvalidRequest(
                    "credential binding totp.issuer must not be empty when provided".to_string(),
                ));
            }
        }
        if let Some(account_name) = &totp.account_name {
            if account_name.trim().is_empty() {
                return Err(SessionStoreError::InvalidRequest(
                    "credential binding totp.account_name must not be empty when provided"
                        .to_string(),
                ));
            }
        }
        if let Some(period_sec) = totp.period_sec {
            if period_sec == 0 {
                return Err(SessionStoreError::InvalidRequest(
                    "credential binding totp.period_sec must be greater than zero".to_string(),
                ));
            }
        }
        if let Some(digits) = totp.digits {
            if digits == 0 {
                return Err(SessionStoreError::InvalidRequest(
                    "credential binding totp.digits must be greater than zero".to_string(),
                ));
            }
        }
    }
    for (key, value) in &request.labels {
        if key.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "credential binding label keys must not be empty".to_string(),
            ));
        }
        if value.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "credential binding label values must not be empty".to_string(),
            ));
        }
    }
    Ok(())
}

pub(in crate::session_control) fn validate_extension_definition_request(
    request: &PersistExtensionDefinitionRequest,
) -> Result<(), SessionStoreError> {
    if request.name.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "extension name must not be empty".to_string(),
        ));
    }
    if let Some(description) = &request.description {
        if description.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "extension description must not be empty when provided".to_string(),
            ));
        }
    }
    for (key, value) in &request.labels {
        if key.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "extension label keys must not be empty".to_string(),
            ));
        }
        if value.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "extension label values must not be empty".to_string(),
            ));
        }
    }
    Ok(())
}

pub(in crate::session_control) fn validate_extension_version_request(
    request: &PersistExtensionVersionRequest,
) -> Result<(), SessionStoreError> {
    if request.version.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "extension version must not be empty".to_string(),
        ));
    }
    if request.install_path.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "extension install_path must not be empty".to_string(),
        ));
    }
    if !Path::new(&request.install_path).is_absolute() {
        return Err(SessionStoreError::InvalidRequest(
            "extension install_path must be an absolute path".to_string(),
        ));
    }
    Ok(())
}

pub(in crate::session_control) fn validate_file_workspace_request(
    request: &PersistFileWorkspaceRequest,
) -> Result<(), SessionStoreError> {
    if request.name.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "file workspace name must not be empty".to_string(),
        ));
    }
    for (key, value) in &request.labels {
        if key.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "file workspace label keys must not be empty".to_string(),
            ));
        }
        if value.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "file workspace label values must not be empty".to_string(),
            ));
        }
    }
    Ok(())
}

pub(in crate::session_control) fn validate_file_workspace_file_request(
    request: &PersistFileWorkspaceFileRequest,
) -> Result<(), SessionStoreError> {
    if request.name.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "file workspace file name must not be empty".to_string(),
        ));
    }
    if let Some(media_type) = &request.media_type {
        if media_type.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "file workspace file media_type must not be empty when provided".to_string(),
            ));
        }
    }
    if request.sha256_hex.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "file workspace file sha256_hex must not be empty".to_string(),
        ));
    }
    if let Some(provenance) = &request.provenance {
        if !provenance.is_object() {
            return Err(SessionStoreError::InvalidRequest(
                "file workspace file provenance must be a JSON object when provided".to_string(),
            ));
        }
    }
    if request.artifact_ref.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "file workspace file artifact_ref must not be empty".to_string(),
        ));
    }
    Ok(())
}
