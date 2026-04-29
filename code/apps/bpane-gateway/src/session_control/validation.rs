use std::collections::HashSet;
use std::path::Path;

use super::*;

pub(super) fn validate_create_request(
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

pub(super) fn validate_automation_delegate_request(
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

pub(super) fn validate_persist_automation_task_request(
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

pub(super) fn validate_automation_task_transition_request(
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

pub(super) fn validate_automation_task_log_message(message: &str) -> Result<(), SessionStoreError> {
    if message.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "automation task log message must not be empty".to_string(),
        ));
    }
    Ok(())
}

pub(super) fn validate_workflow_definition_request(
    request: &PersistWorkflowDefinitionRequest,
) -> Result<(), SessionStoreError> {
    if request.name.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "workflow name must not be empty".to_string(),
        ));
    }
    if let Some(description) = &request.description {
        if description.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "workflow description must not be empty when provided".to_string(),
            ));
        }
    }
    for label in &request.labels {
        if label.0.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "workflow label keys must not be empty".to_string(),
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_workflow_definition_version_request(
    request: &PersistWorkflowDefinitionVersionRequest,
) -> Result<(), SessionStoreError> {
    if request.version.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "workflow version must not be empty".to_string(),
        ));
    }
    if request.executor.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "workflow executor must not be empty".to_string(),
        ));
    }
    if request.entrypoint.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "workflow entrypoint must not be empty".to_string(),
        ));
    }
    if let Some(source) = &request.source {
        match source {
            WorkflowSource::Git(source) => {
                if source.repository_url.trim().is_empty() {
                    return Err(SessionStoreError::InvalidRequest(
                        "workflow git source repository_url must not be empty".to_string(),
                    ));
                }
                if source
                    .r#ref
                    .as_deref()
                    .is_some_and(|value| value.trim().is_empty())
                {
                    return Err(SessionStoreError::InvalidRequest(
                        "workflow git source ref must not be empty when provided".to_string(),
                    ));
                }
                if let Some(commit) = source.resolved_commit.as_deref() {
                    if commit.len() != 40 || !commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                        return Err(SessionStoreError::InvalidRequest(
                            "workflow git source resolved_commit must be a 40-character hex sha"
                                .to_string(),
                        ));
                    }
                }
                if source
                    .root_path
                    .as_deref()
                    .is_some_and(|value| value.trim().is_empty())
                {
                    return Err(SessionStoreError::InvalidRequest(
                        "workflow git source root_path must not be empty when provided".to_string(),
                    ));
                }
            }
        }
    }
    if let Some(default_session) = &request.default_session {
        serde_json::from_value::<CreateSessionRequest>(default_session.clone()).map_err(
            |error| {
                SessionStoreError::InvalidRequest(format!(
                    "default_session must be a valid session create payload: {error}"
                ))
            },
        )?;
    }
    for value in [
        &request.allowed_credential_binding_ids,
        &request.allowed_extension_ids,
        &request.allowed_file_workspace_ids,
    ] {
        for entry in value {
            if entry.trim().is_empty() {
                return Err(SessionStoreError::InvalidRequest(
                    "workflow allowed reference ids must not be empty".to_string(),
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn validate_workflow_run_request(
    request: &PersistWorkflowRunRequest,
) -> Result<(), SessionStoreError> {
    if request.workflow_version.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "workflow_version must not be empty".to_string(),
        ));
    }
    if let Some(source_system) = &request.source_system {
        if source_system.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "workflow run source_system must not be empty".to_string(),
            ));
        }
    }
    if let Some(source_reference) = &request.source_reference {
        if source_reference.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "workflow run source_reference must not be empty".to_string(),
            ));
        }
    }
    if let Some(client_request_id) = &request.client_request_id {
        if client_request_id.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "workflow run client_request_id must not be empty".to_string(),
            ));
        }
    }
    if let Some(create_request_fingerprint) = &request.create_request_fingerprint {
        if create_request_fingerprint.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "workflow run create_request_fingerprint must not be empty".to_string(),
            ));
        }
    }
    for label in &request.labels {
        if label.0.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "workflow run label keys must not be empty".to_string(),
            ));
        }
    }
    if let Some(source_snapshot) = &request.source_snapshot {
        if source_snapshot.entrypoint.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "workflow run source snapshot entrypoint must not be empty".to_string(),
            ));
        }
        if source_snapshot.file_name.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "workflow run source snapshot file_name must not be empty".to_string(),
            ));
        }
    }
    for credential_binding in &request.credential_bindings {
        if credential_binding.name.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "workflow run credential binding name must not be empty".to_string(),
            ));
        }
        if credential_binding.external_ref.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "workflow run credential binding external_ref must not be empty".to_string(),
            ));
        }
        for origin in &credential_binding.allowed_origins {
            if origin.trim().is_empty() {
                return Err(SessionStoreError::InvalidRequest(
                    "workflow run credential binding allowed_origins must not contain empty values"
                        .to_string(),
                ));
            }
        }
    }
    for workspace_input in &request.workspace_inputs {
        if workspace_input.file_name.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "workflow run workspace input file_name must not be empty".to_string(),
            ));
        }
        if workspace_input.sha256_hex.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "workflow run workspace input sha256_hex must not be empty".to_string(),
            ));
        }
        if workspace_input.mount_path.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "workflow run workspace input mount_path must not be empty".to_string(),
            ));
        }
        if workspace_input.artifact_ref.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "workflow run workspace input artifact_ref must not be empty".to_string(),
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_workflow_run_event_request(
    request: &PersistWorkflowRunEventRequest,
) -> Result<(), SessionStoreError> {
    if request.event_type.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "workflow run event_type must not be empty".to_string(),
        ));
    }
    if request.message.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "workflow run event message must not be empty".to_string(),
        ));
    }
    Ok(())
}

