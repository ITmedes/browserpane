use super::*;

impl SessionStore {
    pub async fn create_automation_task(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistAutomationTaskRequest,
    ) -> Result<StoredAutomationTask, SessionStoreError> {
        validate_persist_automation_task_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.create_automation_task(principal, request).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.create_automation_task(principal, request).await
            }
        }
    }

    pub async fn list_automation_tasks_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredAutomationTask>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.list_automation_tasks_for_owner(principal).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.list_automation_tasks_for_owner(principal).await
            }
        }
    }

    pub async fn get_automation_task_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredAutomationTask>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.get_automation_task_for_owner(principal, id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.get_automation_task_for_owner(principal, id).await
            }
        }
    }

    pub async fn get_automation_task_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredAutomationTask>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => store.get_automation_task_by_id(id).await,
            SessionStoreBackend::Postgres(store) => store.get_automation_task_by_id(id).await,
        }
    }

    pub async fn cancel_automation_task_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredAutomationTask>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.cancel_automation_task_for_owner(principal, id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.cancel_automation_task_for_owner(principal, id).await
            }
        }
    }

    pub async fn list_automation_task_events_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Vec<StoredAutomationTaskEvent>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .list_automation_task_events_for_owner(principal, id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .list_automation_task_events_for_owner(principal, id)
                    .await
            }
        }
    }

    pub async fn list_automation_task_logs_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Vec<StoredAutomationTaskLog>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .list_automation_task_logs_for_owner(principal, id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .list_automation_task_logs_for_owner(principal, id)
                    .await
            }
        }
    }

    pub async fn transition_automation_task(
        &self,
        id: Uuid,
        request: AutomationTaskTransitionRequest,
    ) -> Result<Option<StoredAutomationTask>, SessionStoreError> {
        validate_automation_task_transition_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.transition_automation_task(id, request).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.transition_automation_task(id, request).await
            }
        }
    }

    pub async fn append_automation_task_log(
        &self,
        id: Uuid,
        stream: AutomationTaskLogStream,
        message: String,
    ) -> Result<Option<StoredAutomationTaskLog>, SessionStoreError> {
        validate_automation_task_log_message(&message)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.append_automation_task_log(id, stream, message).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.append_automation_task_log(id, stream, message).await
            }
        }
    }
}
