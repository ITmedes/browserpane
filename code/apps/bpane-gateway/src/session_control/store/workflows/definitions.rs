use super::super::*;

impl SessionStore {
    pub async fn create_workflow_definition(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistWorkflowDefinitionRequest,
    ) -> Result<StoredWorkflowDefinition, SessionStoreError> {
        validate_workflow_definition_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.create_workflow_definition(principal, request).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.create_workflow_definition(principal, request).await
            }
        }
    }

    pub async fn list_workflow_definitions_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredWorkflowDefinition>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.list_workflow_definitions_for_owner(principal).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.list_workflow_definitions_for_owner(principal).await
            }
        }
    }

    pub async fn get_workflow_definition_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowDefinition>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.get_workflow_definition_for_owner(principal, id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.get_workflow_definition_for_owner(principal, id).await
            }
        }
    }

    pub async fn create_workflow_definition_version(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistWorkflowDefinitionVersionRequest,
    ) -> Result<StoredWorkflowDefinitionVersion, SessionStoreError> {
        validate_workflow_definition_version_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .create_workflow_definition_version(principal, request)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .create_workflow_definition_version(principal, request)
                    .await
            }
        }
    }

    pub async fn get_workflow_definition_version_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        workflow_definition_id: Uuid,
        version: &str,
    ) -> Result<Option<StoredWorkflowDefinitionVersion>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .get_workflow_definition_version_for_owner(
                        principal,
                        workflow_definition_id,
                        version,
                    )
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .get_workflow_definition_version_for_owner(
                        principal,
                        workflow_definition_id,
                        version,
                    )
                    .await
            }
        }
    }

    pub async fn list_workflow_definition_versions_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        workflow_definition_id: Uuid,
    ) -> Result<Vec<StoredWorkflowDefinitionVersion>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .list_workflow_definition_versions_for_owner(principal, workflow_definition_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .list_workflow_definition_versions_for_owner(principal, workflow_definition_id)
                    .await
            }
        }
    }

    pub async fn get_workflow_definition_version_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowDefinitionVersion>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.get_workflow_definition_version_by_id(id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.get_workflow_definition_version_by_id(id).await
            }
        }
    }
}
