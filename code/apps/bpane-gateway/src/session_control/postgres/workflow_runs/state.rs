use super::*;

impl WorkflowRunRepository<'_> {
    pub(in crate::session_control) async fn transition_workflow_run(
        &self,
        id: Uuid,
        request: WorkflowRunTransitionRequest,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        let mut client = self.store.db.client().await?;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;

        let run_row = transaction
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
                FOR UPDATE
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to lock workflow run for transition: {error}"
                ))
            })?;
        let Some(run_row) = run_row else {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Ok(None);
        };
        let current_run = row_to_stored_workflow_run(&run_row)?;

        let task_row = transaction
            .query_one(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    display_name,
                    executor,
                    state,
                    session_id,
                    session_source,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    cancel_requested_at,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                FROM control_automation_tasks
                WHERE id = $1
                FOR UPDATE
                "#,
                &[&current_run.automation_task_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to lock automation task for workflow run transition: {error}"
                ))
            })?;
        let now = Utc::now();
        let current_task = row_to_stored_automation_task(&task_row)?;
        let plan = plan_workflow_run_transition(&current_task, &request, now)
            .map_err(|error| SessionStoreError::Conflict(error.to_string()))?;
        let artifact_refs = json_string_array(&plan.task_artifact_refs);
        let task_row = transaction
            .query_one(
                r#"
                UPDATE control_automation_tasks
                SET
                    state = $2,
                    output = $3::jsonb,
                    error = $4,
                    artifact_refs = $5::jsonb,
                    started_at = $6,
                    completed_at = $7,
                    updated_at = $8
                WHERE id = $1
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    display_name,
                    executor,
                    state,
                    session_id,
                    session_source,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    cancel_requested_at,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                "#,
                &[
                    &current_task.id,
                    &plan.task_state.as_str(),
                    &plan.task_output,
                    &plan.task_error,
                    &artifact_refs,
                    &plan.task_started_at,
                    &plan.task_completed_at,
                    &plan.task_updated_at,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to update automation task for workflow run transition: {error}"
                ))
            })?;
        let task = row_to_stored_automation_task(&task_row)?;

        transaction
            .execute(
                r#"
                INSERT INTO control_automation_task_events (
                    id,
                    task_id,
                    event_type,
                    message,
                    data,
                    created_at
                )
                VALUES ($1, $2, $3, $4, $5::jsonb, $6)
                "#,
                &[
                    &Uuid::now_v7(),
                    &task.id,
                    &plan.task_event_type,
                    &plan.task_event_message,
                    &plan.task_event_data,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to insert automation task event for workflow run transition: {error}"
                ))
            })?;

        let run_row = transaction
            .query_one(
                r#"
                UPDATE control_workflow_runs
                SET
                    state = $2,
                    output = $3::jsonb,
                    error = $4,
                    artifact_refs = $5::jsonb,
                    started_at = $6,
                    completed_at = $7,
                    updated_at = $8
                WHERE id = $1
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
                    &id,
                    &plan.run_state.as_str(),
                    &plan.run_output,
                    &plan.run_error,
                    &json_string_array(&plan.run_artifact_refs),
                    &plan.run_started_at,
                    &plan.run_completed_at,
                    &plan.run_updated_at,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to update workflow run state: {error}"))
            })?;

        let event = StoredWorkflowRunEvent {
            id: Uuid::now_v7(),
            run_id: id,
            event_type: plan.run_event_type,
            message: plan.run_event_message.clone(),
            data: plan.run_event_data,
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
                    &event.id,
                    &id,
                    &event.event_type,
                    &event.message,
                    &event.data,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to insert workflow run transition event: {error}"
                ))
            })?;
        let run = row_to_stored_workflow_run(&run_row)?;
        PostgresSessionStore::enqueue_workflow_event_deliveries(&transaction, &run, &event).await?;

        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;

        Ok(Some(run))
    }

    pub(in crate::session_control) async fn reconcile_workflow_run_from_task(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        let mut client = self.store.db.client().await?;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;

        let run_row = transaction
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
                FOR UPDATE
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to lock workflow run for reconciliation: {error}"
                ))
            })?;
        let Some(run_row) = run_row else {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Ok(None);
        };
        let current_run = row_to_stored_workflow_run(&run_row)?;

        let task_row = transaction
            .query_one(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    display_name,
                    executor,
                    state,
                    session_id,
                    session_source,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    cancel_requested_at,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                FROM control_automation_tasks
                WHERE id = $1
                FOR UPDATE
                "#,
                &[&current_run.automation_task_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to lock automation task for workflow run reconciliation: {error}"
                ))
            })?;
        let current_task = row_to_stored_automation_task(&task_row)?;
        let now = Utc::now();
        let (decision, plan) = plan_workflow_run_reconciliation(&current_run, &current_task, now);
        match decision {
            WorkflowRunReconciliationDecision::NotTerminal
            | WorkflowRunReconciliationDecision::Unchanged => {
                transaction.commit().await.map_err(|error| {
                    SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
                })?;
                return Ok(Some(current_run));
            }
            WorkflowRunReconciliationDecision::Update => {}
        }
        let plan = plan.expect("workflow run reconciliation update plan must exist");
        let artifact_refs = json_string_array(&plan.run_artifact_refs);
        let run_row = transaction
            .query_one(
                r#"
                UPDATE control_workflow_runs
                SET
                    state = $2,
                    output = $3::jsonb,
                    error = $4,
                    artifact_refs = $5::jsonb,
                    started_at = $6,
                    completed_at = $7,
                    updated_at = $8
                WHERE id = $1
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
                    &id,
                    &plan.run_state.as_str(),
                    &plan.run_output,
                    &plan.run_error,
                    &artifact_refs,
                    &plan.run_started_at,
                    &plan.run_completed_at,
                    &plan.run_updated_at,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to reconcile workflow run state from automation task: {error}"
                ))
            })?;

        let event = StoredWorkflowRunEvent {
            id: Uuid::now_v7(),
            run_id: id,
            event_type: plan.run_event_type,
            message: plan.run_event_message,
            data: plan.run_event_data,
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
                    &event.id,
                    &id,
                    &event.event_type,
                    &event.message,
                    &event.data,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to append workflow run reconciliation event: {error}"
                ))
            })?;
        let run = row_to_stored_workflow_run(&run_row)?;
        PostgresSessionStore::enqueue_workflow_event_deliveries(&transaction, &run, &event).await?;

        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;

        Ok(Some(run))
    }

    pub(in crate::session_control) async fn list_awaiting_input_workflow_runs(
        &self,
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
                WHERE state = 'awaiting_input'
                ORDER BY updated_at ASC, id ASC
                "#,
                &[],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to list awaiting-input workflow runs: {error}"
                ))
            })?;
        rows.into_iter()
            .map(|row| row_to_stored_workflow_run(&row))
            .collect()
    }
}
