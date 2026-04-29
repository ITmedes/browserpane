use super::super::*;

pub(in crate::session_control) fn row_to_stored_automation_task(
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

pub(in crate::session_control) fn row_to_stored_automation_task_event(
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

pub(in crate::session_control) fn row_to_stored_automation_task_log(
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
