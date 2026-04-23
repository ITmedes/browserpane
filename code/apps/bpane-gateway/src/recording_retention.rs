use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use tokio::time::sleep;
use tracing::{info, warn};

use crate::recording_artifact_store::RecordingArtifactStore;
use crate::session_control::{
    RecordingArtifactRetentionCandidate, SessionStore, SessionStoreError,
};

#[derive(Clone)]
pub struct RecordingRetentionManager {
    artifact_store: Arc<RecordingArtifactStore>,
    session_store: SessionStore,
    interval: Duration,
}

impl RecordingRetentionManager {
    pub fn new(
        session_store: SessionStore,
        artifact_store: Arc<RecordingArtifactStore>,
        interval: Duration,
    ) -> Self {
        Self {
            artifact_store,
            session_store,
            interval,
        }
    }

    pub fn start(self: Arc<Self>) {
        tokio::spawn(async move {
            loop {
                sleep(self.interval).await;
                if let Err(error) = self.run_cleanup_pass(Utc::now()).await {
                    warn!("recording artifact cleanup pass failed: {error}");
                }
            }
        });
    }

    pub async fn run_cleanup_pass(&self, now: DateTime<Utc>) -> Result<(), SessionStoreError> {
        let candidates = self
            .session_store
            .list_recording_artifact_retention_candidates(now)
            .await?;
        for candidate in candidates {
            self.cleanup_candidate(candidate).await;
        }
        Ok(())
    }

