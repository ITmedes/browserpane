use std::path::{Component, Path};

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

pub(in crate::session_control) fn validate_session_file_binding_request(
    request: &mut PersistSessionFileBindingRequest,
) -> Result<(), SessionStoreError> {
    request.mount_path = normalize_session_file_mount_path(&request.mount_path)?;
    if request.mode == SessionFileBindingMode::ScratchOutput {
        return Err(SessionStoreError::InvalidRequest(
            "session file workspace bindings do not support scratch_output mode".to_string(),
        ));
    }
    for (key, value) in &request.labels {
        if key.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "session file binding label keys must not be empty".to_string(),
            ));
        }
        if value.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "session file binding label values must not be empty".to_string(),
            ));
        }
    }
    Ok(())
}

fn normalize_session_file_mount_path(mount_path: &str) -> Result<String, SessionStoreError> {
    let trimmed = mount_path.trim();
    if trimmed.is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "session file binding mount_path must not be empty".to_string(),
        ));
    }
    let path = Path::new(trimmed);
    if path.is_absolute() {
        return Err(SessionStoreError::InvalidRequest(
            "session file binding mount_path must be relative".to_string(),
        ));
    }

    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => {
                let value = part.to_string_lossy().trim().to_string();
                if value.is_empty() {
                    return Err(SessionStoreError::InvalidRequest(
                        "session file binding mount_path contains an empty component".to_string(),
                    ));
                }
                parts.push(value);
            }
            _ => {
                return Err(SessionStoreError::InvalidRequest(
                    "session file binding mount_path must not contain traversal or non-normal path components"
                        .to_string(),
                ));
            }
        }
    }

    if parts.is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "session file binding mount_path must contain a relative file path".to_string(),
        ));
    }

    Ok(parts.join("/"))
}
