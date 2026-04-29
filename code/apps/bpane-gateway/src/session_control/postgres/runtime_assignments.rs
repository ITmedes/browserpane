use super::*;

mod recording_workers;
mod session_runtimes;
mod workflow_workers;

const RUNTIME_ASSIGNMENT_COLUMNS: &str = r#"
    session_id,
    runtime_binding,
    status,
    agent_socket_path,
    container_name,
    cdp_endpoint
"#;

const RECORDING_WORKER_COLUMNS: &str = r#"
    session_id,
    recording_id,
    status,
    process_id
"#;

const WORKFLOW_RUN_WORKER_COLUMNS: &str = r#"
    run_id,
    session_id,
    automation_task_id,
    status,
    process_id,
    container_name
"#;

const SESSION_RECOVERY_COLUMNS: &str = r#"
    id,
    owner_subject,
    owner_issuer,
    owner_display_name,
    automation_owner_client_id,
    automation_owner_issuer,
    automation_owner_display_name,
    state,
    template_id,
    owner_mode,
    viewport_width,
    viewport_height,
    idle_timeout_sec,
    labels,
    integration_context,
    extensions,
    recording,
    created_at,
    updated_at,
    stopped_at
"#;

pub(super) struct RuntimeAssignmentRepository<'a> {
    store: &'a PostgresSessionStore,
}

impl PostgresSessionStore {
    fn runtime_assignment_repository(&self) -> RuntimeAssignmentRepository<'_> {
        RuntimeAssignmentRepository { store: self }
    }

    pub(in crate::session_control) async fn upsert_runtime_assignment(
        &self,
        assignment: PersistedSessionRuntimeAssignment,
    ) -> Result<(), SessionStoreError> {
        self.runtime_assignment_repository()
            .upsert_runtime_assignment(assignment)
            .await
    }

    pub(in crate::session_control) async fn clear_runtime_assignment(
        &self,
        id: Uuid,
    ) -> Result<(), SessionStoreError> {
        self.runtime_assignment_repository()
            .clear_runtime_assignment(id)
            .await
    }

    pub(in crate::session_control) async fn upsert_recording_worker_assignment(
        &self,
        assignment: PersistedSessionRecordingWorkerAssignment,
    ) -> Result<(), SessionStoreError> {
        self.runtime_assignment_repository()
            .upsert_recording_worker_assignment(assignment)
            .await
    }

    pub(in crate::session_control) async fn clear_recording_worker_assignment(
        &self,
        id: Uuid,
    ) -> Result<(), SessionStoreError> {
        self.runtime_assignment_repository()
            .clear_recording_worker_assignment(id)
            .await
    }

    pub(in crate::session_control) async fn get_recording_worker_assignment(
        &self,
        id: Uuid,
    ) -> Result<Option<PersistedSessionRecordingWorkerAssignment>, SessionStoreError> {
        self.runtime_assignment_repository()
            .get_recording_worker_assignment(id)
            .await
    }

    pub(in crate::session_control) async fn list_recording_worker_assignments(
        &self,
    ) -> Result<Vec<PersistedSessionRecordingWorkerAssignment>, SessionStoreError> {
        self.runtime_assignment_repository()
            .list_recording_worker_assignments()
            .await
    }

    pub(in crate::session_control) async fn upsert_workflow_run_worker_assignment(
        &self,
        assignment: PersistedWorkflowRunWorkerAssignment,
    ) -> Result<(), SessionStoreError> {
        self.runtime_assignment_repository()
            .upsert_workflow_run_worker_assignment(assignment)
            .await
    }

    pub(in crate::session_control) async fn clear_workflow_run_worker_assignment(
        &self,
        run_id: Uuid,
    ) -> Result<(), SessionStoreError> {
        self.runtime_assignment_repository()
            .clear_workflow_run_worker_assignment(run_id)
            .await
    }

    pub(in crate::session_control) async fn get_workflow_run_worker_assignment(
        &self,
        run_id: Uuid,
    ) -> Result<Option<PersistedWorkflowRunWorkerAssignment>, SessionStoreError> {
        self.runtime_assignment_repository()
            .get_workflow_run_worker_assignment(run_id)
            .await
    }

    pub(in crate::session_control) async fn list_workflow_run_worker_assignments(
        &self,
    ) -> Result<Vec<PersistedWorkflowRunWorkerAssignment>, SessionStoreError> {
        self.runtime_assignment_repository()
            .list_workflow_run_worker_assignments()
            .await
    }

    pub(in crate::session_control) async fn list_runtime_assignments(
        &self,
        runtime_binding: &str,
    ) -> Result<Vec<PersistedSessionRuntimeAssignment>, SessionStoreError> {
        self.runtime_assignment_repository()
            .list_runtime_assignments(runtime_binding)
            .await
    }

    pub(in crate::session_control) async fn mark_session_ready_after_runtime_loss(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        self.runtime_assignment_repository()
            .mark_session_ready_after_runtime_loss(id)
            .await
    }
}
