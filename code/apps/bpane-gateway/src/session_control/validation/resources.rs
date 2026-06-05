use std::path::{Component, Path};

use super::*;

pub(in crate::session_control) fn validate_browser_context_request(
    request: &PersistBrowserContextRequest,
) -> Result<(), SessionStoreError> {
    if request.id == Some(Uuid::nil()) {
        return Err(SessionStoreError::InvalidRequest(
            "browser context id must not be nil when provided".to_string(),
        ));
    }
    if request.project_id == Some(Uuid::nil()) {
        return Err(SessionStoreError::InvalidRequest(
            "browser context project_id must not be nil when provided".to_string(),
        ));
    }
    if request.name.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "browser context name must not be empty".to_string(),
        ));
    }
    if let Some(description) = &request.description {
        if description.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "browser context description must not be empty when provided".to_string(),
            ));
        }
    }
    for (key, value) in &request.labels {
        if key.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "browser context label keys must not be empty".to_string(),
            ));
        }
        if value.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "browser context label values must not be empty".to_string(),
            ));
        }
    }
    if let Some(retention_sec) = request.retention_sec {
        if retention_sec == 0 {
            return Err(SessionStoreError::InvalidRequest(
                "browser context retention_sec must be greater than zero when provided".to_string(),
            ));
        }
    }
    if let Some(max_profile_storage_bytes) = request.max_profile_storage_bytes {
        if max_profile_storage_bytes == 0 {
            return Err(SessionStoreError::InvalidRequest(
                "browser context max_profile_storage_bytes must be greater than zero when provided"
                    .to_string(),
            ));
        }
        if max_profile_storage_bytes > i64::MAX as u64 {
            return Err(SessionStoreError::InvalidRequest(
                "browser context max_profile_storage_bytes exceeds the storage backend limit"
                    .to_string(),
            ));
        }
    }
    Ok(())
}

pub(in crate::session_control) fn validate_project_request(
    request: &PersistProjectRequest,
) -> Result<(), SessionStoreError> {
    if request.name.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "project name must not be empty".to_string(),
        ));
    }
    if let Some(description) = &request.description {
        if description.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "project description must not be empty when provided".to_string(),
            ));
        }
    }
    for (key, value) in &request.labels {
        if key.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "project label keys must not be empty".to_string(),
            ));
        }
        if value.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "project label values must not be empty".to_string(),
            ));
        }
    }
    if request.quotas.max_active_sessions == Some(0) {
        return Err(SessionStoreError::InvalidRequest(
            "project quotas.max_active_sessions must be greater than zero when provided"
                .to_string(),
        ));
    }
    if request.quotas.max_active_workflow_runs == Some(0) {
        return Err(SessionStoreError::InvalidRequest(
            "project quotas.max_active_workflow_runs must be greater than zero when provided"
                .to_string(),
        ));
    }
    if request.quotas.max_retained_storage_bytes == Some(0) {
        return Err(SessionStoreError::InvalidRequest(
            "project quotas.max_retained_storage_bytes must be greater than zero when provided"
                .to_string(),
        ));
    }
    if request.quotas.max_session_creations == Some(0) {
        return Err(SessionStoreError::InvalidRequest(
            "project quotas.max_session_creations must be greater than zero when provided"
                .to_string(),
        ));
    }
    match (
        request.quotas.max_session_creations_per_window,
        request.quotas.session_creation_window_sec,
    ) {
        (Some(0), _) => {
            return Err(SessionStoreError::InvalidRequest(
                "project quotas.max_session_creations_per_window must be greater than zero when provided"
                    .to_string(),
            ));
        }
        (_, Some(0)) => {
            return Err(SessionStoreError::InvalidRequest(
                "project quotas.session_creation_window_sec must be greater than zero when provided"
                    .to_string(),
            ));
        }
        (Some(_), Some(_)) | (None, None) => {}
        (Some(_), None) | (None, Some(_)) => {
            return Err(SessionStoreError::InvalidRequest(
                "project quotas.max_session_creations_per_window and session_creation_window_sec must be provided together"
                    .to_string(),
            ));
        }
    }
    if request.quotas.max_runtime_usage_ms == Some(0) {
        return Err(SessionStoreError::InvalidRequest(
            "project quotas.max_runtime_usage_ms must be greater than zero when provided"
                .to_string(),
        ));
    }
    if request.quotas.max_egress_total_bytes == Some(0) {
        return Err(SessionStoreError::InvalidRequest(
            "project quotas.max_egress_total_bytes must be greater than zero when provided"
                .to_string(),
        ));
    }
    if request
        .policy
        .allowed_session_template_ids
        .iter()
        .any(|template_id| template_id.trim().is_empty())
    {
        return Err(SessionStoreError::InvalidRequest(
            "project policy.allowed_session_template_ids must not contain empty values".to_string(),
        ));
    }
    if request
        .policy
        .allowed_egress_profile_ids
        .iter()
        .any(|profile_id| *profile_id == Uuid::nil())
    {
        return Err(SessionStoreError::InvalidRequest(
            "project policy.allowed_egress_profile_ids must not contain nil UUIDs".to_string(),
        ));
    }
    if request
        .policy
        .allowed_extension_ids
        .iter()
        .any(|extension_id| *extension_id == Uuid::nil())
    {
        return Err(SessionStoreError::InvalidRequest(
            "project policy.allowed_extension_ids must not contain nil UUIDs".to_string(),
        ));
    }
    if request
        .policy
        .allowed_browser_context_ids
        .iter()
        .any(|context_id| *context_id == Uuid::nil())
    {
        return Err(SessionStoreError::InvalidRequest(
            "project policy.allowed_browser_context_ids must not contain nil UUIDs".to_string(),
        ));
    }
    Ok(())
}

