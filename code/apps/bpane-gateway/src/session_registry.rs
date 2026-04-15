use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::{debug, warn};

use crate::session_hub::{ClientHandle, SessionHub};

/// Maps agent socket paths to active SessionHubs.
///
/// When a browser client connects, the registry either returns an existing
/// hub (joining the session) or creates a new one (first client).
pub struct SessionRegistry {
    hubs: Mutex<HashMap<String, Arc<SessionHub>>>,
    max_viewers: u32,
    exclusive_browser_owner: bool,
}

impl SessionRegistry {
    pub fn new(max_viewers: u32, exclusive_browser_owner: bool) -> Self {
        Self {
            hubs: Mutex::new(HashMap::new()),
            max_viewers,
            exclusive_browser_owner,
        }
    }

    fn prune_inactive_hubs(hubs: &mut HashMap<String, Arc<SessionHub>>) {
        hubs.retain(|path, hub| {
            if !hub.is_active() {
                debug!("removing inactive hub for {path}");
                false
            } else {
                true
            }
        });
    }

    async fn lookup_live_hub(&self, agent_socket_path: &str) -> Option<Arc<SessionHub>> {
        let mut hubs = self.hubs.lock().await;
        Self::prune_inactive_hubs(&mut hubs);
        hubs.get(agent_socket_path).cloned()
    }

    async fn insert_or_get_live_hub(
        &self,
        agent_socket_path: &str,
        new_hub: Arc<SessionHub>,
    ) -> (Arc<SessionHub>, bool) {
        let mut hubs = self.hubs.lock().await;
        Self::prune_inactive_hubs(&mut hubs);

        if let Some(existing) = hubs.get(agent_socket_path) {
            debug!(
                path = agent_socket_path,
                "concurrent session hub creation won race, using existing"
            );
            return (existing.clone(), false);
        }

        hubs.insert(agent_socket_path.to_string(), new_hub.clone());
        (new_hub, true)
    }

    async fn get_or_create_hub(
        &self,
        agent_socket_path: &str,
    ) -> anyhow::Result<(Arc<SessionHub>, bool)> {
        if let Some(hub) = self.lookup_live_hub(agent_socket_path).await {
            return Ok((hub, false));
        }

        let new_hub = Arc::new(
            SessionHub::new(
                agent_socket_path,
                self.max_viewers,
                self.exclusive_browser_owner,
            )
            .await?,
        );

        Ok(self
            .insert_or_get_live_hub(agent_socket_path, new_hub)
            .await)
    }

    /// Join an existing session or create a new one.
    /// Returns a ClientHandle for the browser client.
    ///
    /// The registry mutex is held only for the HashMap lookup/insert.
    /// `subscribe()` and `SessionHub::new()` (which does async I/O) both
    /// run outside the critical section so that concurrent client joins for
    /// different sessions — or a late joiner onto an existing session — do
    /// not serialize behind relay setup.
    pub async fn join(
        &self,
        agent_socket_path: &str,
    ) -> anyhow::Result<(ClientHandle, Arc<SessionHub>)> {
        let (hub, created) = self.get_or_create_hub(agent_socket_path).await?;
        let handle = hub.subscribe().await.map_err(anyhow::Error::from)?;

        if created {
            debug!(
                client_id = handle.client_id,
                "created new session hub, client is owner"
            );
        } else {
            debug!(
                client_id = handle.client_id,
                is_owner = handle.is_owner,
                clients = hub.client_count(),
                "client joined existing session"
            );
        }

        Ok((handle, hub))
    }

    /// Get or create a hub without subscribing a browser client.
    /// Used by the HTTP API for MCP owner registration.
    ///
    /// Like `join()`, the mutex is held only for the HashMap operation;
    /// `SessionHub::new()` runs outside the critical section.
    pub async fn ensure_hub(&self, agent_socket_path: &str) -> anyhow::Result<Arc<SessionHub>> {
        let (hub, _) = self.get_or_create_hub(agent_socket_path).await?;
        Ok(hub)
    }

