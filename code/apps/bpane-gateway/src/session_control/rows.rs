use super::*;

pub(super) fn json_labels(labels: &HashMap<String, String>) -> Value {
    let mut object = JsonMap::new();
    for (key, value) in labels {
        object.insert(key.clone(), Value::String(value.clone()));
    }
    Value::Object(object)
}

pub(super) fn json_workflow_source(
    source: Option<&WorkflowSource>,
) -> Result<Option<Value>, SessionStoreError> {
    source
        .map(|source| {
            serde_json::to_value(source).map_err(|error| {
                SessionStoreError::Backend(format!("failed to encode workflow source: {error}"))
            })
        })
        .transpose()
}

pub(super) fn json_workflow_run_source_snapshot(
    source_snapshot: Option<&WorkflowRunSourceSnapshot>,
) -> Result<Option<Value>, SessionStoreError> {
    source_snapshot
        .map(|source_snapshot| {
            serde_json::to_value(source_snapshot).map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to encode workflow run source snapshot: {error}"
                ))
            })
        })
        .transpose()
}

pub(super) fn json_applied_extensions(
    extensions: &[AppliedExtension],
) -> Result<Value, SessionStoreError> {
    serde_json::to_value(extensions).map_err(|error| {
        SessionStoreError::Backend(format!("failed to encode applied extensions: {error}"))
    })
}

pub(super) fn json_workflow_run_credential_bindings(
    credential_bindings: &[WorkflowRunCredentialBinding],
) -> Result<Value, SessionStoreError> {
    serde_json::to_value(credential_bindings).map_err(|error| {
        SessionStoreError::Backend(format!(
            "failed to encode workflow run credential_bindings: {error}"
        ))
    })
}

pub(super) fn json_workflow_run_workspace_inputs(
    workspace_inputs: &[WorkflowRunWorkspaceInput],
) -> Result<Value, SessionStoreError> {
    serde_json::to_value(workspace_inputs).map_err(|error| {
        SessionStoreError::Backend(format!(
            "failed to encode workflow run workspace_inputs: {error}"
        ))
    })
}

pub(super) fn json_workflow_run_produced_files(
    produced_files: &[WorkflowRunProducedFile],
) -> Result<Value, SessionStoreError> {
    serde_json::to_value(produced_files).map_err(|error| {
        SessionStoreError::Backend(format!(
            "failed to encode workflow run produced_files: {error}"
        ))
    })
}

pub(super) fn describe_postgres_error(error: &tokio_postgres::Error) -> String {
    if let Some(db_error) = error.as_db_error() {
        let mut message = db_error.message().to_string();
        if let Some(detail) = db_error.detail() {
            message.push_str(&format!("; detail: {detail}"));
        }
        if let Some(hint) = db_error.hint() {
            message.push_str(&format!("; hint: {hint}"));
        }
        return message;
    }
    error.to_string()
}

pub(super) fn sync_workflow_run_with_task(
    run: &mut StoredWorkflowRun,
    task: &StoredAutomationTask,
) {
    run.state = WorkflowRunState::from(task.state);
    run.output = task.output.clone();
    run.error = task.error.clone();
    run.artifact_refs = task.artifact_refs.clone();
    run.started_at = task.started_at;
    run.completed_at = task.completed_at;
    run.updated_at = std::cmp::max(run.updated_at, task.updated_at);
}

pub(super) fn json_string_array(values: &[String]) -> Value {
    Value::Array(
        values
            .iter()
            .cloned()
            .map(Value::String)
            .collect::<Vec<_>>(),
    )
}

pub(super) fn row_to_json_string_array(
    value: Value,
    field_name: &str,
) -> Result<Vec<String>, SessionStoreError> {
    value
        .as_array()
        .with_context(|| format!("{field_name} column must be a JSON array"))
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?
        .iter()
        .map(|entry| {
            entry
                .as_str()
                .with_context(|| format!("{field_name} entries must be strings"))
                .map(|entry| entry.to_string())
                .map_err(|error| SessionStoreError::Backend(error.to_string()))
        })
        .collect()
}

pub(super) fn recording_mime_type(format: SessionRecordingFormat) -> &'static str {
    match format {
        SessionRecordingFormat::Webm => "video/webm",
    }
}

