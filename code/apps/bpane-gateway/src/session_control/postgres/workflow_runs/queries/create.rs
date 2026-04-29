use super::*;

impl WorkflowRunRepository<'_> {
    pub(in crate::session_control) async fn create_workflow_run(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistWorkflowRunRequest,
    ) -> Result<CreateWorkflowRunResult, SessionStoreError> {
        let mut client = self.store.db.client().await?;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;

        let workflow_row = transaction
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
                    "failed to validate workflow definition for run: {error}"
                ))
            })?;
        if workflow_row.is_none() {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Err(SessionStoreError::NotFound(format!(
                "workflow definition {} not found",
                request.workflow_definition_id
            )));
        }

        let version_row = transaction
            .query_opt(
                r#"
                SELECT id, workflow_definition_id, version
                FROM control_workflow_definition_versions
                WHERE id = $1
                "#,
                &[&request.workflow_definition_version_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to validate workflow definition version for run: {error}"
                ))
            })?;
        let Some(version_row) = version_row else {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Err(SessionStoreError::NotFound(format!(
                "workflow definition version {} not found",
                request.workflow_definition_version_id
            )));
        };
        let version_workflow_id: Uuid = version_row.get("workflow_definition_id");
        let version_name: String = version_row.get("version");
        if version_workflow_id != request.workflow_definition_id
            || version_name != request.workflow_version
        {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Err(SessionStoreError::InvalidRequest(
                "workflow run version must belong to the requested workflow definition".to_string(),
            ));
        }

        let task_row = transaction
            .query_opt(
                r#"
                SELECT id, session_id
                FROM control_automation_tasks
                WHERE id = $1
                  AND owner_subject = $2
                  AND owner_issuer = $3
                "#,
                &[
                    &request.automation_task_id,
                    &principal.subject,
                    &principal.issuer,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to validate automation task for workflow run: {error}"
                ))
            })?;
        let Some(task_row) = task_row else {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Err(SessionStoreError::NotFound(format!(
                "automation task {} not found",
                request.automation_task_id
            )));
        };
        let task_session_id: Uuid = task_row.get("session_id");
        if task_session_id != request.session_id {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Err(SessionStoreError::InvalidRequest(
                "workflow run session_id must match the bound automation task session".to_string(),
            ));
        }

        let now = Utc::now();
        let source_snapshot = json_workflow_run_source_snapshot(request.source_snapshot.as_ref())?;
        let extensions = json_applied_extensions(&request.extensions)?;
        let credential_bindings =
            json_workflow_run_credential_bindings(&request.credential_bindings)?;
        let workspace_inputs = json_workflow_run_workspace_inputs(&request.workspace_inputs)?;
        let produced_files = json_workflow_run_produced_files(&Vec::new())?;
        if let Some(client_request_id) = request.client_request_id.as_deref() {
            let existing_row = transaction
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
                        "failed to check existing workflow run by client_request_id: {error}"
                    ))
                })?;
            if let Some(existing_row) = existing_row {
                let existing_run = row_to_stored_workflow_run(&existing_row)?;
                transaction.commit().await.map_err(|error| {
                    SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
                })?;
                if existing_run.create_request_fingerprint == request.create_request_fingerprint {
                    return Ok(CreateWorkflowRunResult {
                        run: existing_run,
                        created: false,
                    });
                }
                return Err(SessionStoreError::Conflict(format!(
                    "workflow run client_request_id {} is already bound to a different request",
                    client_request_id
                )));
            }
        }
        let row = transaction
            .query_one(
                r#"
                INSERT INTO control_workflow_runs (
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
                )
                VALUES (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9,
                    $10, $11, $12, $13,
                    $14::jsonb, $15::jsonb, $16::jsonb, $17::jsonb, $18::jsonb, $19::jsonb, NULL, NULL, $20::jsonb, $21::jsonb, NULL, NULL, $22, $22
                )
                RETURNING
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
                "#,
                &[
                    &Uuid::now_v7(),
                    &principal.subject,
                    &principal.issuer,
                    &request.workflow_definition_id,
                    &request.workflow_definition_version_id,
                    &request.workflow_version,
                    &request.session_id,
                    &request.automation_task_id,
                    &WorkflowRunState::Pending.as_str(),
                    &request.source_system,
                    &request.source_reference,
                    &request.client_request_id,
                    &request.create_request_fingerprint,
                    &source_snapshot,
                    &extensions,
                    &credential_bindings,
                    &workspace_inputs,
                    &produced_files,
                    &request.input,
                    &json_string_array(&Vec::new()),
                    &json_labels(&request.labels),
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to insert workflow run: {error}"))
            })?;
        let run = row_to_stored_workflow_run(&row)?;
        let run_id = run.id;
        let event_id = Uuid::now_v7();
        let event = StoredWorkflowRunEvent {
            id: event_id,
            run_id,
            event_type: "workflow_run.created".to_string(),
            message: "workflow run created".to_string(),
            data: Some(serde_json::json!({
                "workflow_definition_id": request.workflow_definition_id,
                "workflow_definition_version_id": request.workflow_definition_version_id,
                "workflow_version": request.workflow_version,
                "session_id": request.session_id,
                "automation_task_id": request.automation_task_id,
                "source_system": request.source_system,
                "source_reference": request.source_reference,
                "client_request_id": request.client_request_id,
            })),
            created_at: now,
        };

        transaction
            .execute(
                r#"
                INSERT INTO control_workflow_run_events (
                    id,
                    run_id,
                    event_type,
                    message,
                    data,
                    created_at
                )
                VALUES ($1, $2, $3, $4, $5::jsonb, $6)
                "#,
                &[
                    &event_id,
                    &run_id,
                    &"workflow_run.created",
                    &"workflow run created",
                    &event.data,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to insert workflow run event: {error}"))
            })?;
        PostgresSessionStore::enqueue_workflow_event_deliveries(&transaction, &run, &event).await?;

        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;
        Ok(CreateWorkflowRunResult { run, created: true })
    }
}
