use super::*;

pub(super) struct RuntimeAssignmentRepository<'a> {
    store: &'a PostgresSessionStore,
}

impl PostgresSessionStore {
    fn runtime_assignment_repository(&self) -> RuntimeAssignmentRepository<'_> {
        RuntimeAssignmentRepository { store: self }
    }

    pub(in crate::session_control) async fn upsert_runtime_assignment(
        &self,
        assignment: PersistedSessionRuntimeAssignment,
    ) -> Result<(), SessionStoreError> {
        self.runtime_assignment_repository()
            .upsert_runtime_assignment(assignment)
            .await
    }

    pub(in crate::session_control) async fn clear_runtime_assignment(
        &self,
        id: Uuid,
    ) -> Result<(), SessionStoreError> {
        self.runtime_assignment_repository()
            .clear_runtime_assignment(id)
            .await
    }

    pub(in crate::session_control) async fn upsert_recording_worker_assignment(
        &self,
        assignment: PersistedSessionRecordingWorkerAssignment,
    ) -> Result<(), SessionStoreError> {
        self.runtime_assignment_repository()
            .upsert_recording_worker_assignment(assignment)
            .await
    }

    pub(in crate::session_control) async fn clear_recording_worker_assignment(
        &self,
        id: Uuid,
    ) -> Result<(), SessionStoreError> {
        self.runtime_assignment_repository()
            .clear_recording_worker_assignment(id)
            .await
    }

    pub(in crate::session_control) async fn get_recording_worker_assignment(
        &self,
        id: Uuid,
    ) -> Result<Option<PersistedSessionRecordingWorkerAssignment>, SessionStoreError> {
        self.runtime_assignment_repository()
            .get_recording_worker_assignment(id)
            .await
    }

    pub(in crate::session_control) async fn list_recording_worker_assignments(
        &self,
    ) -> Result<Vec<PersistedSessionRecordingWorkerAssignment>, SessionStoreError> {
        self.runtime_assignment_repository()
            .list_recording_worker_assignments()
            .await
    }

    pub(in crate::session_control) async fn upsert_workflow_run_worker_assignment(
        &self,
        assignment: PersistedWorkflowRunWorkerAssignment,
    ) -> Result<(), SessionStoreError> {
        self.runtime_assignment_repository()
            .upsert_workflow_run_worker_assignment(assignment)
            .await
    }

    pub(in crate::session_control) async fn clear_workflow_run_worker_assignment(
        &self,
        run_id: Uuid,
    ) -> Result<(), SessionStoreError> {
        self.runtime_assignment_repository()
            .clear_workflow_run_worker_assignment(run_id)
            .await
    }

    pub(in crate::session_control) async fn get_workflow_run_worker_assignment(
        &self,
        run_id: Uuid,
    ) -> Result<Option<PersistedWorkflowRunWorkerAssignment>, SessionStoreError> {
        self.runtime_assignment_repository()
            .get_workflow_run_worker_assignment(run_id)
            .await
    }

    pub(in crate::session_control) async fn list_workflow_run_worker_assignments(
        &self,
    ) -> Result<Vec<PersistedWorkflowRunWorkerAssignment>, SessionStoreError> {
        self.runtime_assignment_repository()
            .list_workflow_run_worker_assignments()
            .await
    }

    pub(in crate::session_control) async fn list_runtime_assignments(
        &self,
        runtime_binding: &str,
    ) -> Result<Vec<PersistedSessionRuntimeAssignment>, SessionStoreError> {
        self.runtime_assignment_repository()
            .list_runtime_assignments(runtime_binding)
            .await
    }

    pub(in crate::session_control) async fn mark_session_ready_after_runtime_loss(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        self.runtime_assignment_repository()
            .mark_session_ready_after_runtime_loss(id)
            .await
    }
}