pub(in crate::session_control) fn validate_service_principal_request(
    request: &PersistServicePrincipalRequest,
) -> Result<(), SessionStoreError> {
    if request.name.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "service principal name must not be empty".to_string(),
        ));
    }
    if request.client_id.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "service principal client_id must not be empty".to_string(),
        ));
    }
    if request.issuer.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "service principal issuer must not be empty".to_string(),
        ));
    }
    if let Some(description) = &request.description {
        if description.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "service principal description must not be empty when provided".to_string(),
            ));
        }
    }
    for (key, value) in &request.labels {
        if key.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "service principal label keys must not be empty".to_string(),
            ));
        }
        if value.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "service principal label values must not be empty".to_string(),
            ));
        }
    }
    for scope in &request.scopes {
        if scope.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "service principal scopes must not contain empty values".to_string(),
            ));
        }
    }
    if request
        .allowed_project_ids
        .iter()
        .any(|project_id| *project_id == Uuid::nil())
    {
        return Err(SessionStoreError::InvalidRequest(
            "service principal allowed_project_ids must not contain nil UUIDs".to_string(),
        ));
    }
    Ok(())
}

pub(in crate::session_control) fn validate_identity_mapping_request(
    request: &PersistIdentityMappingRequest,
) -> Result<(), SessionStoreError> {
    if request.name.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "identity mapping name must not be empty".to_string(),
        ));
    }
    if request.issuer.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "identity mapping issuer must not be empty".to_string(),
        ));
    }
    if request.external_id.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "identity mapping external_id must not be empty".to_string(),
        ));
    }
    if request.project_id == Uuid::nil() {
        return Err(SessionStoreError::InvalidRequest(
            "identity mapping project_id must not be nil".to_string(),
        ));
    }
    if let Some(description) = &request.description {
        if description.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "identity mapping description must not be empty when provided".to_string(),
            ));
        }
    }
    for (key, value) in &request.labels {
        if key.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "identity mapping label keys must not be empty".to_string(),
            ));
        }
        if value.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "identity mapping label values must not be empty".to_string(),
            ));
        }
    }
    for scope in &request.scopes {
        if scope.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "identity mapping scopes must not contain empty values".to_string(),
            ));
        }
    }
    match request.kind {
        IdentityMappingKind::Claim => {
            if request
                .claim_name
                .as_ref()
                .is_none_or(|claim_name| claim_name.trim().is_empty())
            {
                return Err(SessionStoreError::InvalidRequest(
                    "identity mapping claim_name is required for claim mappings".to_string(),
                ));
            }
            if request.service_principal_id.is_some() {
                return Err(SessionStoreError::InvalidRequest(
                    "identity mapping service_principal_id is not valid for claim mappings"
                        .to_string(),
                ));
            }
        }
        IdentityMappingKind::ServicePrincipal => {
            if request.service_principal_id.is_none() {
                return Err(SessionStoreError::InvalidRequest(
                    "identity mapping service_principal_id is required for service_principal mappings"
                        .to_string(),
                ));
            }
            if request.claim_name.is_some() {
                return Err(SessionStoreError::InvalidRequest(
                    "identity mapping claim_name is not valid for service_principal mappings"
                        .to_string(),
                ));
            }
        }
        IdentityMappingKind::User | IdentityMappingKind::Group => {
            if request.claim_name.is_some() {
                return Err(SessionStoreError::InvalidRequest(
                    "identity mapping claim_name is only valid for claim mappings".to_string(),
                ));
            }
            if request.service_principal_id.is_some() {
                return Err(SessionStoreError::InvalidRequest(
                    "identity mapping service_principal_id is only valid for service_principal mappings"
                        .to_string(),
                ));
            }
        }
    }
    if request.service_principal_id == Some(Uuid::nil()) {
        return Err(SessionStoreError::InvalidRequest(
            "identity mapping service_principal_id must not be nil when provided".to_string(),
        ));
    }
    Ok(())
}

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

