use super::*;

impl InMemorySessionStore {
    pub(in crate::session_control) async fn create_recording_for_session(
        &self,
        session_id: Uuid,
        format: SessionRecordingFormat,
        previous_recording_id: Option<Uuid>,
    ) -> Result<StoredSessionRecording, SessionStoreError> {
        let mut recordings = self.recordings.lock().await;
        if let Some(active) = recordings
            .iter()
            .find(|recording| recording.session_id == session_id && recording.state.is_active())
        {
            return Err(SessionStoreError::Conflict(format!(
                "session {session_id} already has active recording {}",
                active.id
            )));
        }

        let now = Utc::now();
        let recording = StoredSessionRecording {
            id: Uuid::now_v7(),
            session_id,
            previous_recording_id,
            state: SessionRecordingState::Recording,
            format,
            mime_type: Some(recording_mime_type(format).to_string()),
            bytes: None,
            duration_ms: None,
            error: None,
            termination_reason: None,
            artifact_ref: None,
            started_at: now,
            completed_at: None,
            created_at: now,
            updated_at: now,
        };
        recordings.push(recording.clone());
        Ok(recording)
    }

    pub(in crate::session_control) async fn list_recordings_for_session(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<StoredSessionRecording>, SessionStoreError> {
        let mut recordings = self
            .recordings
            .lock()
            .await
            .iter()
            .filter(|recording| recording.session_id == session_id)
            .cloned()
            .collect::<Vec<_>>();
        recordings.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(recordings)
    }

    pub(in crate::session_control) async fn get_recording_for_session(
        &self,
        session_id: Uuid,
        recording_id: Uuid,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        Ok(self
            .recordings
            .lock()
            .await
            .iter()
            .find(|recording| recording.session_id == session_id && recording.id == recording_id)
            .cloned())
    }

    pub(in crate::session_control) async fn get_latest_recording_for_session(
        &self,
        session_id: Uuid,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        Ok(self
            .recordings
            .lock()
            .await
            .iter()
            .filter(|recording| recording.session_id == session_id)
            .max_by(|left, right| {
                left.updated_at
                    .cmp(&right.updated_at)
                    .then_with(|| left.created_at.cmp(&right.created_at))
            })
            .cloned())
    }

    pub(in crate::session_control) async fn list_recording_artifact_retention_candidates(
        &self,
        now: DateTime<Utc>,
    ) -> Result<Vec<RecordingArtifactRetentionCandidate>, SessionStoreError> {
        let sessions = self.sessions.lock().await;
        let session_retention = sessions
            .iter()
            .filter_map(|session| {
                session
                    .recording
                    .retention_sec
                    .map(|retention| (session.id, retention))
            })
            .collect::<HashMap<_, _>>();
        let recordings = self.recordings.lock().await;
        let mut candidates = recordings
            .iter()
            .filter_map(|recording| {
                if recording.state != SessionRecordingState::Ready {
                    return None;
                }
                let artifact_ref = recording.artifact_ref.clone()?;
                let completed_at = recording.completed_at?;
                let retention_sec = *session_retention.get(&recording.session_id)?;
                let expires_at = completed_at + ChronoDuration::seconds(i64::from(retention_sec));
                if expires_at > now {
                    return None;
                }
                Some(RecordingArtifactRetentionCandidate {
                    session_id: recording.session_id,
                    recording_id: recording.id,
                    artifact_ref,
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
        let mut recordings = self.recordings.lock().await;
        let Some(recording) = recordings
            .iter_mut()
            .find(|recording| recording.session_id == session_id && recording.id == recording_id)
        else {
            return Ok(None);
        };

        if !recording.state.is_active() {
            return Err(SessionStoreError::Conflict(format!(
                "recording {recording_id} is not active"
            )));
        }

        recording.state = SessionRecordingState::Finalizing;
        recording.termination_reason = Some(termination_reason);
        recording.updated_at = Utc::now();
        Ok(Some(recording.clone()))
    }

    pub(in crate::session_control) async fn complete_recording_for_session(
        &self,
        session_id: Uuid,
        recording_id: Uuid,
        request: PersistCompletedSessionRecordingRequest,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        let mut recordings = self.recordings.lock().await;
        let Some(recording) = recordings
            .iter_mut()
            .find(|recording| recording.session_id == session_id && recording.id == recording_id)
        else {
            return Ok(None);
        };

        if !recording.state.is_active() {
            return Err(SessionStoreError::Conflict(format!(
                "recording {recording_id} is not active"
            )));
        }

        let now = Utc::now();
        recording.state = SessionRecordingState::Ready;
        recording.artifact_ref = Some(request.artifact_ref);
        recording.mime_type = request
            .mime_type
            .or_else(|| recording.mime_type.clone())
            .or_else(|| Some(recording_mime_type(recording.format).to_string()));
        recording.bytes = request.bytes;
        recording.duration_ms = request.duration_ms;
        recording.error = None;
        recording.completed_at = Some(now);
        recording.updated_at = now;
        Ok(Some(recording.clone()))
    }

    pub(in crate::session_control) async fn clear_recording_artifact_path(
        &self,
        session_id: Uuid,
        recording_id: Uuid,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        let mut recordings = self.recordings.lock().await;
        let Some(recording) = recordings
            .iter_mut()
            .find(|recording| recording.session_id == session_id && recording.id == recording_id)
        else {
            return Ok(None);
        };

        recording.artifact_ref = None;
        recording.updated_at = Utc::now();
        Ok(Some(recording.clone()))
    }

    pub(in crate::session_control) async fn fail_recording_for_session(
        &self,
        session_id: Uuid,
        recording_id: Uuid,
        request: FailSessionRecordingRequest,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        let mut recordings = self.recordings.lock().await;
        let Some(recording) = recordings
            .iter_mut()
            .find(|recording| recording.session_id == session_id && recording.id == recording_id)
        else {
            return Ok(None);
        };

        if matches!(recording.state, SessionRecordingState::Ready) {
            return Err(SessionStoreError::Conflict(format!(
                "recording {recording_id} is already complete"
            )));
        }

        let now = Utc::now();
        recording.state = SessionRecordingState::Failed;
        recording.error = Some(request.error);
        recording.termination_reason = request.termination_reason;
        recording.completed_at = Some(now);
        recording.updated_at = now;
        Ok(Some(recording.clone()))
    }
}