pub(super) fn json_recording_policy(
    recording: &SessionRecordingPolicy,
) -> Result<Value, SessionStoreError> {
    serde_json::to_value(recording).map_err(|error| {
        SessionStoreError::Backend(format!("failed to encode recording policy: {error}"))
    })
}

pub(super) fn row_to_stored_session(row: &Row) -> Result<StoredSession, SessionStoreError> {
    let state = row
        .get::<_, String>("state")
        .parse::<SessionLifecycleState>()
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?;
    let owner_mode = row
        .get::<_, String>("owner_mode")
        .parse::<SessionOwnerMode>()
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?;
    let labels_value: Value = row.get("labels");
    let labels = labels_value
        .as_object()
        .context("labels column must be a JSON object")
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?
        .iter()
        .map(|(key, value)| {
            Ok((
                key.clone(),
                value
                    .as_str()
                    .context("label values must be strings")
                    .map_err(|error| SessionStoreError::Backend(error.to_string()))?
                    .to_string(),
            ))
        })
        .collect::<Result<HashMap<_, _>, SessionStoreError>>()?;
    let recording = serde_json::from_value::<SessionRecordingPolicy>(row.get("recording"))
        .map_err(|error| {
            SessionStoreError::Backend(format!("failed to decode recording policy: {error}"))
        })?;
    let extensions = serde_json::from_value::<Vec<AppliedExtension>>(row.get("extensions"))
        .map_err(|error| {
            SessionStoreError::Backend(format!("failed to decode session extensions: {error}"))
        })?;

    let width = row.get::<_, i32>("viewport_width");
    let height = row.get::<_, i32>("viewport_height");
    let automation_owner_client_id = row.get::<_, Option<String>>("automation_owner_client_id");
    let automation_owner_issuer = row.get::<_, Option<String>>("automation_owner_issuer");

    Ok(StoredSession {
        id: row.get("id"),
        state,
        template_id: row.get("template_id"),
        owner_mode,
        viewport: SessionViewport {
            width: width as u16,
            height: height as u16,
        },
        owner: SessionOwner {
            subject: row.get("owner_subject"),
            issuer: row.get("owner_issuer"),
            display_name: row.get("owner_display_name"),
        },
        automation_delegate: match (automation_owner_client_id, automation_owner_issuer) {
            (Some(client_id), Some(issuer)) => Some(SessionAutomationDelegate {
                client_id,
                issuer,
                display_name: row.get("automation_owner_display_name"),
            }),
            _ => None,
        },
        idle_timeout_sec: row
            .get::<_, Option<i32>>("idle_timeout_sec")
            .map(|value| value as u32),
        labels,
        integration_context: row.get("integration_context"),
        extensions,
        recording,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        stopped_at: row.get("stopped_at"),
    })
}