impl RuntimeAssignmentRepository<'_> {
    pub(in crate::session_control) async fn upsert_runtime_assignment(
        &self,
        assignment: PersistedSessionRuntimeAssignment,
    ) -> Result<(), SessionStoreError> {
        self.store
            .db
            .client()
            .await?
            .execute(
                r#"
                INSERT INTO control_session_runtimes (
                    session_id,
                    runtime_binding,
                    status,
                    agent_socket_path,
                    container_name,
                    cdp_endpoint,
                    created_at,
                    updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, NOW(), NOW())
                ON CONFLICT (session_id)
                DO UPDATE SET
                    runtime_binding = EXCLUDED.runtime_binding,
                    status = EXCLUDED.status,
                    agent_socket_path = EXCLUDED.agent_socket_path,
                    container_name = EXCLUDED.container_name,
                    cdp_endpoint = EXCLUDED.cdp_endpoint,
                    updated_at = NOW()
                "#,
                &[
                    &assignment.session_id,
                    &assignment.runtime_binding,
                    &assignment.status.as_str(),
                    &assignment.agent_socket_path,
                    &assignment.container_name,
                    &assignment.cdp_endpoint,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to upsert runtime assignment: {error}"))
            })?;
        Ok(())
    }

    pub(in crate::session_control) async fn clear_runtime_assignment(
        &self,
        id: Uuid,
    ) -> Result<(), SessionStoreError> {
        self.store
            .db
            .client()
            .await?
            .execute(
                "DELETE FROM control_session_runtimes WHERE session_id = $1",
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to clear runtime assignment: {error}"))
            })?;
        Ok(())
    }

    pub(in crate::session_control) async fn upsert_recording_worker_assignment(
        &self,
        assignment: PersistedSessionRecordingWorkerAssignment,
    ) -> Result<(), SessionStoreError> {
        let process_id = assignment.process_id.map(i64::from);
        self.store
            .db
            .client()
            .await?
            .execute(
                r#"
                INSERT INTO control_session_recording_workers (
                    session_id,
                    recording_id,
                    status,
                    process_id,
                    created_at,
                    updated_at
                )
                VALUES ($1, $2, $3, $4, NOW(), NOW())
                ON CONFLICT (session_id)
                DO UPDATE SET
                    recording_id = EXCLUDED.recording_id,
                    status = EXCLUDED.status,
                    process_id = EXCLUDED.process_id,
                    updated_at = NOW()
                "#,
                &[
                    &assignment.session_id,
                    &assignment.recording_id,
                    &assignment.status.as_str(),
                    &process_id,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to upsert recording worker assignment: {error}"
                ))
            })?;
        Ok(())
    }

    pub(in crate::session_control) async fn clear_recording_worker_assignment(
        &self,
        id: Uuid,
    ) -> Result<(), SessionStoreError> {
        self.store
            .db
            .client()
            .await?
            .execute(
                "DELETE FROM control_session_recording_workers WHERE session_id = $1",
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to clear recording worker assignment: {error}"
                ))
            })?;
        Ok(())
    }

    pub(in crate::session_control) async fn get_recording_worker_assignment(
        &self,
        id: Uuid,
    ) -> Result<Option<PersistedSessionRecordingWorkerAssignment>, SessionStoreError> {
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(
                r#"
                SELECT
                    session_id,
                    recording_id,
                    status,
                    process_id
                FROM control_session_recording_workers
                WHERE session_id = $1
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to load recording worker assignment: {error}"
                ))
            })?;
        row.as_ref()
            .map(row_to_recording_worker_assignment)
            .transpose()
    }

    pub(in crate::session_control) async fn list_recording_worker_assignments(
        &self,
    ) -> Result<Vec<PersistedSessionRecordingWorkerAssignment>, SessionStoreError> {
        let rows = self
            .store
            .db
            .client()
            .await?
            .query(
                r#"
                SELECT
                    session_id,
                    recording_id,
                    status,
                    process_id
                FROM control_session_recording_workers
                ORDER BY updated_at DESC, created_at DESC
                "#,
                &[],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to list recording worker assignments: {error}"
                ))
            })?;

        rows.iter()
            .map(row_to_recording_worker_assignment)
            .collect()
    }

    pub(in crate::session_control) async fn upsert_workflow_run_worker_assignment(
        &self,
        assignment: PersistedWorkflowRunWorkerAssignment,
    ) -> Result<(), SessionStoreError> {
        let process_id = assignment.process_id.map(i64::from);
        self.store
            .db
            .client()
            .await?
            .execute(
                r#"
                INSERT INTO control_workflow_run_workers (
                    run_id,
                    session_id,
                    automation_task_id,
                    status,
                    process_id,
                    container_name,
                    created_at,
                    updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, NOW(), NOW())
                ON CONFLICT (run_id)
                DO UPDATE SET
                    session_id = EXCLUDED.session_id,
                    automation_task_id = EXCLUDED.automation_task_id,
                    status = EXCLUDED.status,
                    process_id = EXCLUDED.process_id,
                    container_name = EXCLUDED.container_name,
                    updated_at = NOW()
                "#,
                &[
                    &assignment.run_id,
                    &assignment.session_id,
                    &assignment.automation_task_id,
                    &assignment.status.as_str(),
                    &process_id,
                    &assignment.container_name,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to upsert workflow run worker assignment: {error}"
                ))
            })?;
        Ok(())
    }

    pub(in crate::session_control) async fn clear_workflow_run_worker_assignment(
        &self,
        run_id: Uuid,
    ) -> Result<(), SessionStoreError> {
        self.store
            .db
            .client()
            .await?
            .execute(
                "DELETE FROM control_workflow_run_workers WHERE run_id = $1",
                &[&run_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to clear workflow run worker assignment: {error}"
                ))
            })?;
        Ok(())
    }

    pub(in crate::session_control) async fn get_workflow_run_worker_assignment(
        &self,
        run_id: Uuid,
    ) -> Result<Option<PersistedWorkflowRunWorkerAssignment>, SessionStoreError> {
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(
                r#"
                SELECT
                    run_id,
                    session_id,
                    automation_task_id,
                    status,
                    process_id,
                    container_name
                FROM control_workflow_run_workers
                WHERE run_id = $1
                "#,
                &[&run_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to load workflow run worker assignment: {error}"
                ))
            })?;
        row.as_ref()
            .map(row_to_workflow_run_worker_assignment)
            .transpose()
    }

    pub(in crate::session_control) async fn list_workflow_run_worker_assignments(
        &self,
    ) -> Result<Vec<PersistedWorkflowRunWorkerAssignment>, SessionStoreError> {
        let rows = self
            .store
            .db
            .client()
            .await?
            .query(
                r#"
                SELECT
                    run_id,
                    session_id,
                    automation_task_id,
                    status,
                    process_id,
                    container_name
                FROM control_workflow_run_workers
                ORDER BY updated_at DESC, created_at DESC
                "#,
                &[],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to list workflow run worker assignments: {error}"
                ))
            })?;

        rows.iter()
            .map(row_to_workflow_run_worker_assignment)
            .collect()
    }

    pub(in crate::session_control) async fn list_runtime_assignments(
        &self,
        runtime_binding: &str,
    ) -> Result<Vec<PersistedSessionRuntimeAssignment>, SessionStoreError> {
        let rows = self
            .store
            .db
            .client()
            .await?
            .query(
                r#"
                SELECT
                    session_id,
                    runtime_binding,
                    status,
                    agent_socket_path,
                    container_name,
                    cdp_endpoint
                FROM control_session_runtimes
                WHERE runtime_binding = $1
                ORDER BY updated_at DESC, created_at DESC
                "#,
                &[&runtime_binding],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to list runtime assignments: {error}"))
            })?;

        rows.iter().map(row_to_runtime_assignment).collect()
    }

    pub(in crate::session_control) async fn mark_session_ready_after_runtime_loss(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(
                r#"
                UPDATE control_sessions
                SET
                    state = 'ready',
                    updated_at = NOW()
                WHERE id = $1
                  AND state IN ('pending', 'starting', 'ready', 'active', 'idle')
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    automation_owner_client_id,
                    automation_owner_issuer,
                    automation_owner_display_name,
                    state,
                    template_id,
                    owner_mode,
                    viewport_width,
                    viewport_height,
                    idle_timeout_sec,
                    labels,
                    integration_context,
                    extensions,
                    recording,
                    created_at,
                    updated_at,
                    stopped_at
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to restore session to ready after runtime loss: {error}"
                ))
            })?;
        row.as_ref().map(row_to_stored_session).transpose()
    }
}
