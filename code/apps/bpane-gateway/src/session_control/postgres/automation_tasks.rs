use super::*;

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

impl AutomationTaskRepository<'_> {
    pub(in crate::session_control) async fn create_automation_task(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistAutomationTaskRequest,
    ) -> Result<StoredAutomationTask, SessionStoreError> {
        let mut client = self.store.db.client().await?;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;

        let now = Utc::now();
        let task_id = Uuid::now_v7();
        let labels_value = json_labels(&request.labels);
        let artifact_refs_value = Value::Array(Vec::new());
        let row = transaction
            .query_one(
                r#"
                INSERT INTO control_automation_tasks (
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
                )
                VALUES (
                    $1, $2, $3, $4, $5, $6, 'pending', $7, $8, $9::jsonb, NULL, NULL,
                    $10::jsonb, $11::jsonb, NULL, NULL, NULL, $12, $12
                )
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
                    &task_id,
                    &principal.subject,
                    &principal.issuer,
                    &principal.display_name,
                    &request.display_name,
                    &request.executor,
                    &request.session_id,
                    &request.session_source.as_str(),
                    &request.input,
                    &artifact_refs_value,
                    &labels_value,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to insert automation task: {error}"))
            })?;

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
                    &task_id,
                    &"automation_task.created",
                    &"automation task created",
                    &Some(serde_json::json!({
                        "session_id": request.session_id,
                        "session_source": request.session_source.as_str(),
                        "executor": request.executor,
                    })),
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to insert automation task event: {error}"
                ))
            })?;

        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;

        row_to_stored_automation_task(&row)
    }

    pub(in crate::session_control) async fn list_automation_tasks_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredAutomationTask>, SessionStoreError> {
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
                WHERE owner_subject = $1
                  AND owner_issuer = $2
                ORDER BY created_at DESC
                "#,
                &[&principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to list automation tasks: {error}"))
            })?;

        rows.iter().map(row_to_stored_automation_task).collect()
    }

    pub(in crate::session_control) async fn get_automation_task_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredAutomationTask>, SessionStoreError> {
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
                  AND owner_subject = $2
                  AND owner_issuer = $3
                "#,
                &[&id, &principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to load automation task: {error}"))
            })?;
        row.as_ref().map(row_to_stored_automation_task).transpose()
    }

    pub(in crate::session_control) async fn get_automation_task_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredAutomationTask>, SessionStoreError> {
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
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to load automation task by id: {error}"))
            })?;
        row.as_ref().map(row_to_stored_automation_task).transpose()
    }

    pub(in crate::session_control) async fn cancel_automation_task_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredAutomationTask>, SessionStoreError> {
        let mut client = self.store.db.client().await?;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;

        let current_row = transaction
            .query_opt(
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
                  AND owner_subject = $2
                  AND owner_issuer = $3
                FOR UPDATE
                "#,
                &[&id, &principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to lock automation task for cancellation: {error}"
                ))
            })?;
        let Some(current_row) = current_row else {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Ok(None);
        };
        let current = row_to_stored_automation_task(&current_row)?;
        let cancellation_plan = plan_automation_task_cancellation(&current, Utc::now())
            .map_err(|error| SessionStoreError::Conflict(error.to_string()))?;

        let now = cancellation_plan.task_updated_at;
        let row = transaction
            .query_one(
                r#"
                UPDATE control_automation_tasks
                SET
                    state = $2,
                    cancel_requested_at = $3,
                    completed_at = $4,
                    updated_at = $5
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
                    &id,
                    &cancellation_plan.task_state.as_str(),
                    &cancellation_plan.task_cancel_requested_at,
                    &cancellation_plan.task_completed_at,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to cancel automation task: {error}"))
            })?;
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
                    &id,
                    &cancellation_plan.task_event_type,
                    &cancellation_plan.task_event_message,
                    &cancellation_plan.task_event_data,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to insert automation task cancel event: {error}"
                ))
            })?;
        transaction
            .execute(
                r#"
                INSERT INTO control_automation_task_logs (
                    id,
                    task_id,
                    stream,
                    message,
                    created_at
                )
                VALUES ($1, $2, $3, $4, $5)
                "#,
                &[
                    &Uuid::now_v7(),
                    &id,
                    &cancellation_plan.task_log_stream.as_str(),
                    &cancellation_plan.task_log_message,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to insert automation task cancel log: {error}"
                ))
            })?;

        let task = row_to_stored_automation_task(&row)?;
        let workflow_run_row = transaction
            .execute(
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
                WHERE automation_task_id = $1
                "#,
                &[
                    &task.id,
                    &WorkflowRunState::from(task.state).as_str(),
                    &task.output,
                    &task.error,
                    &json_string_array(&task.artifact_refs),
                    &task.started_at,
                    &task.completed_at,
                    &task.updated_at,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to sync workflow run after automation task cancel: {error}"
                ))
            })?;

        let workflow_run_id = if workflow_run_row > 0 {
            transaction
                .query_opt(
                    r#"
                    SELECT id
                    FROM control_workflow_runs
                    WHERE automation_task_id = $1
                    "#,
                    &[&task.id],
                )
                .await
                .map_err(|error| {
                    SessionStoreError::Backend(format!(
                        "failed to load workflow run after automation task cancel: {error}"
                    ))
                })?
                .map(|row| row.get::<_, Uuid>("id"))
        } else {
            None
        };

        if let Some(run_id) = workflow_run_id {
            let run_row = transaction
                .query_one(
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
                    &[&run_id],
                )
                .await
                .map_err(|error| {
                    SessionStoreError::Backend(format!(
                        "failed to reload workflow run after automation task cancel: {error}"
                    ))
                })?;
            let run = row_to_stored_workflow_run(&run_row)?;
            let event = StoredWorkflowRunEvent {
                id: Uuid::now_v7(),
                run_id,
                event_type: cancellation_plan.run_event_type,
                message: cancellation_plan.run_event_message,
                data: cancellation_plan.run_event_data,
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
                    VALUES ($1, $2, $3, $4, NULL, $5)
                    "#,
                    &[&event.id, &run_id, &event.event_type, &event.message, &now],
                )
                .await
                .map_err(|error| {
                    SessionStoreError::Backend(format!(
                        "failed to insert workflow run cancel event: {error}"
                    ))
                })?;
            PostgresSessionStore::enqueue_workflow_event_deliveries(&transaction, &run, &event)
                .await?;
            transaction
                .execute(
                    r#"
                    INSERT INTO control_workflow_run_logs (
                        id,
                        run_id,
                        stream,
                        message,
                        created_at
                    )
                    VALUES ($1, $2, $3, $4, $5)
                    "#,
                    &[
                        &Uuid::now_v7(),
                        &run_id,
                        &cancellation_plan.run_log_stream.as_str(),
                        &cancellation_plan.run_log_message,
                        &now,
                    ],
                )
                .await
                .map_err(|error| {
                    SessionStoreError::Backend(format!(
                        "failed to insert workflow run cancel log: {error}"
                    ))
                })?;
        }

        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;

        Ok(Some(task))
    }

    pub(in crate::session_control) async fn list_automation_task_events_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Vec<StoredAutomationTaskEvent>, SessionStoreError> {
        if self
            .get_automation_task_for_owner(principal, id)
            .await?
            .is_none()
        {
            return Ok(Vec::new());
        }
        let rows = self
            .store
            .db
            .client()
            .await?
            .query(
                r#"
                SELECT
                    id,
                    task_id,
                    event_type,
                    message,
                    data,
                    created_at
                FROM control_automation_task_events
                WHERE task_id = $1
                ORDER BY created_at ASC, id ASC
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to list automation task events: {error}"
                ))
            })?;
        rows.iter()
            .map(row_to_stored_automation_task_event)
            .collect()
    }

    pub(in crate::session_control) async fn list_automation_task_logs_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Vec<StoredAutomationTaskLog>, SessionStoreError> {
        if self
            .get_automation_task_for_owner(principal, id)
            .await?
            .is_none()
        {
            return Ok(Vec::new());
        }
        let rows = self
            .store
            .db
            .client()
            .await?
            .query(
                r#"
                SELECT
                    id,
                    task_id,
                    stream,
                    message,
                    created_at
                FROM control_automation_task_logs
                WHERE task_id = $1
                ORDER BY created_at ASC, id ASC
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to list automation task logs: {error}"))
            })?;
        rows.iter().map(row_to_stored_automation_task_log).collect()
    }

    pub(in crate::session_control) async fn transition_automation_task(
        &self,
        id: Uuid,
        request: AutomationTaskTransitionRequest,
    ) -> Result<Option<StoredAutomationTask>, SessionStoreError> {
        let mut client = self.store.db.client().await?;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;

        let current_row = transaction
            .query_opt(
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
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to lock automation task for transition: {error}"
                ))
            })?;
        let Some(current_row) = current_row else {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Ok(None);
        };
        let current = row_to_stored_automation_task(&current_row)?;
        let transition_plan = plan_automation_task_transition(&current, &request, Utc::now())
            .map_err(|error| SessionStoreError::Conflict(error.to_string()))?;
        let now = transition_plan.task_updated_at;
        let artifact_refs = json_string_array(&transition_plan.task_artifact_refs);
        let row = transaction
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
                    &id,
                    &transition_plan.task_state.as_str(),
                    &transition_plan.task_output,
                    &transition_plan.task_error,
                    &artifact_refs,
                    &transition_plan.task_started_at,
                    &transition_plan.task_completed_at,
                    &transition_plan.task_updated_at,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to update automation task state: {error}"
                ))
            })?;

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
                    &id,
                    &transition_plan.task_event_type,
                    &transition_plan.task_event_message,
                    &transition_plan.task_event_data,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to insert automation task transition event: {error}"
                ))
            })?;

        let task = row_to_stored_automation_task(&row)?;
        transaction
            .execute(
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
                WHERE automation_task_id = $1
                "#,
                &[
                    &task.id,
                    &WorkflowRunState::from(task.state).as_str(),
                    &task.output,
                    &task.error,
                    &json_string_array(&task.artifact_refs),
                    &task.started_at,
                    &task.completed_at,
                    &task.updated_at,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to sync workflow run after automation task transition: {error}"
                ))
            })?;

        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;

        Ok(Some(task))
    }

    pub(in crate::session_control) async fn append_automation_task_log(
        &self,
        id: Uuid,
        stream: AutomationTaskLogStream,
        message: String,
    ) -> Result<Option<StoredAutomationTaskLog>, SessionStoreError> {
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(
                r#"
                INSERT INTO control_automation_task_logs (
                    id,
                    task_id,
                    stream,
                    message,
                    created_at
                )
                SELECT $2, $1, $3, $4, $5
                WHERE EXISTS (
                    SELECT 1
                    FROM control_automation_tasks
                    WHERE id = $1
                )
                RETURNING
                    id,
                    task_id,
                    stream,
                    message,
                    created_at
                "#,
                &[
                    &id,
                    &Uuid::now_v7(),
                    &stream.as_str(),
                    &message,
                    &Utc::now(),
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to append automation task log: {error}"))
            })?;
        row.as_ref()
            .map(row_to_stored_automation_task_log)
            .transpose()
    }
}
