use super::*;

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

    pub(in crate::session_control) async fn list_runtime_assignments(
        &self,
        runtime_binding: &str,
    ) -> Result<Vec<PersistedSessionRuntimeAssignment>, SessionStoreError> {
        let query = format!(
            r#"
            SELECT
                {RUNTIME_ASSIGNMENT_COLUMNS}
            FROM control_session_runtimes
            WHERE runtime_binding = $1
            ORDER BY updated_at DESC, created_at DESC
            "#
        );
        let rows = self
            .store
            .db
            .client()
            .await?
            .query(&query, &[&runtime_binding])
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
        let update_query = format!(
            r#"
            UPDATE control_sessions
            SET
                state = 'ready',
                updated_at = NOW()
            WHERE id = $1
              AND state IN ('pending', 'starting', 'ready', 'active', 'idle')
            RETURNING
                {SESSION_RECOVERY_COLUMNS}
            "#
        );
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(&update_query, &[&id])
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to restore session to ready after runtime loss: {error}"
                ))
            })?;
        row.as_ref().map(row_to_stored_session).transpose()
    }
}
