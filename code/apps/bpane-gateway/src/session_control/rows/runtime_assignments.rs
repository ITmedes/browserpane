use super::super::*;

pub(in crate::session_control) fn row_to_runtime_assignment(
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

pub(in crate::session_control) fn row_to_recording_worker_assignment(
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

pub(in crate::session_control) fn row_to_workflow_run_worker_assignment(
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
