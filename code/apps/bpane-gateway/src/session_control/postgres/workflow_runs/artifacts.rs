use super::*;

impl PostgresSessionStore {
    pub(in crate::session_control) async fn append_workflow_run_produced_file(
        &self,
        id: Uuid,
        request: PersistWorkflowRunProducedFileRequest,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        let mut client = self.db.client().await?;
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
                    "failed to lock workflow run for produced file append: {error}"
                ))
            })?;
        let Some(run_row) = run_row else {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Ok(None);
        };

        let mut run = row_to_stored_workflow_run(&run_row)?;
        if run
            .produced_files
            .iter()
            .any(|file| file.file_id == request.file_id)
        {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Err(SessionStoreError::Conflict(format!(
                "workflow run {id} already contains produced file {}",
                request.file_id
            )));
        }

        let now = Utc::now();
        let produced_file = WorkflowRunProducedFile {
            workspace_id: request.workspace_id,
            file_id: request.file_id,
            file_name: request.file_name,
            media_type: request.media_type,
            byte_count: request.byte_count,
            sha256_hex: request.sha256_hex,
            provenance: request.provenance,
            artifact_ref: request.artifact_ref,
            created_at: now,
        };
        run.produced_files.push(produced_file.clone());

        let row = transaction
            .query_one(
                r#"
                UPDATE control_workflow_runs
                SET
                    produced_files = $2::jsonb,
                    updated_at = $3
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
                    &json_workflow_run_produced_files(&run.produced_files)?,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to update workflow run produced files: {error}"
                ))
            })?;

        let event = StoredWorkflowRunEvent {
            id: Uuid::now_v7(),
            run_id: id,
            event_type: "workflow_run.produced_file_added".to_string(),
            message: format!(
                "workflow run produced file {} stored in workspace {}",
                produced_file.file_id, produced_file.workspace_id
            ),
            data: Some(serde_json::json!({
                "workspace_id": produced_file.workspace_id,
                "file_id": produced_file.file_id,
                "file_name": produced_file.file_name,
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
                    "failed to insert workflow produced file event: {error}"
                ))
            })?;
        let updated_run = row_to_stored_workflow_run(&row)?;
        Self::enqueue_workflow_event_deliveries(&transaction, &updated_run, &event).await?;

        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;

        Ok(Some(updated_run))
    }

    pub(in crate::session_control) async fn list_workflow_run_log_retention_candidates(
        &self,
        now: DateTime<Utc>,
        retention: ChronoDuration,
    ) -> Result<Vec<WorkflowRunLogRetentionCandidate>, SessionStoreError> {
        let retention_secs = retention.num_seconds() as f64;
        let rows = self
            .db
            .client()
            .await?
            .query(
                r#"
                SELECT
                    run.id AS run_id,
                    run.automation_task_id,
                    run.session_id,
                    run.completed_at
                FROM control_workflow_runs run
                WHERE run.completed_at IS NOT NULL
                  AND EXTRACT(EPOCH FROM ($1 - run.completed_at)) >= $2::DOUBLE PRECISION
                  AND (
                    EXISTS (
                        SELECT 1
                        FROM control_workflow_run_logs logs
                        WHERE logs.run_id = run.id
                    )
                    OR EXISTS (
                        SELECT 1
                        FROM control_automation_task_logs logs
                        WHERE logs.task_id = run.automation_task_id
                    )
                  )
                ORDER BY run.completed_at ASC, run.id ASC
                "#,
                &[&now, &retention_secs],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to list workflow run log retention candidates: {}",
                    describe_postgres_error(&error)
                ))
            })?;
        Ok(rows
            .iter()
            .map(|row| {
                let completed_at: DateTime<Utc> = row.get("completed_at");
                WorkflowRunLogRetentionCandidate {
                    run_id: row.get("run_id"),
                    automation_task_id: row.get("automation_task_id"),
                    session_id: row.get("session_id"),
                    expires_at: completed_at + retention,
                }
            })
            .collect())
    }

    pub(in crate::session_control) async fn delete_workflow_run_logs(
        &self,
        run_id: Uuid,
        automation_task_id: Uuid,
    ) -> Result<usize, SessionStoreError> {
        let mut client = self.db.client().await?;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;
        let run_deleted = transaction
            .execute(
                "DELETE FROM control_workflow_run_logs WHERE run_id = $1",
                &[&run_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to delete workflow run logs for {run_id}: {error}"
                ))
            })? as usize;
        let task_deleted = transaction
            .execute(
                "DELETE FROM control_automation_task_logs WHERE task_id = $1",
                &[&automation_task_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to delete automation task logs for {automation_task_id}: {error}"
                ))
            })? as usize;
        let now = Utc::now();
        transaction
            .execute(
                "UPDATE control_workflow_runs SET updated_at = $2 WHERE id = $1",
                &[&run_id, &now],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to update workflow run after log deletion: {error}"
                ))
            })?;
        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;
        Ok(run_deleted + task_deleted)
    }

    pub(in crate::session_control) async fn list_workflow_run_output_retention_candidates(
        &self,
        now: DateTime<Utc>,
        retention: ChronoDuration,
    ) -> Result<Vec<WorkflowRunOutputRetentionCandidate>, SessionStoreError> {
        let retention_secs = retention.num_seconds() as f64;
        let rows = self
            .db
            .client()
            .await?
            .query(
                r#"
                SELECT
                    run.id AS run_id,
                    run.session_id,
                    run.completed_at
                FROM control_workflow_runs run
                WHERE run.completed_at IS NOT NULL
                  AND run.output IS NOT NULL
                  AND EXTRACT(EPOCH FROM ($1 - run.completed_at)) >= $2::DOUBLE PRECISION
                ORDER BY run.completed_at ASC, run.id ASC
                "#,
                &[&now, &retention_secs],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to list workflow run output retention candidates: {}",
                    describe_postgres_error(&error)
                ))
            })?;
        Ok(rows
            .iter()
            .map(|row| {
                let completed_at: DateTime<Utc> = row.get("completed_at");
                WorkflowRunOutputRetentionCandidate {
                    run_id: row.get("run_id"),
                    session_id: row.get("session_id"),
                    expires_at: completed_at + retention,
                }
            })
            .collect())
    }

    pub(in crate::session_control) async fn clear_workflow_run_output(
        &self,
        run_id: Uuid,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        let row = self
            .db
            .client()
            .await?
            .query_opt(
                r#"
                UPDATE control_workflow_runs
                SET
                    output = NULL,
                    updated_at = $2
                WHERE id = $1
                RETURNING
                    id,
                    workflow_definition_id,
                    workflow_definition_version_id,
                    workflow_version,
                    session_id,
                    automation_task_id,
                    state,
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
                &[&run_id, &Utc::now()],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to clear workflow run output: {error}"))
            })?;
        row.as_ref().map(row_to_stored_workflow_run).transpose()
    }
}
