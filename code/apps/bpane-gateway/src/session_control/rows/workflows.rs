use super::super::*;
use super::encoding::row_to_json_string_array;

pub(in crate::session_control) fn row_to_stored_workflow_run(
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

pub(in crate::session_control) fn row_to_stored_workflow_run_event(
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

pub(in crate::session_control) fn row_to_stored_workflow_run_log(
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
