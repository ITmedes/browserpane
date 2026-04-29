use super::*;

mod artifacts;
mod events;
mod logs;
mod queries;
mod state;

pub(super) struct WorkflowRunRepository<'a> {
    store: &'a PostgresSessionStore,
}

impl PostgresSessionStore {
    fn workflow_run_repository(&self) -> WorkflowRunRepository<'_> {
        WorkflowRunRepository { store: self }
    }

    pub(in crate::session_control) async fn create_workflow_run(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistWorkflowRunRequest,
    ) -> Result<CreateWorkflowRunResult, SessionStoreError> {
        self.workflow_run_repository()
            .create_workflow_run(principal, request)
            .await
    }

    pub(in crate::session_control) async fn get_workflow_run_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        self.workflow_run_repository()
            .get_workflow_run_for_owner(principal, id)
            .await
    }

    pub(in crate::session_control) async fn get_workflow_run_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        self.workflow_run_repository()
            .get_workflow_run_by_id(id)
            .await
    }

    pub(in crate::session_control) async fn list_dispatchable_workflow_runs(
        &self,
    ) -> Result<Vec<StoredWorkflowRun>, SessionStoreError> {
        self.workflow_run_repository()
            .list_dispatchable_workflow_runs()
            .await
    }

    pub(in crate::session_control) async fn find_workflow_run_by_client_request_id_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        client_request_id: &str,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        self.workflow_run_repository()
            .find_workflow_run_by_client_request_id_for_owner(principal, client_request_id)
            .await
    }

    pub(in crate::session_control) async fn list_workflow_run_events_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Vec<StoredWorkflowRunEvent>, SessionStoreError> {
        self.workflow_run_repository()
            .list_workflow_run_events_for_owner(principal, id)
            .await
    }

    pub(in crate::session_control) async fn list_workflow_run_events(
        &self,
        id: Uuid,
    ) -> Result<Vec<StoredWorkflowRunEvent>, SessionStoreError> {
        self.workflow_run_repository()
            .list_workflow_run_events(id)
            .await
    }

    pub(in crate::session_control) async fn append_workflow_run_event_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        request: PersistWorkflowRunEventRequest,
    ) -> Result<Option<StoredWorkflowRunEvent>, SessionStoreError> {
        self.workflow_run_repository()
            .append_workflow_run_event_for_owner(principal, id, request)
            .await
    }

    pub(in crate::session_control) async fn append_workflow_run_event(
        &self,
        id: Uuid,
        request: PersistWorkflowRunEventRequest,
    ) -> Result<Option<StoredWorkflowRunEvent>, SessionStoreError> {
        self.workflow_run_repository()
            .append_workflow_run_event(id, request)
            .await
    }

    pub(in crate::session_control) async fn list_workflow_run_logs_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Vec<StoredWorkflowRunLog>, SessionStoreError> {
        self.workflow_run_repository()
            .list_workflow_run_logs_for_owner(principal, id)
            .await
    }

    pub(in crate::session_control) async fn append_workflow_run_log(
        &self,
        id: Uuid,
        request: PersistWorkflowRunLogRequest,
    ) -> Result<Option<StoredWorkflowRunLog>, SessionStoreError> {
        self.workflow_run_repository()
            .append_workflow_run_log(id, request)
            .await
    }

    pub(in crate::session_control) async fn transition_workflow_run(
        &self,
        id: Uuid,
        request: WorkflowRunTransitionRequest,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        self.workflow_run_repository()
            .transition_workflow_run(id, request)
            .await
    }

    pub(in crate::session_control) async fn reconcile_workflow_run_from_task(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        self.workflow_run_repository()
            .reconcile_workflow_run_from_task(id)
            .await
    }

    pub(in crate::session_control) async fn list_awaiting_input_workflow_runs(
        &self,
    ) -> Result<Vec<StoredWorkflowRun>, SessionStoreError> {
        self.workflow_run_repository()
            .list_awaiting_input_workflow_runs()
            .await
    }

    pub(in crate::session_control) async fn append_workflow_run_produced_file(
        &self,
        id: Uuid,
        request: PersistWorkflowRunProducedFileRequest,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        self.workflow_run_repository()
            .append_workflow_run_produced_file(id, request)
            .await
    }

    pub(in crate::session_control) async fn list_workflow_run_log_retention_candidates(
        &self,
        now: DateTime<Utc>,
        retention: ChronoDuration,
    ) -> Result<Vec<WorkflowRunLogRetentionCandidate>, SessionStoreError> {
        self.workflow_run_repository()
            .list_workflow_run_log_retention_candidates(now, retention)
            .await
    }

    pub(in crate::session_control) async fn delete_workflow_run_logs(
        &self,
        run_id: Uuid,
        automation_task_id: Uuid,
    ) -> Result<usize, SessionStoreError> {
        self.workflow_run_repository()
            .delete_workflow_run_logs(run_id, automation_task_id)
            .await
    }

    pub(in crate::session_control) async fn list_workflow_run_output_retention_candidates(
        &self,
        now: DateTime<Utc>,
        retention: ChronoDuration,
    ) -> Result<Vec<WorkflowRunOutputRetentionCandidate>, SessionStoreError> {
        self.workflow_run_repository()
            .list_workflow_run_output_retention_candidates(now, retention)
            .await
    }

    pub(in crate::session_control) async fn clear_workflow_run_output(
        &self,
        run_id: Uuid,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        self.workflow_run_repository()
            .clear_workflow_run_output(run_id)
            .await
    }
}