pub(in crate::session_control) fn validate_egress_profile_request(
    request: &PersistEgressProfileRequest,
) -> Result<(), SessionStoreError> {
    if request.name.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "egress profile name must not be empty".to_string(),
        ));
    }
    if let Some(description) = &request.description {
        if description.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "egress profile description must not be empty when provided".to_string(),
            ));
        }
    }
    for (key, value) in &request.labels {
        if key.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "egress profile label keys must not be empty".to_string(),
            ));
        }
        if value.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "egress profile label values must not be empty".to_string(),
            ));
        }
    }
    if let Some(proxy) = &request.proxy {
        validate_egress_proxy(proxy)?;
    }
    for bypass_rule in &request.bypass_rules {
        if bypass_rule.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "egress profile bypass_rules must not contain empty values".to_string(),
            ));
        }
    }
    if let Some(custom_ca) = &request.custom_ca {
        if custom_ca.certificate_ref.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "egress profile custom_ca.certificate_ref must not be empty".to_string(),
            ));
        }
        if let Some(display_name) = &custom_ca.display_name {
            if display_name.trim().is_empty() {
                return Err(SessionStoreError::InvalidRequest(
                    "egress profile custom_ca.display_name must not be empty when provided"
                        .to_string(),
                ));
            }
        }
    }
    validate_egress_traffic_observation(request)?;
    Ok(())
}

fn validate_egress_traffic_observation(
    request: &PersistEgressProfileRequest,
) -> Result<(), SessionStoreError> {
    let observation = &request.traffic_observation;
    if let Some(sink_ref) = &observation.sensitive_log_sink_ref {
        if sink_ref.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "egress profile traffic_observation.sensitive_log_sink_ref must not be empty when provided".to_string(),
            ));
        }
        if sink_ref.contains(['\r', '\n']) {
            return Err(SessionStoreError::InvalidRequest(
                "egress profile traffic_observation.sensitive_log_sink_ref must be a single line"
                    .to_string(),
            ));
        }
        if reference_contains_inline_credentials(sink_ref) {
            return Err(SessionStoreError::InvalidRequest(
                "egress profile traffic_observation.sensitive_log_sink_ref must not contain inline credentials".to_string(),
            ));
        }
    }
    if let Some(display_name) = &observation.sensitive_log_sink_display_name {
        if display_name.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "egress profile traffic_observation.sensitive_log_sink_display_name must not be empty when provided".to_string(),
            ));
        }
    }
    if observation.mode == EgressTrafficObservationMode::TlsIntercept {
        if request.proxy.is_none() {
            return Err(SessionStoreError::InvalidRequest(
                "egress profile traffic_observation.mode=tls_intercept requires proxy".to_string(),
            ));
        }
        if request.custom_ca.is_none() {
            return Err(SessionStoreError::InvalidRequest(
                "egress profile traffic_observation.mode=tls_intercept requires custom_ca"
                    .to_string(),
            ));
        }
        if observation
            .sensitive_log_sink_ref
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
        {
            return Err(SessionStoreError::InvalidRequest(
                "egress profile traffic_observation.mode=tls_intercept requires sensitive_log_sink_ref".to_string(),
            ));
        }
    }
    Ok(())
}

fn validate_egress_proxy(proxy: &EgressProxyConfig) -> Result<(), SessionStoreError> {
    let url = proxy.url.trim();
    if url.is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "egress profile proxy.url must not be empty".to_string(),
        ));
    }
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(SessionStoreError::InvalidRequest(
            "egress profile proxy.url must start with http:// or https://".to_string(),
        ));
    }
    let Some(authority_and_path) = url.split_once("://").map(|(_, rest)| rest) else {
        return Err(SessionStoreError::InvalidRequest(
            "egress profile proxy.url must include an authority".to_string(),
        ));
    };
    let authority = authority_and_path
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    if authority.is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "egress profile proxy.url must include an authority".to_string(),
        ));
    }
    if authority.contains('@') {
        return Err(SessionStoreError::InvalidRequest(
            "egress profile proxy.url must not contain inline credentials".to_string(),
        ));
    }
    Ok(())
}

fn reference_contains_inline_credentials(value: &str) -> bool {
    value
        .split_once("://")
        .and_then(|(_, rest)| rest.split(['/', '?', '#']).next())
        .is_some_and(|authority| authority.contains('@'))
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
    if request.project_id == Some(Uuid::nil()) {
        return Err(SessionStoreError::InvalidRequest(
            "file workspace project_id must not be nil when provided".to_string(),
        ));
    }
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

pub(in crate::session_control) fn validate_session_file_request(
    request: &PersistSessionFileRequest,
) -> Result<(), SessionStoreError> {
    if request.name.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "session file name must not be empty".to_string(),
        ));
    }
    if let Some(media_type) = &request.media_type {
        if media_type.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "session file media_type must not be empty when provided".to_string(),
            ));
        }
    }
    if request.sha256_hex.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "session file sha256_hex must not be empty".to_string(),
        ));
    }
    if request.artifact_ref.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "session file artifact_ref must not be empty".to_string(),
        ));
    }
    for (key, value) in &request.labels {
        if key.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "session file label keys must not be empty".to_string(),
            ));
        }
        if value.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "session file label values must not be empty".to_string(),
            ));
        }
    }
    Ok(())
}
