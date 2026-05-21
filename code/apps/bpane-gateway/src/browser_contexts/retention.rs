use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use tokio::time::sleep;
use tracing::{info, warn};

use crate::auth::AuthenticatedPrincipal;
use crate::session_control::{BrowserContextRetentionCandidate, SessionStore, SessionStoreError};
use crate::session_manager::SessionManager;

#[derive(Clone)]
pub struct BrowserContextRetentionManager {
    session_store: SessionStore,
    session_manager: Arc<SessionManager>,
    interval: Duration,
}

impl BrowserContextRetentionManager {
    pub fn new(
        session_store: SessionStore,
        session_manager: Arc<SessionManager>,
        interval: Duration,
    ) -> Self {
        Self {
            session_store,
            session_manager,
            interval,
        }
    }

    pub fn start(self: Arc<Self>) {
        tokio::spawn(async move {
            loop {
                sleep(self.interval).await;
                if let Err(error) = self.run_cleanup_pass(Utc::now()).await {
                    warn!("browser context retention cleanup pass failed: {error}");
                }
            }
        });
    }

    pub async fn run_cleanup_pass(&self, now: DateTime<Utc>) -> Result<(), SessionStoreError> {
        let candidates = self
            .session_store
            .list_browser_context_retention_candidates(now)
            .await?;
        for candidate in candidates {
            self.cleanup_candidate(candidate).await;
        }
        Ok(())
    }

    async fn cleanup_candidate(&self, candidate: BrowserContextRetentionCandidate) {
        let context = candidate.context;
        match self
            .session_manager
            .delete_browser_context_data(context.id)
            .await
        {
            Ok(()) => {}
            Err(error) => {
                warn!(
                    browser_context_id = %context.id,
                    expires_at = %candidate.expires_at,
                    "skipped expired browser context cleanup because runtime data removal failed: {error}"
                );
                return;
            }
        }

        let principal = AuthenticatedPrincipal {
            subject: context.owner_subject,
            issuer: context.owner_issuer,
            display_name: None,
            client_id: None,
        };
        match self
            .session_store
            .delete_browser_context_for_owner(&principal, context.id)
            .await
        {
            Ok(Some(_)) => {
                info!(
                    browser_context_id = %context.id,
                    expires_at = %candidate.expires_at,
                    "deleted expired browser context after retention window elapsed"
                );
            }
            Ok(None) => {}
            Err(error) => {
                warn!(
                    browser_context_id = %context.id,
                    "failed to mark expired browser context deleted: {error}"
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

    use super::BrowserContextRetentionManager;
    use crate::auth::AuthenticatedPrincipal;
    use crate::runtime_manager::RuntimeManagerConfig;
    use crate::session_control::{
        BrowserContextPersistenceMode, BrowserContextState, PersistBrowserContextRequest,
        SessionStore,
    };
    use crate::session_manager::SessionManager;

    fn owner() -> AuthenticatedPrincipal {
        AuthenticatedPrincipal {
            subject: "owner".to_string(),
            issuer: "issuer".to_string(),
            display_name: Some("Owner".to_string()),
            client_id: None,
        }
    }

    fn manager(store: SessionStore) -> BrowserContextRetentionManager {
        BrowserContextRetentionManager::new(
            store,
            Arc::new(
                SessionManager::new(RuntimeManagerConfig::StaticSingle {
                    agent_socket_path: "/tmp/bpane-agent.sock".to_string(),
                    cdp_endpoint: None,
                    idle_timeout: Duration::from_secs(300),
                })
                .unwrap(),
            ),
            Duration::from_secs(60),
        )
    }

    async fn create_context(store: &SessionStore, retention_sec: Option<u32>) -> uuid::Uuid {
        store
            .create_browser_context(
                &owner(),
                PersistBrowserContextRequest {
                    name: format!("context-{}", uuid::Uuid::now_v7()),
                    description: None,
                    labels: HashMap::new(),
                    persistence_mode: BrowserContextPersistenceMode::Reusable,
                    retention_sec,
                },
            )
            .await
            .unwrap()
            .id
    }

    #[tokio::test]
    async fn cleanup_pass_deletes_expired_browser_contexts() {
        let store = SessionStore::in_memory();
        let context_id = create_context(&store, Some(60)).await;
        let stored = store
            .get_browser_context_for_owner(&owner(), context_id)
            .await
            .unwrap()
            .unwrap();

        manager(store.clone())
            .run_cleanup_pass(stored.created_at + ChronoDuration::seconds(61))
            .await
            .unwrap();

        let deleted = store
            .get_browser_context_for_owner(&owner(), context_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(deleted.state, BrowserContextState::Deleted);
        assert!(deleted.deleted_at.is_some());
    }

    #[tokio::test]
    async fn cleanup_pass_keeps_unexpired_and_manual_contexts() {
        let store = SessionStore::in_memory();
        let expiring_id = create_context(&store, Some(60)).await;
        let manual_id = create_context(&store, None).await;
        let expiring = store
            .get_browser_context_for_owner(&owner(), expiring_id)
            .await
            .unwrap()
            .unwrap();

        manager(store.clone())
            .run_cleanup_pass(expiring.created_at + ChronoDuration::seconds(59))
            .await
            .unwrap();

        assert_eq!(
            store
                .get_browser_context_for_owner(&owner(), expiring_id)
                .await
                .unwrap()
                .unwrap()
                .state,
            BrowserContextState::Ready
        );
        assert_eq!(
            store
                .get_browser_context_for_owner(&owner(), manual_id)
                .await
                .unwrap()
                .unwrap()
                .state,
            BrowserContextState::Ready
        );
    }
}
