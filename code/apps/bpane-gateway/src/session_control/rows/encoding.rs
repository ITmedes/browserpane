use super::super::*;

pub(in crate::session_control) fn json_labels(labels: &HashMap<String, String>) -> Value {
    let mut object = JsonMap::new();
    for (key, value) in labels {
        object.insert(key.clone(), Value::String(value.clone()));
    }
    Value::Object(object)
}

pub(in crate::session_control) fn json_workflow_source(
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

pub(in crate::session_control) fn json_workflow_run_source_snapshot(
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

pub(in crate::session_control) fn json_applied_extensions(
    extensions: &[AppliedExtension],
) -> Result<Value, SessionStoreError> {
    serde_json::to_value(extensions).map_err(|error| {
        SessionStoreError::Backend(format!("failed to encode applied extensions: {error}"))
    })
}

pub(in crate::session_control) fn json_workflow_run_credential_bindings(
    credential_bindings: &[WorkflowRunCredentialBinding],
) -> Result<Value, SessionStoreError> {
    serde_json::to_value(credential_bindings).map_err(|error| {
        SessionStoreError::Backend(format!(
            "failed to encode workflow run credential_bindings: {error}"
        ))
    })
}

pub(in crate::session_control) fn json_workflow_run_workspace_inputs(
    workspace_inputs: &[WorkflowRunWorkspaceInput],
) -> Result<Value, SessionStoreError> {
    serde_json::to_value(workspace_inputs).map_err(|error| {
        SessionStoreError::Backend(format!(
            "failed to encode workflow run workspace_inputs: {error}"
        ))
    })
}

pub(in crate::session_control) fn json_workflow_run_produced_files(
    produced_files: &[WorkflowRunProducedFile],
) -> Result<Value, SessionStoreError> {
    serde_json::to_value(produced_files).map_err(|error| {
        SessionStoreError::Backend(format!(
            "failed to encode workflow run produced_files: {error}"
        ))
    })
}

pub(in crate::session_control) fn describe_postgres_error(error: &tokio_postgres::Error) -> String {
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

pub(in crate::session_control) fn sync_workflow_run_with_task(
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

pub(in crate::session_control) fn json_string_array(values: &[String]) -> Value {
    Value::Array(
        values
            .iter()
            .cloned()
            .map(Value::String)
            .collect::<Vec<_>>(),
    )
}

pub(in crate::session_control) fn row_to_json_string_array(
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

pub(in crate::session_control) fn recording_mime_type(
    format: SessionRecordingFormat,
) -> &'static str {
    match format {
        SessionRecordingFormat::Webm => "video/webm",
    }
}

pub(in crate::session_control) fn json_recording_policy(
    recording: &SessionRecordingPolicy,
) -> Result<Value, SessionStoreError> {
    serde_json::to_value(recording).map_err(|error| {
        SessionStoreError::Backend(format!("failed to encode recording policy: {error}"))
    })
}
