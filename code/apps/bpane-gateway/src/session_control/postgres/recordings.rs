use super::*;

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

impl RecordingRepository<'_> {
    pub(in crate::session_control) async fn create_recording_for_session(
        &self,
        session_id: Uuid,
        format: SessionRecordingFormat,
        previous_recording_id: Option<Uuid>,
    ) -> Result<StoredSessionRecording, SessionStoreError> {
        let mut client = self.store.db.client().await?;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;

        let active = transaction
            .query_opt(
                r#"
                SELECT id
                FROM control_session_recordings
                WHERE session_id = $1
                  AND state IN ('starting', 'recording', 'finalizing')
                ORDER BY updated_at DESC, created_at DESC
                LIMIT 1
                FOR UPDATE
                "#,
                &[&session_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to check active recordings: {error}"))
            })?;
        if let Some(active) = active {
            let active_id: Uuid = active.get("id");
            return Err(SessionStoreError::Conflict(format!(
                "session {session_id} already has active recording {active_id}"
            )));
        }

        let now = Utc::now();
        let recording_id = Uuid::now_v7();
        let row = transaction
            .query_one(
                r#"
                INSERT INTO control_session_recordings (
                    id,
                    session_id,
                    previous_recording_id,
                    state,
                    format,
                    mime_type,
                    started_at,
                    created_at,
                    updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $7, $7)
                RETURNING
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
                "#,
                &[
                    &recording_id,
                    &session_id,
                    &previous_recording_id,
                    &SessionRecordingState::Recording.as_str(),
                    &format.as_str(),
                    &recording_mime_type(format),
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to insert recording: {error}"))
            })?;

        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;

        row_to_stored_session_recording(&row)
    }

    pub(in crate::session_control) async fn list_recordings_for_session(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<StoredSessionRecording>, SessionStoreError> {
        let rows = self
            .store
            .db
            .client()
            .await?
            .query(
                r#"
                SELECT
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
                FROM control_session_recordings
                WHERE session_id = $1
                ORDER BY created_at DESC, updated_at DESC
                "#,
                &[&session_id],
            )
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
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(
                r#"
                SELECT
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
                FROM control_session_recordings
                WHERE session_id = $1 AND id = $2
                "#,
                &[&session_id, &recording_id],
            )
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
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(
                r#"
                SELECT
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
                FROM control_session_recordings
                WHERE session_id = $1
                ORDER BY updated_at DESC, created_at DESC
                LIMIT 1
                "#,
                &[&session_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to load latest recording: {error}"))
            })?;
        row.as_ref()
            .map(row_to_stored_session_recording)
            .transpose()
    }

    pub(in crate::session_control) async fn list_recording_artifact_retention_candidates(
        &self,
        now: DateTime<Utc>,
    ) -> Result<Vec<RecordingArtifactRetentionCandidate>, SessionStoreError> {
        let rows = self
            .store
            .db
            .client()
            .await?
            .query(
                r#"
                SELECT
                    r.session_id,
                    r.id AS recording_id,
                    r.artifact_path AS artifact_ref,
                    r.completed_at,
                    ((s.recording ->> 'retention_sec')::INTEGER) AS retention_sec
                FROM control_session_recordings r
                INNER JOIN control_sessions s
                    ON s.id = r.session_id
                WHERE r.state = 'ready'
                  AND r.artifact_path IS NOT NULL
                  AND r.completed_at IS NOT NULL
                  AND (s.recording ->> 'retention_sec') IS NOT NULL
                ORDER BY r.completed_at ASC, r.created_at ASC
                "#,
                &[],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to list recording artifact retention candidates: {error}"
                ))
            })?;

        let mut candidates = rows
            .iter()
            .filter_map(|row| {
                let completed_at = row.get::<_, DateTime<Utc>>("completed_at");
                let retention_sec = row.get::<_, i32>("retention_sec");
                let expires_at = completed_at + ChronoDuration::seconds(i64::from(retention_sec));
                if expires_at > now {
                    return None;
                }
                Some(RecordingArtifactRetentionCandidate {
                    session_id: row.get("session_id"),
                    recording_id: row.get("recording_id"),
                    artifact_ref: row.get("artifact_ref"),
                    expires_at,
                })
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| left.expires_at.cmp(&right.expires_at));
        Ok(candidates)
    }

    pub(in crate::session_control) async fn stop_recording_for_session(
        &self,
        session_id: Uuid,
        recording_id: Uuid,
        termination_reason: SessionRecordingTerminationReason,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(
                r#"
                UPDATE control_session_recordings
                SET
                    state = 'finalizing',
                    termination_reason = $3,
                    updated_at = NOW()
                WHERE session_id = $1
                  AND id = $2
                  AND state IN ('starting', 'recording', 'finalizing')
                RETURNING
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
                "#,
                &[&session_id, &recording_id, &termination_reason.as_str()],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to stop recording: {error}"))
            })?;
        if let Some(row) = row {
            return row_to_stored_session_recording(&row).map(Some);
        }

        let existing = self
            .get_recording_for_session(session_id, recording_id)
            .await?;
        if existing.is_some() {
            return Err(SessionStoreError::Conflict(format!(
                "recording {recording_id} is not active"
            )));
        }
        Ok(None)
    }

    pub(in crate::session_control) async fn complete_recording_for_session(
        &self,
        session_id: Uuid,
        recording_id: Uuid,
        request: PersistCompletedSessionRecordingRequest,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(
                r#"
                UPDATE control_session_recordings
                SET
                    state = 'ready',
                    artifact_path = $3,
                    mime_type = COALESCE($4, mime_type),
                    byte_count = $5,
                    duration_ms = $6,
                    error = NULL,
                    completed_at = NOW(),
                    updated_at = NOW()
                WHERE session_id = $1
                  AND id = $2
                  AND state IN ('starting', 'recording', 'finalizing')
                RETURNING
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
                "#,
                &[
                    &session_id,
                    &recording_id,
                    &request.artifact_ref,
                    &request.mime_type,
                    &request.bytes.map(|value| value as i64),
                    &request.duration_ms.map(|value| value as i64),
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to complete recording: {error}"))
            })?;
        if let Some(row) = row {
            return row_to_stored_session_recording(&row).map(Some);
        }

        let existing = self
            .get_recording_for_session(session_id, recording_id)
            .await?;
        if existing.is_some() {
            return Err(SessionStoreError::Conflict(format!(
                "recording {recording_id} is not active"
            )));
        }
        Ok(None)
    }

    pub(in crate::session_control) async fn clear_recording_artifact_path(
        &self,
        session_id: Uuid,
        recording_id: Uuid,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(
                r#"
                UPDATE control_session_recordings
                SET
                    artifact_path = NULL,
                    updated_at = NOW()
                WHERE session_id = $1
                  AND id = $2
                RETURNING
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
                "#,
                &[&session_id, &recording_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to clear recording artifact path: {error}"
                ))
            })?;
        row.as_ref()
            .map(row_to_stored_session_recording)
            .transpose()
    }

    pub(in crate::session_control) async fn fail_recording_for_session(
        &self,
        session_id: Uuid,
        recording_id: Uuid,
        request: FailSessionRecordingRequest,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(
                r#"
                UPDATE control_session_recordings
                SET
                    state = 'failed',
                    error = $3,
                    termination_reason = $4,
                    completed_at = NOW(),
                    updated_at = NOW()
                WHERE session_id = $1
                  AND id = $2
                  AND state IN ('starting', 'recording', 'finalizing', 'failed')
                RETURNING
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
                "#,
                &[
                    &session_id,
                    &recording_id,
                    &request.error,
                    &request
                        .termination_reason
                        .map(|reason| reason.as_str().to_string()),
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to fail recording: {error}"))
            })?;
        if let Some(row) = row {
            return row_to_stored_session_recording(&row).map(Some);
        }

        let existing = self
            .get_recording_for_session(session_id, recording_id)
            .await?;
        if let Some(existing) = existing {
            if matches!(existing.state, SessionRecordingState::Ready) {
                return Err(SessionStoreError::Conflict(format!(
                    "recording {recording_id} is already complete"
                )));
            }
        } else {
            return Ok(None);
        }

        self.get_recording_for_session(session_id, recording_id)
            .await
    }
}
