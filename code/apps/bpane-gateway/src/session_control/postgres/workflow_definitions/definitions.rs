use super::*;

impl WorkflowDefinitionRepository<'_> {
    pub(in crate::session_control) async fn create_workflow_definition(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistWorkflowDefinitionRequest,
    ) -> Result<StoredWorkflowDefinition, SessionStoreError> {
        let now = Utc::now();
        let labels_value = json_labels(&request.labels);
        let insert_query = format!(
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
                {WORKFLOW_DEFINITION_COLUMNS}
            "#
        );
        let row = self
            .store
            .db
            .client()
            .await?
            .query_one(
                &insert_query,
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
        let query = format!(
            r#"
            SELECT
                {WORKFLOW_DEFINITION_COLUMNS}
            FROM control_workflow_definitions
            WHERE owner_subject = $1
              AND owner_issuer = $2
            ORDER BY created_at DESC
            "#
        );
        let rows = self
            .store
            .db
            .client()
            .await?
            .query(&query, &[&principal.subject, &principal.issuer])
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
        let query = format!(
            r#"
            SELECT
                {WORKFLOW_DEFINITION_COLUMNS}
            FROM control_workflow_definitions
            WHERE id = $1
              AND owner_subject = $2
              AND owner_issuer = $3
            "#
        );
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(&query, &[&id, &principal.subject, &principal.issuer])
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to load workflow definition: {error}"))
            })?;
        row.as_ref()
            .map(row_to_stored_workflow_definition)
            .transpose()
    }
}
