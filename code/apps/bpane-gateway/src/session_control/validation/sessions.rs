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
    if let Some(browser_context) = &request.browser_context {
        match browser_context.mode {
            SessionBrowserContextMode::Fresh | SessionBrowserContextMode::Ephemeral => {
                if browser_context.context_id.is_some() {
                    return Err(SessionStoreError::InvalidRequest(format!(
                        "browser_context.context_id must not be set for {} mode",
                        browser_context.mode.as_str()
                    )));
                }
            }
            SessionBrowserContextMode::Reusable => {
                if browser_context.context_id.is_none() {
                    return Err(SessionStoreError::InvalidRequest(
                        "browser_context.context_id is required for reusable mode".to_string(),
                    ));
                }
            }
        }
    }
    if let Some(network_identity) = &request.network_identity {
        validate_network_identity(network_identity)?;
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

pub(in crate::session_control) fn validate_session_template_request(
    request: &PersistSessionTemplateRequest,
) -> Result<(), SessionStoreError> {
    if request.name.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "session template name must not be empty".to_string(),
        ));
    }
    if let Some(description) = &request.description {
        if description.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "session template description must not be empty when provided".to_string(),
            ));
        }
    }
    validate_label_map(&request.labels, "session template labels")?;
    validate_template_defaults(&request.defaults)?;
    Ok(())
}

fn validate_template_defaults(defaults: &SessionTemplateDefaults) -> Result<(), SessionStoreError> {
    let request = CreateSessionRequest {
        owner_mode: defaults.owner_mode,
        viewport: defaults.viewport.clone(),
        idle_timeout_sec: defaults.idle_timeout_sec,
        labels: defaults.labels.clone(),
        integration_context: defaults.integration_context.clone(),
        network_identity: defaults.network_identity.clone(),
        recording: defaults.recording.clone().unwrap_or_default(),
        ..CreateSessionRequest::default()
    };
    validate_create_request(&request)?;
    validate_label_map(&defaults.labels, "session template default labels")?;
    Ok(())
}

fn validate_network_identity(identity: &SessionNetworkIdentity) -> Result<(), SessionStoreError> {
    if let Some(locale) = &identity.locale {
        validate_locale_tag(locale, "network_identity.locale")?;
    }
    for language in &identity.languages {
        validate_locale_tag(language, "network_identity.languages")?;
    }
    if let Some(timezone) = &identity.timezone {
        validate_timezone(timezone)?;
    }
    if let Some(geolocation) = &identity.geolocation {
        if !(-90.0..=90.0).contains(&geolocation.latitude) {
            return Err(SessionStoreError::InvalidRequest(
                "network_identity.geolocation.latitude must be between -90 and 90".to_string(),
            ));
        }
        if !(-180.0..=180.0).contains(&geolocation.longitude) {
            return Err(SessionStoreError::InvalidRequest(
                "network_identity.geolocation.longitude must be between -180 and 180".to_string(),
            ));
        }
        if let Some(accuracy) = geolocation.accuracy_meters {
            if accuracy <= 0.0 {
                return Err(SessionStoreError::InvalidRequest(
                    "network_identity.geolocation.accuracy_meters must be greater than zero"
                        .to_string(),
                ));
            }
        }
    }
    if let Some(user_agent) = &identity.user_agent {
        if user_agent.trim().is_empty() || user_agent.contains('\r') || user_agent.contains('\n') {
            return Err(SessionStoreError::InvalidRequest(
                "network_identity.user_agent must be non-empty and single-line when provided"
                    .to_string(),
            ));
        }
    }
    if let Some(browser_identity) = &identity.browser_identity {
        if browser_identity.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "network_identity.browser_identity must not be empty when provided".to_string(),
            ));
        }
    }
    if identity.egress_profile_id == Some(Uuid::nil()) {
        return Err(SessionStoreError::InvalidRequest(
            "network_identity.egress_profile_id must not be nil".to_string(),
        ));
    }
    Ok(())
}

fn validate_locale_tag(value: &str, field: &str) -> Result<(), SessionStoreError> {
    if value.trim().is_empty() || value.len() > 64 {
        return Err(SessionStoreError::InvalidRequest(format!(
            "{field} must be non-empty and at most 64 characters"
        )));
    }
    let valid = value
        .split('-')
        .all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_alphanumeric()));
    if !valid {
        return Err(SessionStoreError::InvalidRequest(format!(
            "{field} must be a BCP-47-like tag with alphanumeric subtags"
        )));
    }
    Ok(())
}

fn validate_timezone(value: &str) -> Result<(), SessionStoreError> {
    if value == "UTC" {
        return Ok(());
    }
    if value.trim().is_empty()
        || value.len() > 128
        || value.starts_with('/')
        || value.ends_with('/')
        || value.contains("..")
        || !value.contains('/')
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '_' | '-' | '+'))
    {
        return Err(SessionStoreError::InvalidRequest(
            "network_identity.timezone must be UTC or an IANA-style timezone".to_string(),
        ));
    }
    Ok(())
}

fn validate_label_map(
    labels: &std::collections::HashMap<String, String>,
    context: &str,
) -> Result<(), SessionStoreError> {
    for (key, value) in labels {
        if key.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(format!(
                "{context} must not contain empty keys"
            )));
        }
        if value.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(format!(
                "{context} must not contain empty values"
            )));
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