    async fn cleanup_candidate(&self, candidate: RecordingArtifactRetentionCandidate) {
        match self.artifact_store.delete(&candidate.artifact_ref).await {
            Ok(()) => {}
            Err(error) => {
                warn!(
                    session_id = %candidate.session_id,
                    recording_id = %candidate.recording_id,
                    artifact_ref = %candidate.artifact_ref,
                    "failed to remove expired recording artifact: {error}"
                );
                return;
            }
        }

        match self
            .session_store
            .clear_recording_artifact_path(candidate.session_id, candidate.recording_id)
            .await
        {
            Ok(Some(_)) => {
                info!(
                    session_id = %candidate.session_id,
                    recording_id = %candidate.recording_id,
                    expired_at = %candidate.expires_at,
                    "cleared retained recording artifact after expiration"
                );
            }
            Ok(None) => {}
            Err(error) => {
                warn!(
                    session_id = %candidate.session_id,
                    recording_id = %candidate.recording_id,
                    "failed to clear expired recording artifact metadata: {error}"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::Duration;

    use chrono::Duration as ChronoDuration;
    use tempfile::tempdir;

    use super::*;
    use crate::auth::AuthenticatedPrincipal;
    use crate::recording_artifact_store::{
        FinalizeRecordingArtifactRequest, RecordingArtifactStore,
    };
    use crate::session_control::{
        CreateSessionRequest, PersistCompletedSessionRecordingRequest, SessionOwnerMode,
        SessionRecordingFormat, SessionRecordingMode, SessionRecordingPolicy,
    };

    fn owner() -> AuthenticatedPrincipal {
        AuthenticatedPrincipal {
            subject: "owner".to_string(),
            issuer: "issuer".to_string(),
            display_name: Some("Owner".to_string()),
            client_id: None,
        }
    }

    async fn create_manual_recording_session(
        store: &SessionStore,
        retention_sec: Option<u32>,
    ) -> uuid::Uuid {
        let session = store
            .create_session(
                &owner(),
                CreateSessionRequest {
                    template_id: None,
                    owner_mode: None,
                    viewport: None,
                    idle_timeout_sec: None,
                    labels: HashMap::new(),
                    integration_context: None,
                    recording: SessionRecordingPolicy {
                        mode: SessionRecordingMode::Manual,
                        format: SessionRecordingFormat::Webm,
                        retention_sec,
                    },
                },
                SessionOwnerMode::Collaborative,
            )
            .await
            .unwrap();
        session.id
    }

    #[tokio::test]
    async fn cleanup_pass_removes_expired_ready_artifacts() {
        let store = SessionStore::in_memory();
        let session_id = create_manual_recording_session(&store, Some(60)).await;
        let temp_dir = tempdir().unwrap();
        let source_path = temp_dir.path().join("recording.webm");
        std::fs::write(&source_path, b"artifact").unwrap();
        let artifact_root = temp_dir.path().join("artifacts");
        let artifact_store = Arc::new(RecordingArtifactStore::local_fs(artifact_root.clone()));

        let recording = store
            .create_recording_for_session(session_id, SessionRecordingFormat::Webm, None)
            .await
            .unwrap();
        let stored_artifact = artifact_store
            .finalize(FinalizeRecordingArtifactRequest {
                session_id,
                recording_id: recording.id,
                format: SessionRecordingFormat::Webm,
                source_path: source_path.to_string_lossy().to_string(),
            })
            .await
            .unwrap();
        store
            .complete_recording_for_session(
                session_id,
                recording.id,
                PersistCompletedSessionRecordingRequest {
                    artifact_ref: stored_artifact.artifact_ref.clone(),
                    mime_type: Some("video/webm".to_string()),
                    bytes: Some(8),
                    duration_ms: Some(500),
                },
            )
            .await
            .unwrap();

        let completed = store
            .get_recording_for_session(session_id, recording.id)
            .await
            .unwrap()
            .unwrap();
        let manager = Arc::new(RecordingRetentionManager::new(
            store.clone(),
            artifact_store.clone(),
            Duration::from_secs(60),
        ));

        manager
            .run_cleanup_pass(completed.completed_at.unwrap() + ChronoDuration::seconds(61))
            .await
            .unwrap();

        assert!(artifact_store
            .read(&stored_artifact.artifact_ref)
            .await
            .is_err());
        let reloaded = store
            .get_recording_for_session(session_id, recording.id)
            .await
            .unwrap()
            .unwrap();
        assert!(reloaded.artifact_ref.is_none());
        assert_eq!(
            reloaded.state,
            crate::session_control::SessionRecordingState::Ready
        );
    }

    #[tokio::test]
    async fn cleanup_pass_keeps_unexpired_ready_artifacts() {
        let store = SessionStore::in_memory();
        let session_id = create_manual_recording_session(&store, Some(60)).await;
        let temp_dir = tempdir().unwrap();
        let source_path = temp_dir.path().join("recording.webm");
        std::fs::write(&source_path, b"artifact").unwrap();
        let artifact_root = temp_dir.path().join("artifacts");
        let artifact_store = Arc::new(RecordingArtifactStore::local_fs(artifact_root.clone()));

        let recording = store
            .create_recording_for_session(session_id, SessionRecordingFormat::Webm, None)
            .await
            .unwrap();
        let stored_artifact = artifact_store
            .finalize(FinalizeRecordingArtifactRequest {
                session_id,
                recording_id: recording.id,
                format: SessionRecordingFormat::Webm,
                source_path: source_path.to_string_lossy().to_string(),
            })
            .await
            .unwrap();
        store
            .complete_recording_for_session(
                session_id,
                recording.id,
                PersistCompletedSessionRecordingRequest {
                    artifact_ref: stored_artifact.artifact_ref.clone(),
                    mime_type: Some("video/webm".to_string()),
                    bytes: Some(8),
                    duration_ms: Some(500),
                },
            )
            .await
            .unwrap();

        let completed = store
            .get_recording_for_session(session_id, recording.id)
            .await
            .unwrap()
            .unwrap();
        let manager = Arc::new(RecordingRetentionManager::new(
            store.clone(),
            artifact_store.clone(),
            Duration::from_secs(60),
        ));

        manager
            .run_cleanup_pass(completed.completed_at.unwrap() + ChronoDuration::seconds(59))
            .await
            .unwrap();

        let bytes = artifact_store
            .read(&stored_artifact.artifact_ref)
            .await
            .unwrap();
        assert_eq!(bytes.as_slice(), b"artifact");
        let reloaded = store
            .get_recording_for_session(session_id, recording.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            reloaded.artifact_ref.as_deref(),
            Some(stored_artifact.artifact_ref.as_str())
        );
    }
}
