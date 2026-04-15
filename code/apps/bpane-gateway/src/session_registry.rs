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
mod tests;
