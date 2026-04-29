use super::*;

mod definitions;
mod versions;

const WORKFLOW_DEFINITION_COLUMNS: &str = r#"
    id,
    owner_subject,
    owner_issuer,
    owner_display_name,
    name,
    description,
    labels,
    latest_version,
    created_at,
    updated_at
"#;

const WORKFLOW_DEFINITION_VERSION_COLUMNS: &str = r#"
    id,
    workflow_definition_id,
    version,
    executor,
    entrypoint,
    source,
    input_schema,
    output_schema,
    default_session,
    allowed_credential_binding_ids,
    allowed_extension_ids,
    allowed_file_workspace_ids,
    created_at
"#;

const WORKFLOW_DEFINITION_VERSION_COLUMNS_FROM_VERSION_ALIAS: &str = r#"
    version.id AS id,
    version.workflow_definition_id AS workflow_definition_id,
    version.version AS version,
    version.executor AS executor,
    version.entrypoint AS entrypoint,
    version.source AS source,
    version.input_schema AS input_schema,
    version.output_schema AS output_schema,
    version.default_session AS default_session,
    version.allowed_credential_binding_ids AS allowed_credential_binding_ids,
    version.allowed_extension_ids AS allowed_extension_ids,
    version.allowed_file_workspace_ids AS allowed_file_workspace_ids,
    version.created_at AS created_at
"#;

pub(super) struct WorkflowDefinitionRepository<'a> {
    store: &'a PostgresSessionStore,
}

impl PostgresSessionStore {
    fn workflow_definition_repository(&self) -> WorkflowDefinitionRepository<'_> {
        WorkflowDefinitionRepository { store: self }
    }

    pub(in crate::session_control) async fn create_workflow_definition(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistWorkflowDefinitionRequest,
    ) -> Result<StoredWorkflowDefinition, SessionStoreError> {
        self.workflow_definition_repository()
            .create_workflow_definition(principal, request)
            .await
    }

    pub(in crate::session_control) async fn list_workflow_definitions_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredWorkflowDefinition>, SessionStoreError> {
        self.workflow_definition_repository()
            .list_workflow_definitions_for_owner(principal)
            .await
    }

    pub(in crate::session_control) async fn get_workflow_definition_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowDefinition>, SessionStoreError> {
        self.workflow_definition_repository()
            .get_workflow_definition_for_owner(principal, id)
            .await
    }

    pub(in crate::session_control) async fn create_workflow_definition_version(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistWorkflowDefinitionVersionRequest,
    ) -> Result<StoredWorkflowDefinitionVersion, SessionStoreError> {
        self.workflow_definition_repository()
            .create_workflow_definition_version(principal, request)
            .await
    }

    pub(in crate::session_control) async fn get_workflow_definition_version_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        workflow_definition_id: Uuid,
        version: &str,
    ) -> Result<Option<StoredWorkflowDefinitionVersion>, SessionStoreError> {
        self.workflow_definition_repository()
            .get_workflow_definition_version_for_owner(principal, workflow_definition_id, version)
            .await
    }

    pub(in crate::session_control) async fn get_workflow_definition_version_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowDefinitionVersion>, SessionStoreError> {
        self.workflow_definition_repository()
            .get_workflow_definition_version_by_id(id)
            .await
    }
}
