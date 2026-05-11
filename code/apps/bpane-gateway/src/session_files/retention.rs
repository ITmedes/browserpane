use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use tokio::time::sleep;
use tracing::{info, warn};

use super::SessionFileRetentionCandidate;
use crate::session_control::{SessionStore, SessionStoreError};
use crate::workspaces::WorkspaceFileStore;

#[derive(Clone)]
pub struct SessionFileRetentionManager {
    session_store: SessionStore,
    file_store: Arc<WorkspaceFileStore>,
    interval: Duration,
    retention: ChronoDuration,
}

impl SessionFileRetentionManager {
    pub fn new(
        session_store: SessionStore,
        file_store: Arc<WorkspaceFileStore>,
        interval: Duration,
        retention: ChronoDuration,
    ) -> Self {
        Self {
            session_store,
            file_store,
            interval,
            retention,
        }
    }

    pub fn start(self: Arc<Self>) {
        tokio::spawn(async move {
            loop {
                sleep(self.interval).await;
                if let Err(error) = self.run_cleanup_pass(Utc::now()).await {
                    warn!("session file cleanup pass failed: {error}");
                }
            }
        });
    }

    pub async fn run_cleanup_pass(&self, now: DateTime<Utc>) -> Result<(), SessionStoreError> {
        let candidates = self
            .session_store
            .list_session_file_retention_candidates(now, self.retention)
            .await?;
        for candidate in candidates {
            self.cleanup_candidate(candidate).await;
        }
        Ok(())
    }

    async fn cleanup_candidate(&self, candidate: SessionFileRetentionCandidate) {
        match self.file_store.delete(&candidate.artifact_ref).await {
            Ok(()) => {}
            Err(error) => {
                warn!(
                    session_id = %candidate.session_id,
                    file_id = %candidate.file_id,
                    artifact_ref = %candidate.artifact_ref,
                    "failed to remove expired session file artifact: {error}"
                );
                return;
            }
        }

        match self
            .session_store
            .delete_session_file_for_session(candidate.session_id, candidate.file_id)
            .await
        {
            Ok(Some(_)) => {
                info!(
                    session_id = %candidate.session_id,
                    file_id = %candidate.file_id,
                    expired_at = %candidate.expires_at,
                    "deleted retained session file after expiration"
                );
            }
            Ok(None) => {}
            Err(error) => {
                warn!(
                    session_id = %candidate.session_id,
                    file_id = %candidate.file_id,
                    "failed to delete expired session file metadata: {error}"
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
    use uuid::Uuid;

    use super::SessionFileRetentionManager;
    use crate::auth::AuthenticatedPrincipal;
    use crate::session_control::{
        CreateSessionRequest, PersistSessionFileRequest, SessionOwnerMode, SessionRecordingPolicy,
        SessionStore,
    };
    use crate::session_files::SessionFileSource;
    use crate::workspaces::{StoreWorkspaceFileRequest, WorkspaceFileStore};

    fn owner() -> AuthenticatedPrincipal {
        AuthenticatedPrincipal {
            subject: "owner".to_string(),
            issuer: "issuer".to_string(),
            display_name: Some("Owner".to_string()),
            client_id: None,
        }
    }

    async fn create_session(store: &SessionStore) -> Uuid {
        store
            .create_session(
                &owner(),
                CreateSessionRequest {
                    template_id: None,
                    owner_mode: None,
                    viewport: None,
                    idle_timeout_sec: None,
                    labels: HashMap::new(),
                    integration_context: None,
                    extension_ids: Vec::new(),
                    extensions: Vec::new(),
                    recording: SessionRecordingPolicy::default(),
                },
                SessionOwnerMode::Collaborative,
            )
            .await
            .unwrap()
            .id
    }

    async fn record_session_file(
        store: &SessionStore,
        file_store: &WorkspaceFileStore,
        session_id: Uuid,
    ) -> Uuid {
        let file_id = Uuid::now_v7();
        let artifact = file_store
            .write(StoreWorkspaceFileRequest {
                workspace_id: session_id,
                file_id,
                file_name: "upload.txt".to_string(),
                bytes: b"session upload".to_vec(),
            })
            .await
            .unwrap();
        store
            .record_session_file(PersistSessionFileRequest {
                id: file_id,
                session_id,
                name: "upload.txt".to_string(),
                media_type: Some("text/plain".to_string()),
                byte_count: 14,
                sha256_hex: "sha256".to_string(),
                artifact_ref: artifact.artifact_ref,
                source: SessionFileSource::BrowserUpload,
                labels: HashMap::new(),
            })
            .await
            .unwrap()
            .id
    }

    #[tokio::test]
    async fn cleanup_pass_removes_expired_session_file_artifacts() {
        let store = SessionStore::in_memory();
        let temp_dir = tempdir().unwrap();
        let file_store = Arc::new(WorkspaceFileStore::local_fs(temp_dir.path().join("files")));
        let session_id = create_session(&store).await;
        let file_id = record_session_file(&store, &file_store, session_id).await;
        let stored = store
            .get_session_file_for_session(session_id, file_id)
            .await
            .unwrap()
            .unwrap();
        let manager = Arc::new(SessionFileRetentionManager::new(
            store.clone(),
            file_store.clone(),
            Duration::from_secs(60),
            ChronoDuration::seconds(60),
        ));

        manager
            .run_cleanup_pass(stored.created_at + ChronoDuration::seconds(61))
            .await
            .unwrap();

        assert!(store
            .get_session_file_for_session(session_id, file_id)
            .await
            .unwrap()
            .is_none());
        assert!(file_store.read(&stored.artifact_ref).await.is_err());
    }

    #[tokio::test]
    async fn cleanup_pass_keeps_unexpired_session_file_artifacts() {
        let store = SessionStore::in_memory();
        let temp_dir = tempdir().unwrap();
        let file_store = Arc::new(WorkspaceFileStore::local_fs(temp_dir.path().join("files")));
        let session_id = create_session(&store).await;
        let file_id = record_session_file(&store, &file_store, session_id).await;
        let stored = store
            .get_session_file_for_session(session_id, file_id)
            .await
            .unwrap()
            .unwrap();
        let manager = Arc::new(SessionFileRetentionManager::new(
            store.clone(),
            file_store.clone(),
            Duration::from_secs(60),
            ChronoDuration::seconds(60),
        ));

        manager
            .run_cleanup_pass(stored.created_at + ChronoDuration::seconds(59))
            .await
            .unwrap();

        assert!(store
            .get_session_file_for_session(session_id, file_id)
            .await
            .unwrap()
            .is_some());
        assert_eq!(
            file_store
                .read(&stored.artifact_ref)
                .await
                .unwrap()
                .as_slice(),
            b"session upload"
        );
    }
}
