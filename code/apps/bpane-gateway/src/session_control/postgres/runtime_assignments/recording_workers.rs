use super::*;

impl RuntimeAssignmentRepository<'_> {
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
        let query = format!(
            r#"
            SELECT
                {RECORDING_WORKER_COLUMNS}
            FROM control_session_recording_workers
            WHERE session_id = $1
            "#
        );
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(&query, &[&id])
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
        let query = format!(
            r#"
            SELECT
                {RECORDING_WORKER_COLUMNS}
            FROM control_session_recording_workers
            ORDER BY updated_at DESC, created_at DESC
            "#
        );
        let rows = self
            .store
            .db
            .client()
            .await?
            .query(&query, &[])
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
}
