use super::*;

impl WorkflowRunRepository<'_> {
    pub(in crate::session_control) async fn list_workflow_runs_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredWorkflowRun>, SessionStoreError> {
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
                    workflow_definition_id,
                    workflow_definition_version_id,
                    workflow_version,
                    session_id,
                    automation_task_id,
                    state,
                    source_system,
                    source_reference,
                    client_request_id,
                    create_request_fingerprint,
                    source_snapshot,
                    extensions,
                    credential_bindings,
                    workspace_inputs,
                    produced_files,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                FROM control_workflow_runs
                WHERE owner_subject = $1
                  AND owner_issuer = $2
                ORDER BY created_at, id
                "#,
                &[&principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to list workflow runs: {error}"))
            })?;
        rows.iter().map(row_to_stored_workflow_run).collect()
    }

    pub(in crate::session_control) async fn get_workflow_run_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(
                r#"
                SELECT
                    run.id,
                    run.owner_subject,
                    run.owner_issuer,
                    run.workflow_definition_id,
                    run.workflow_definition_version_id,
                    run.workflow_version,
                    run.session_id,
                    run.automation_task_id,
                    run.state,
                    run.source_system,
                    run.source_reference,
                    run.client_request_id,
                    run.create_request_fingerprint,
                    run.source_snapshot,
                    run.extensions,
                    run.credential_bindings,
                    run.workspace_inputs,
                    run.produced_files,
                    run.input,
                    run.output,
                    run.error,
                    run.artifact_refs,
                    run.labels,
                    run.started_at,
                    run.completed_at,
                    run.created_at,
                    run.updated_at
                FROM control_workflow_runs run
                JOIN control_workflow_definitions workflow
                  ON workflow.id = run.workflow_definition_id
                WHERE run.id = $1
                  AND workflow.owner_subject = $2
                  AND workflow.owner_issuer = $3
                "#,
                &[&id, &principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to load workflow run: {error}"))
            })?;
        row.as_ref().map(row_to_stored_workflow_run).transpose()
    }

    pub(in crate::session_control) async fn get_workflow_run_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
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
                    workflow_definition_id,
                    workflow_definition_version_id,
                    workflow_version,
                    session_id,
                    automation_task_id,
                    state,
                    source_system,
                    source_reference,
                    client_request_id,
                    create_request_fingerprint,
                    source_snapshot,
                    extensions,
                    credential_bindings,
                    workspace_inputs,
                    produced_files,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                FROM control_workflow_runs
                WHERE id = $1
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to load workflow run by id: {error}"))
            })?;
        row.as_ref().map(row_to_stored_workflow_run).transpose()
    }

    pub(in crate::session_control) async fn find_workflow_run_by_client_request_id_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        client_request_id: &str,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
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
                    workflow_definition_id,
                    workflow_definition_version_id,
                    workflow_version,
                    session_id,
                    automation_task_id,
                    state,
                    source_system,
                    source_reference,
                    client_request_id,
                    create_request_fingerprint,
                    source_snapshot,
                    extensions,
                    credential_bindings,
                    workspace_inputs,
                    produced_files,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                FROM control_workflow_runs
                WHERE owner_subject = $1
                  AND owner_issuer = $2
                  AND client_request_id = $3
                "#,
                &[&principal.subject, &principal.issuer, &client_request_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to find workflow run by client_request_id: {error}"
                ))
            })?;
        row.as_ref().map(row_to_stored_workflow_run).transpose()
    }
}