pub(super) fn row_to_stored_session_recording(
    row: &Row,
) -> Result<StoredSessionRecording, SessionStoreError> {
    let state = row
        .get::<_, String>("state")
        .parse::<SessionRecordingState>()
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?;
    let format = row
        .get::<_, String>("format")
        .parse::<SessionRecordingFormat>()
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?;
    let termination_reason = row
        .get::<_, Option<String>>("termination_reason")
        .map(|value| {
            value
                .parse::<SessionRecordingTerminationReason>()
                .map_err(|error| SessionStoreError::Backend(error.to_string()))
        })
        .transpose()?;

    Ok(StoredSessionRecording {
        id: row.get("id"),
        session_id: row.get("session_id"),
        previous_recording_id: row.get("previous_recording_id"),
        state,
        format,
        mime_type: row.get("mime_type"),
        bytes: row
            .get::<_, Option<i64>>("byte_count")
            .map(|value| value as u64),
        duration_ms: row
            .get::<_, Option<i64>>("duration_ms")
            .map(|value| value as u64),
        error: row.get("error"),
        termination_reason,
        artifact_ref: row.get("artifact_ref"),
        started_at: row.get("started_at"),
        completed_at: row.get("completed_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub(super) fn row_to_stored_automation_task(
    row: &Row,
) -> Result<StoredAutomationTask, SessionStoreError> {
    let state = row
        .get::<_, String>("state")
        .parse::<AutomationTaskState>()
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?;
    let session_source = row
        .get::<_, String>("session_source")
        .parse::<AutomationTaskSessionSource>()
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?;
    let labels_value: Value = row.get("labels");
    let labels = labels_value
        .as_object()
        .context("automation task labels column must be a JSON object")
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?
        .iter()
        .map(|(key, value)| {
            Ok((
                key.clone(),
                value
                    .as_str()
                    .context("automation task label values must be strings")
                    .map_err(|error| SessionStoreError::Backend(error.to_string()))?
                    .to_string(),
            ))
        })
        .collect::<Result<HashMap<_, _>, SessionStoreError>>()?;
    let artifact_refs_value: Value = row.get("artifact_refs");
    let artifact_refs = artifact_refs_value
        .as_array()
        .context("automation task artifact_refs column must be a JSON array")
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .context("automation task artifact_refs entries must be strings")
                .map(|entry| entry.to_string())
                .map_err(|error| SessionStoreError::Backend(error.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(StoredAutomationTask {
        id: row.get("id"),
        display_name: row.get("display_name"),
        executor: row.get("executor"),
        state,
        session_id: row.get("session_id"),
        session_source,
        input: row.get("input"),
        output: row.get("output"),
        error: row.get("error"),
        artifact_refs,
        labels,
        cancel_requested_at: row.get("cancel_requested_at"),
        started_at: row.get("started_at"),
        completed_at: row.get("completed_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub(super) fn row_to_stored_automation_task_event(
    row: &Row,
) -> Result<StoredAutomationTaskEvent, SessionStoreError> {
    Ok(StoredAutomationTaskEvent {
        id: row.get("id"),
        task_id: row.get("task_id"),
        event_type: row.get("event_type"),
        message: row.get("message"),
        data: row.get("data"),
        created_at: row.get("created_at"),
    })
}

pub(super) fn row_to_stored_automation_task_log(
    row: &Row,
) -> Result<StoredAutomationTaskLog, SessionStoreError> {
    let stream = row
        .get::<_, String>("stream")
        .parse::<AutomationTaskLogStream>()
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?;
    Ok(StoredAutomationTaskLog {
        id: row.get("id"),
        task_id: row.get("task_id"),
        stream,
        message: row.get("message"),
        created_at: row.get("created_at"),
    })
}

pub(super) fn row_to_stored_credential_binding(
    row: &Row,
) -> Result<StoredCredentialBinding, SessionStoreError> {
    let provider = row
        .get::<_, String>("provider")
        .parse::<CredentialBindingProvider>()
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?;
    let injection_mode = row
        .get::<_, String>("injection_mode")
        .parse::<CredentialInjectionMode>()
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?;
    let labels_value: Value = row.get("labels");
    let labels = labels_value
        .as_object()
        .context("credential binding labels column must be a JSON object")
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?
        .iter()
        .map(|(key, value)| {
            Ok((
                key.clone(),
                value
                    .as_str()
                    .context("credential binding label values must be strings")
                    .map_err(|error| SessionStoreError::Backend(error.to_string()))?
                    .to_string(),
            ))
        })
        .collect::<Result<HashMap<_, _>, SessionStoreError>>()?;
    let allowed_origins = row_to_json_string_array(
        row.get("allowed_origins"),
        "credential binding allowed_origins",
    )?;
    let totp = row
        .get::<_, Option<Value>>("totp")
        .map(|value| {
            serde_json::from_value::<CredentialTotpMetadata>(value).map_err(|error| {
                SessionStoreError::Backend(format!(
                    "credential binding totp column must be valid totp json: {error}"
                ))
            })
        })
        .transpose()?;

    Ok(StoredCredentialBinding {
        id: row.get("id"),
        owner_subject: row.get("owner_subject"),
        owner_issuer: row.get("owner_issuer"),
        name: row.get("name"),
        provider,
        external_ref: row.get("external_ref"),
        namespace: row.get("namespace"),
        allowed_origins,
        injection_mode,
        totp,
        labels,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub(super) fn row_to_stored_extension_definition(
    row: &Row,
) -> Result<StoredExtensionDefinition, SessionStoreError> {
    let labels_value: Value = row.get("labels");
    let labels = labels_value
        .as_object()
        .context("extension labels column must be a JSON object")
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?
        .iter()
        .map(|(key, value)| {
            Ok((
                key.clone(),
                value
                    .as_str()
                    .context("extension label values must be strings")
                    .map_err(|error| SessionStoreError::Backend(error.to_string()))?
                    .to_string(),
            ))
        })
        .collect::<Result<HashMap<_, _>, SessionStoreError>>()?;

    Ok(StoredExtensionDefinition {
        id: row.get("id"),
        owner_subject: row.get("owner_subject"),
        owner_issuer: row.get("owner_issuer"),
        name: row.get("name"),
        description: row.get("description"),
        enabled: row.get("enabled"),
        latest_version: row.get("latest_version"),
        labels,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub(super) fn row_to_stored_extension_version(
    row: &Row,
) -> Result<StoredExtensionVersion, SessionStoreError> {
    Ok(StoredExtensionVersion {
        id: row.get("id"),
        extension_definition_id: row.get("extension_definition_id"),
        version: row.get("version"),
        install_path: row.get("install_path"),
        created_at: row.get("created_at"),
    })
}

pub(super) fn row_to_stored_workflow_definition(
    row: &Row,
) -> Result<StoredWorkflowDefinition, SessionStoreError> {
    let labels_value: Value = row.get("labels");
    let labels = labels_value
        .as_object()
        .context("workflow definition labels column must be a JSON object")
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?
        .iter()
        .map(|(key, value)| {
            Ok((
                key.clone(),
                value
                    .as_str()
                    .context("workflow definition label values must be strings")
                    .map_err(|error| SessionStoreError::Backend(error.to_string()))?
                    .to_string(),
            ))
        })
        .collect::<Result<HashMap<_, _>, SessionStoreError>>()?;

    Ok(StoredWorkflowDefinition {
        id: row.get("id"),
        owner_subject: row.get("owner_subject"),
        owner_issuer: row.get("owner_issuer"),
        name: row.get("name"),
        description: row.get("description"),
        labels,
        latest_version: row.get("latest_version"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub(super) fn row_to_stored_workflow_definition_version(
    row: &Row,
) -> Result<StoredWorkflowDefinitionVersion, SessionStoreError> {
    let source = row
        .get::<_, Option<Value>>("source")
        .map(|value| {
            serde_json::from_value::<WorkflowSource>(value).map_err(|error| {
                SessionStoreError::Backend(format!(
                    "workflow definition version source must be valid workflow source json: {error}"
                ))
            })
        })
        .transpose()?;
    let allowed_credential_binding_ids = row_to_json_string_array(
        row.get("allowed_credential_binding_ids"),
        "workflow allowed_credential_binding_ids",
    )?;
    let allowed_extension_ids = row_to_json_string_array(
        row.get("allowed_extension_ids"),
        "workflow allowed_extension_ids",
    )?;
    let allowed_file_workspace_ids = row_to_json_string_array(
        row.get("allowed_file_workspace_ids"),
        "workflow allowed_file_workspace_ids",
    )?;

    Ok(StoredWorkflowDefinitionVersion {
        id: row.get("id"),
        workflow_definition_id: row.get("workflow_definition_id"),
        version: row.get("version"),
        executor: row.get("executor"),
        entrypoint: row.get("entrypoint"),
        source,
        input_schema: row.get("input_schema"),
        output_schema: row.get("output_schema"),
        default_session: row.get("default_session"),
        allowed_credential_binding_ids,
        allowed_extension_ids,
        allowed_file_workspace_ids,
        created_at: row.get("created_at"),
    })
}

pub(super) fn row_to_stored_file_workspace(
    row: &Row,
) -> Result<StoredFileWorkspace, SessionStoreError> {
    let labels_value: Value = row.get("labels");
    let labels = labels_value
        .as_object()
        .context("file workspace labels column must be a JSON object")
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?
        .iter()
        .map(|(key, value)| {
            Ok((
                key.clone(),
                value
                    .as_str()
                    .context("file workspace label values must be strings")
                    .map_err(|error| SessionStoreError::Backend(error.to_string()))?
                    .to_string(),
            ))
        })
        .collect::<Result<HashMap<_, _>, SessionStoreError>>()?;

    Ok(StoredFileWorkspace {
        id: row.get("id"),
        owner_subject: row.get("owner_subject"),
        owner_issuer: row.get("owner_issuer"),
        name: row.get("name"),
        description: row.get("description"),
        labels,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub(super) fn row_to_stored_file_workspace_file(
    row: &Row,
) -> Result<StoredFileWorkspaceFile, SessionStoreError> {
    let byte_count = u64::try_from(row.get::<_, i64>("byte_count")).map_err(|error| {
        SessionStoreError::Backend(format!(
            "workspace file byte_count must be non-negative and fit u64: {error}"
        ))
    })?;
    let provenance: Option<Value> = row.get("provenance");
    if provenance.as_ref().is_some_and(|value| !value.is_object()) {
        return Err(SessionStoreError::Backend(
            "workspace file provenance column must be a JSON object".to_string(),
        ));
    }

    Ok(StoredFileWorkspaceFile {
        id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        name: row.get("name"),
        media_type: row.get("media_type"),
        byte_count,
        sha256_hex: row.get("sha256_hex"),
        provenance,
        artifact_ref: row.get("artifact_ref"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub(super) fn row_to_stored_workflow_run(
    row: &Row,
) -> Result<StoredWorkflowRun, SessionStoreError> {
    let state = row
        .get::<_, String>("state")
        .parse::<WorkflowRunState>()
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?;
    let source_snapshot = row
        .get::<_, Option<Value>>("source_snapshot")
        .map(|value| {
            serde_json::from_value::<WorkflowRunSourceSnapshot>(value).map_err(|error| {
                SessionStoreError::Backend(format!(
                    "workflow run source_snapshot column must be a valid source snapshot: {error}"
                ))
            })
        })
        .transpose()?;
    let extensions_value: Value = row.get("extensions");
    extensions_value
        .as_array()
        .context("workflow run extensions column must be a JSON array")
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?;
    let extensions =
        serde_json::from_value::<Vec<AppliedExtension>>(extensions_value).map_err(|error| {
            SessionStoreError::Backend(format!(
                "workflow run extensions column must be valid extension json: {error}"
            ))
        })?;
    let credential_bindings_value: Value = row.get("credential_bindings");
    credential_bindings_value
        .as_array()
        .context("workflow run credential_bindings column must be a JSON array")
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?;
    let credential_bindings =
        serde_json::from_value::<Vec<WorkflowRunCredentialBinding>>(credential_bindings_value)
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "workflow run credential_bindings column must be valid binding json: {error}"
                ))
            })?;
    let workspace_inputs_value: Value = row.get("workspace_inputs");
    workspace_inputs_value
        .as_array()
        .context("workflow run workspace_inputs column must be a JSON array")
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?;
    let workspace_inputs = serde_json::from_value::<Vec<WorkflowRunWorkspaceInput>>(
        workspace_inputs_value,
    )
    .map_err(|error| {
        SessionStoreError::Backend(format!(
            "workflow run workspace_inputs column must be valid workspace input json: {error}"
        ))
    })?;
    let produced_files_value: Value = row.get("produced_files");
    produced_files_value
        .as_array()
        .context("workflow run produced_files column must be a JSON array")
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?;
    let produced_files = serde_json::from_value::<Vec<WorkflowRunProducedFile>>(
        produced_files_value,
    )
    .map_err(|error| {
        SessionStoreError::Backend(format!(
            "workflow run produced_files column must be valid produced file json: {error}"
        ))
    })?;
    let artifact_refs =
        row_to_json_string_array(row.get("artifact_refs"), "workflow run artifact_refs")?;
    let labels_value: Value = row.get("labels");
    let labels = labels_value
        .as_object()
        .context("workflow run labels column must be a JSON object")
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?
        .iter()
        .map(|(key, value)| {
            Ok((
                key.clone(),
                value
                    .as_str()
                    .context("workflow run label values must be strings")
                    .map_err(|error| SessionStoreError::Backend(error.to_string()))?
                    .to_string(),
            ))
        })
        .collect::<Result<HashMap<_, _>, SessionStoreError>>()?;

    Ok(StoredWorkflowRun {
        id: row.get("id"),
        owner_subject: row.get("owner_subject"),
        owner_issuer: row.get("owner_issuer"),
        workflow_definition_id: row.get("workflow_definition_id"),
        workflow_definition_version_id: row.get("workflow_definition_version_id"),
        workflow_version: row.get("workflow_version"),
        session_id: row.get("session_id"),
        automation_task_id: row.get("automation_task_id"),
        state,
        source_system: row.get("source_system"),
        source_reference: row.get("source_reference"),
        client_request_id: row.get("client_request_id"),
        create_request_fingerprint: row.get("create_request_fingerprint"),
        source_snapshot,
        extensions,
        credential_bindings,
        workspace_inputs,
        produced_files,
        input: row.get("input"),
        output: row.get("output"),
        error: row.get("error"),
        artifact_refs,
        labels,
        started_at: row.get("started_at"),
        completed_at: row.get("completed_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub(super) fn row_to_stored_workflow_run_event(
    row: &Row,
) -> Result<StoredWorkflowRunEvent, SessionStoreError> {
    Ok(StoredWorkflowRunEvent {
        id: row.get("id"),
        run_id: row.get("run_id"),
        event_type: row.get("event_type"),
        message: row.get("message"),
        data: row.get("data"),
        created_at: row.get("created_at"),
    })
}

pub(super) fn row_to_stored_workflow_event_subscription(
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

pub(super) fn row_to_stored_workflow_event_delivery(
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

pub(super) fn row_to_stored_workflow_event_delivery_attempt(
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

pub(super) fn row_to_stored_workflow_run_log(
    row: &Row,
) -> Result<StoredWorkflowRunLog, SessionStoreError> {
    let stream = row
        .get::<_, String>("stream")
        .parse::<AutomationTaskLogStream>()
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?;
    Ok(StoredWorkflowRunLog {
        id: row.get("id"),
        run_id: row.get("run_id"),
        stream,
        message: row.get("message"),
        created_at: row.get("created_at"),
    })
}

pub(super) fn row_to_runtime_assignment(
    row: &Row,
) -> Result<PersistedSessionRuntimeAssignment, SessionStoreError> {
    let status = row
        .get::<_, String>("status")
        .parse::<SessionRuntimeAssignmentStatus>()
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?;
    Ok(PersistedSessionRuntimeAssignment {
        session_id: row.get("session_id"),
        runtime_binding: row.get("runtime_binding"),
        status,
        agent_socket_path: row.get("agent_socket_path"),
        container_name: row.get("container_name"),
        cdp_endpoint: row.get("cdp_endpoint"),
    })
}

pub(super) fn row_to_recording_worker_assignment(
    row: &Row,
) -> Result<PersistedSessionRecordingWorkerAssignment, SessionStoreError> {
    let status = row
        .get::<_, String>("status")
        .parse::<SessionRecordingWorkerAssignmentStatus>()
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?;
    let process_id = row
        .get::<_, Option<i64>>("process_id")
        .map(u32::try_from)
        .transpose()
        .map_err(|error| {
            SessionStoreError::Backend(format!(
                "recording worker process_id is out of range: {error}"
            ))
        })?;
    Ok(PersistedSessionRecordingWorkerAssignment {
        session_id: row.get("session_id"),
        recording_id: row.get("recording_id"),
        status,
        process_id,
    })
}

pub(super) fn row_to_workflow_run_worker_assignment(
    row: &Row,
) -> Result<PersistedWorkflowRunWorkerAssignment, SessionStoreError> {
    let status = row
        .get::<_, String>("status")
        .parse::<WorkflowRunWorkerAssignmentStatus>()
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?;
    let process_id = row
        .get::<_, Option<i64>>("process_id")
        .map(u32::try_from)
        .transpose()
        .map_err(|error| {
            SessionStoreError::Backend(format!(
                "workflow run worker process_id is out of range: {error}"
            ))
        })?;
    Ok(PersistedWorkflowRunWorkerAssignment {
        run_id: row.get("run_id"),
        session_id: row.get("session_id"),
        automation_task_id: row.get("automation_task_id"),
        status,
        process_id,
        container_name: row.get("container_name"),
    })
}
