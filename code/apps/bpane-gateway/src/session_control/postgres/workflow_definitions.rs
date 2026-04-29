use super::*;

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

impl WorkflowDefinitionRepository<'_> {
    pub(in crate::session_control) async fn create_workflow_definition(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistWorkflowDefinitionRequest,
    ) -> Result<StoredWorkflowDefinition, SessionStoreError> {
        let now = Utc::now();
        let labels_value = json_labels(&request.labels);
        let row = self
            .store
            .db
            .client()
            .await?
            .query_one(
                r#"
                INSERT INTO control_workflow_definitions (
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
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7::jsonb, NULL, $8, $8)
                RETURNING
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
                "#,
                &[
                    &Uuid::now_v7(),
                    &principal.subject,
                    &principal.issuer,
                    &principal.display_name,
                    &request.name,
                    &request.description,
                    &labels_value,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to insert workflow definition: {error}"))
            })?;
        row_to_stored_workflow_definition(&row)
    }

    pub(in crate::session_control) async fn list_workflow_definitions_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredWorkflowDefinition>, SessionStoreError> {
        let rows = self
            .store
            .db
            .client()
            .await?
            .query(
                r#"
                SELECT
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
                FROM control_workflow_definitions
                WHERE owner_subject = $1
                  AND owner_issuer = $2
                ORDER BY created_at DESC
                "#,
                &[&principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to list workflow definitions: {error}"))
            })?;
        rows.iter().map(row_to_stored_workflow_definition).collect()
    }

    pub(in crate::session_control) async fn get_workflow_definition_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowDefinition>, SessionStoreError> {
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(
                r#"
                SELECT
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
                FROM control_workflow_definitions
                WHERE id = $1
                  AND owner_subject = $2
                  AND owner_issuer = $3
                "#,
                &[&id, &principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to load workflow definition: {error}"))
            })?;
        row.as_ref()
            .map(row_to_stored_workflow_definition)
            .transpose()
    }

    pub(in crate::session_control) async fn create_workflow_definition_version(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistWorkflowDefinitionVersionRequest,
    ) -> Result<StoredWorkflowDefinitionVersion, SessionStoreError> {
        let mut client = self.store.db.client().await?;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;

        let visible = transaction
            .query_opt(
                r#"
                SELECT id
                FROM control_workflow_definitions
                WHERE id = $1
                  AND owner_subject = $2
                  AND owner_issuer = $3
                "#,
                &[
                    &request.workflow_definition_id,
                    &principal.subject,
                    &principal.issuer,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to validate workflow definition ownership: {error}"
                ))
            })?;
        if visible.is_none() {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Err(SessionStoreError::NotFound(format!(
                "workflow definition {} not found",
                request.workflow_definition_id
            )));
        }

        let now = Utc::now();
        let source_value = json_workflow_source(request.source.as_ref())?;
        let row = transaction
            .query_one(
                r#"
                INSERT INTO control_workflow_definition_versions (
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
                )
                VALUES (
                    $1, $2, $3, $4, $5, $6::jsonb, $7::jsonb, $8::jsonb, $9::jsonb,
                    $10::jsonb, $11::jsonb, $12::jsonb, $13
                )
                RETURNING
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
                "#,
                &[
                    &Uuid::now_v7(),
                    &request.workflow_definition_id,
                    &request.version,
                    &request.executor,
                    &request.entrypoint,
                    &source_value,
                    &request.input_schema,
                    &request.output_schema,
                    &request.default_session,
                    &json_string_array(&request.allowed_credential_binding_ids),
                    &json_string_array(&request.allowed_extension_ids),
                    &json_string_array(&request.allowed_file_workspace_ids),
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                if let Some(code) = error.code() {
                    if code.code() == "23505" {
                        return SessionStoreError::Conflict(format!(
                            "workflow version {} already exists",
                            request.version
                        ));
                    }
                }
                SessionStoreError::Backend(format!(
                    "failed to insert workflow definition version: {error}"
                ))
            })?;

        transaction
            .execute(
                r#"
                UPDATE control_workflow_definitions
                SET latest_version = $2, updated_at = $3
                WHERE id = $1
                "#,
                &[&request.workflow_definition_id, &request.version, &now],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to update workflow definition latest_version: {error}"
                ))
            })?;

        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;
        row_to_stored_workflow_definition_version(&row)
    }

    pub(in crate::session_control) async fn get_workflow_definition_version_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        workflow_definition_id: Uuid,
        version: &str,
    ) -> Result<Option<StoredWorkflowDefinitionVersion>, SessionStoreError> {
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(
                r#"
                SELECT
                    version.id,
                    version.workflow_definition_id,
                    version.version,
                    version.executor,
                    version.entrypoint,
                    version.source,
                    version.input_schema,
                    version.output_schema,
                    version.default_session,
                    version.allowed_credential_binding_ids,
                    version.allowed_extension_ids,
                    version.allowed_file_workspace_ids,
                    version.created_at
                FROM control_workflow_definition_versions version
                JOIN control_workflow_definitions workflow
                  ON workflow.id = version.workflow_definition_id
                WHERE version.workflow_definition_id = $1
                  AND version.version = $2
                  AND workflow.owner_subject = $3
                  AND workflow.owner_issuer = $4
                "#,
                &[
                    &workflow_definition_id,
                    &version,
                    &principal.subject,
                    &principal.issuer,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to load workflow definition version: {error}"
                ))
            })?;
        row.as_ref()
            .map(row_to_stored_workflow_definition_version)
            .transpose()
    }

    pub(in crate::session_control) async fn get_workflow_definition_version_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowDefinitionVersion>, SessionStoreError> {
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(
                r#"
                SELECT
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
                FROM control_workflow_definition_versions
                WHERE id = $1
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to load workflow definition version by id: {error}"
                ))
            })?;
        row.as_ref()
            .map(row_to_stored_workflow_definition_version)
            .transpose()
    }
}
