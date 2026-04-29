use super::*;

mod events;
mod logs;
mod queries;
mod state;

pub(super) struct AutomationTaskRepository<'a> {
    store: &'a PostgresSessionStore,
}

impl PostgresSessionStore {
    fn automation_task_repository(&self) -> AutomationTaskRepository<'_> {
        AutomationTaskRepository { store: self }
    }

    pub(in crate::session_control) async fn create_automation_task(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistAutomationTaskRequest,
    ) -> Result<StoredAutomationTask, SessionStoreError> {
        self.automation_task_repository()
            .create_automation_task(principal, request)
            .await
    }

    pub(in crate::session_control) async fn list_automation_tasks_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredAutomationTask>, SessionStoreError> {
        self.automation_task_repository()
            .list_automation_tasks_for_owner(principal)
            .await
    }

    pub(in crate::session_control) async fn get_automation_task_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredAutomationTask>, SessionStoreError> {
        self.automation_task_repository()
            .get_automation_task_for_owner(principal, id)
            .await
    }

    pub(in crate::session_control) async fn get_automation_task_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredAutomationTask>, SessionStoreError> {
        self.automation_task_repository()
            .get_automation_task_by_id(id)
            .await
    }

    pub(in crate::session_control) async fn cancel_automation_task_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredAutomationTask>, SessionStoreError> {
        self.automation_task_repository()
            .cancel_automation_task_for_owner(principal, id)
            .await
    }

    pub(in crate::session_control) async fn list_automation_task_events_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Vec<StoredAutomationTaskEvent>, SessionStoreError> {
        self.automation_task_repository()
            .list_automation_task_events_for_owner(principal, id)
            .await
    }

    pub(in crate::session_control) async fn list_automation_task_logs_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Vec<StoredAutomationTaskLog>, SessionStoreError> {
        self.automation_task_repository()
            .list_automation_task_logs_for_owner(principal, id)
            .await
    }

    pub(in crate::session_control) async fn transition_automation_task(
        &self,
        id: Uuid,
        request: AutomationTaskTransitionRequest,
    ) -> Result<Option<StoredAutomationTask>, SessionStoreError> {
        self.automation_task_repository()
            .transition_automation_task(id, request)
            .await
    }

    pub(in crate::session_control) async fn append_automation_task_log(
        &self,
        id: Uuid,
        stream: AutomationTaskLogStream,
        message: String,
    ) -> Result<Option<StoredAutomationTaskLog>, SessionStoreError> {
        self.automation_task_repository()
            .append_automation_task_log(id, stream, message)
            .await
    }
}