pub(super) fn validate_workflow_run_transition_request(
    request: &WorkflowRunTransitionRequest,
) -> Result<(), SessionStoreError> {
    let task_request = AutomationTaskTransitionRequest {
        state: request.state.into(),
        output: request.output.clone(),
        error: request.error.clone(),
        artifact_refs: request.artifact_refs.clone(),
        event_type: "workflow_run.transition".to_string(),
        event_message: request
            .message
            .clone()
            .unwrap_or_else(|| "workflow run transition".to_string()),
        event_data: request.data.clone(),
    };
    validate_automation_task_transition_request(&task_request).and_then(|_| {
        match (&request.state, request.data.as_ref()) {
            (WorkflowRunState::AwaitingInput, Some(data)) => {
                crate::workflow::parse_workflow_run_runtime_hold_request(data)
                    .map(|_| ())
                    .map_err(|error| SessionStoreError::InvalidRequest(error.to_string()))
            }
            (_, Some(data))
                if data
                    .as_object()
                    .and_then(|value| value.get("runtime_hold"))
                    .is_some() =>
            {
                Err(SessionStoreError::InvalidRequest(
                    "workflow runtime_hold is only valid when transitioning to awaiting_input"
                        .to_string(),
                ))
            }
            _ => Ok(()),
        }
    })
}

pub(super) fn validate_workflow_run_log_request(
    request: &PersistWorkflowRunLogRequest,
) -> Result<(), SessionStoreError> {
    validate_automation_task_log_message(&request.message)
}

pub(super) fn validate_workflow_run_produced_file_request(
    request: &PersistWorkflowRunProducedFileRequest,
) -> Result<(), SessionStoreError> {
    if request.file_name.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "workflow run produced file_name must not be empty".to_string(),
        ));
    }
    if request.sha256_hex.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "workflow run produced file sha256_hex must not be empty".to_string(),
        ));
    }
    if request.artifact_ref.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "workflow run produced file artifact_ref must not be empty".to_string(),
        ));
    }
    Ok(())
}

pub(super) fn validate_credential_binding_request(
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

pub(super) fn validate_extension_definition_request(
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

pub(super) fn validate_extension_version_request(
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

pub(super) fn validate_file_workspace_request(
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

pub(super) fn validate_file_workspace_file_request(
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

pub(super) fn validate_persist_completed_recording_request(
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

pub(super) fn validate_fail_recording_request(
    request: &FailSessionRecordingRequest,
) -> Result<(), SessionStoreError> {
    if request.error.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "error must not be empty".to_string(),
        ));
    }
    Ok(())
}
