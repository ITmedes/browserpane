use super::*;

mod queries;
mod retention;
mod state;

const RECORDING_COLUMNS: &str = r#"
    id,
    session_id,
    previous_recording_id,
    state,
    format,
    mime_type,
    byte_count,
    duration_ms,
    error,
    termination_reason,
    artifact_path AS artifact_ref,
    started_at,
    completed_at,
    created_at,
    updated_at
"#;

pub(super) struct RecordingRepository<'a> {
    store: &'a PostgresSessionStore,
}

impl PostgresSessionStore {
    fn recording_repository(&self) -> RecordingRepository<'_> {
        RecordingRepository { store: self }
    }

    pub(in crate::session_control) async fn create_recording_for_session(
        &self,
        session_id: Uuid,
        format: SessionRecordingFormat,
        previous_recording_id: Option<Uuid>,
    ) -> Result<StoredSessionRecording, SessionStoreError> {
        self.recording_repository()
            .create_recording_for_session(session_id, format, previous_recording_id)
            .await
    }

    pub(in crate::session_control) async fn list_recordings_for_session(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<StoredSessionRecording>, SessionStoreError> {
        self.recording_repository()
            .list_recordings_for_session(session_id)
            .await
    }

    pub(in crate::session_control) async fn get_recording_for_session(
        &self,
        session_id: Uuid,
        recording_id: Uuid,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        self.recording_repository()
            .get_recording_for_session(session_id, recording_id)
            .await
    }

    pub(in crate::session_control) async fn get_latest_recording_for_session(
        &self,
        session_id: Uuid,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        self.recording_repository()
            .get_latest_recording_for_session(session_id)
            .await
    }

    pub(in crate::session_control) async fn list_recording_artifact_retention_candidates(
        &self,
        now: DateTime<Utc>,
    ) -> Result<Vec<RecordingArtifactRetentionCandidate>, SessionStoreError> {
        self.recording_repository()
            .list_recording_artifact_retention_candidates(now)
            .await
    }

    pub(in crate::session_control) async fn stop_recording_for_session(
        &self,
        session_id: Uuid,
        recording_id: Uuid,
        termination_reason: SessionRecordingTerminationReason,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        self.recording_repository()
            .stop_recording_for_session(session_id, recording_id, termination_reason)
            .await
    }

    pub(in crate::session_control) async fn complete_recording_for_session(
        &self,
        session_id: Uuid,
        recording_id: Uuid,
        request: PersistCompletedSessionRecordingRequest,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        self.recording_repository()
            .complete_recording_for_session(session_id, recording_id, request)
            .await
    }

    pub(in crate::session_control) async fn clear_recording_artifact_path(
        &self,
        session_id: Uuid,
        recording_id: Uuid,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        self.recording_repository()
            .clear_recording_artifact_path(session_id, recording_id)
            .await
    }

    pub(in crate::session_control) async fn fail_recording_for_session(
        &self,
        session_id: Uuid,
        recording_id: Uuid,
        request: FailSessionRecordingRequest,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        self.recording_repository()
            .fail_recording_for_session(session_id, recording_id, request)
            .await
    }
}