    /// Called when a client disconnects.
    pub async fn leave(&self, agent_socket_path: &str, client_id: u64) {
        let hub = {
            let hubs = self.hubs.lock().await;
            hubs.get(agent_socket_path).cloned()
        };

        if let Some(hub) = hub {
            hub.unsubscribe(client_id).await;
            let remaining = hub.client_count();
            debug!(client_id, remaining, "client left session");
        } else {
            warn!(client_id, "leave called but no hub found for path");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bpane_protocol::ControlMessage;
    use tokio::io::AsyncWriteExt;
    use tokio::net::UnixListener;

    async fn mock_agent(sock_path: &str) -> tokio::task::JoinHandle<()> {
        mock_agent_with_connections(sock_path, 1).await
    }

    async fn mock_agent_with_connections(
        sock_path: &str,
        expected_connections: usize,
    ) -> tokio::task::JoinHandle<()> {
        let listener = UnixListener::bind(sock_path).unwrap();
        tokio::spawn(async move {
            let ready = ControlMessage::SessionReady {
                version: 1,
                flags: bpane_protocol::SessionFlags::empty(),
            };
            let encoded = ready.to_frame().encode();
            let mut streams = Vec::with_capacity(expected_connections);
            for _ in 0..expected_connections {
                let (mut stream, _) = listener.accept().await.unwrap();
                stream.write_all(&encoded).await.unwrap();
                streams.push(stream);
            }
            // Keep connection alive
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            drop(streams);
        })
    }

    #[tokio::test]
    async fn two_clients_share_same_hub() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("test.sock");
        let sock_str = sock.to_str().unwrap();

        let _agent = mock_agent(sock_str).await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let registry = SessionRegistry::new(10, false);

        let (c1, hub1) = registry.join(sock_str).await.unwrap();
        assert!(c1.is_owner);

        let (c2, hub2) = registry.join(sock_str).await.unwrap();
        assert!(c2.is_owner);

        // Both should reference the same hub
        assert!(Arc::ptr_eq(&hub1, &hub2));
        assert_eq!(hub1.client_count(), 2);
    }

    #[tokio::test]
    async fn exclusive_owner_mode_marks_late_joiner_as_viewer() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("test.sock");
        let sock_str = sock.to_str().unwrap();

        let _agent = mock_agent(sock_str).await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let registry = SessionRegistry::new(10, true);

        let (c1, _) = registry.join(sock_str).await.unwrap();
        let (c2, _) = registry.join(sock_str).await.unwrap();
        assert!(c1.is_owner);
        assert!(!c2.is_owner);
    }

    #[tokio::test]
    async fn leave_nonexistent_session_does_not_panic() {
        let registry = SessionRegistry::new(10, false);
        // Should not panic even when no hub exists
        registry.leave("/nonexistent/path.sock", 42).await;
    }

    #[tokio::test]
    async fn ensure_hub_creates_hub_without_subscribing() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("test.sock");
        let sock_str = sock.to_str().unwrap();

        let _agent = mock_agent(sock_str).await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let registry = SessionRegistry::new(10, false);

        // ensure_hub should create a hub but not subscribe any browser client
        let hub = registry.ensure_hub(sock_str).await.unwrap();
        assert_eq!(hub.client_count(), 0);
        assert!(hub.is_active());
    }

    #[tokio::test]
    async fn ensure_hub_reuses_existing() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("test.sock");
        let sock_str = sock.to_str().unwrap();

        let _agent = mock_agent(sock_str).await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let registry = SessionRegistry::new(10, false);

        let hub1 = registry.ensure_hub(sock_str).await.unwrap();
        let hub2 = registry.ensure_hub(sock_str).await.unwrap();
        assert!(Arc::ptr_eq(&hub1, &hub2));
    }

    #[tokio::test]
    async fn concurrent_ensure_hub_shares_same_hub() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("test.sock");
        let sock_str = sock.to_str().unwrap();

        let _agent = mock_agent_with_connections(sock_str, 2).await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let registry = Arc::new(SessionRegistry::new(10, false));
        let registry2 = Arc::clone(&registry);

        let (hub1, hub2) = tokio::join!(
            registry.ensure_hub(sock_str),
            registry2.ensure_hub(sock_str)
        );
        let hub1 = hub1.unwrap();
        let hub2 = hub2.unwrap();

        assert!(Arc::ptr_eq(&hub1, &hub2));
        assert_eq!(hub1.client_count(), 0);
    }

    #[tokio::test]
    async fn join_after_ensure_hub_shares_same_hub() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("test.sock");
        let sock_str = sock.to_str().unwrap();

        let _agent = mock_agent(sock_str).await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let registry = SessionRegistry::new(10, false);

        let hub_via_ensure = registry.ensure_hub(sock_str).await.unwrap();
        assert_eq!(hub_via_ensure.client_count(), 0);

        let (c1, hub_via_join) = registry.join(sock_str).await.unwrap();
        assert!(Arc::ptr_eq(&hub_via_ensure, &hub_via_join));
        assert!(c1.is_owner);
        assert_eq!(hub_via_ensure.client_count(), 1);
    }

    #[tokio::test]
    async fn leave_decrements_count() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("test.sock");
        let sock_str = sock.to_str().unwrap();

        let _agent = mock_agent(sock_str).await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let registry = SessionRegistry::new(10, false);

        let (c1, hub) = registry.join(sock_str).await.unwrap();
        let (c2, _) = registry.join(sock_str).await.unwrap();
        assert_eq!(hub.client_count(), 2);

        registry.leave(sock_str, c1.client_id).await;
        assert_eq!(hub.client_count(), 1);

        registry.leave(sock_str, c2.client_id).await;
        assert_eq!(hub.client_count(), 0);
    }
}
