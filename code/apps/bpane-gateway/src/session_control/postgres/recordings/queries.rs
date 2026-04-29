use super::*;

impl RecordingRepository<'_> {
    pub(in crate::session_control) async fn list_recordings_for_session(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<StoredSessionRecording>, SessionStoreError> {
        let query = format!(
            r#"
            SELECT
                {RECORDING_COLUMNS}
            FROM control_session_recordings
            WHERE session_id = $1
            ORDER BY created_at DESC, updated_at DESC
            "#
        );
        let rows = self
            .store
            .db
            .client()
            .await?
            .query(&query, &[&session_id])
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to list session recordings: {error}"))
            })?;

        rows.iter().map(row_to_stored_session_recording).collect()
    }

    pub(in crate::session_control) async fn get_recording_for_session(
        &self,
        session_id: Uuid,
        recording_id: Uuid,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        let query = format!(
            r#"
            SELECT
                {RECORDING_COLUMNS}
            FROM control_session_recordings
            WHERE session_id = $1 AND id = $2
            "#
        );
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(&query, &[&session_id, &recording_id])
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to load session recording: {error}"))
            })?;

        row.as_ref()
            .map(row_to_stored_session_recording)
            .transpose()
    }

    pub(in crate::session_control) async fn get_latest_recording_for_session(
        &self,
        session_id: Uuid,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        let query = format!(
            r#"
            SELECT
                {RECORDING_COLUMNS}
            FROM control_session_recordings
            WHERE session_id = $1
            ORDER BY updated_at DESC, created_at DESC
            LIMIT 1
            "#
        );
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(&query, &[&session_id])
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to load latest recording: {error}"))
            })?;

        row.as_ref()
            .map(row_to_stored_session_recording)
            .transpose()
    }
}
