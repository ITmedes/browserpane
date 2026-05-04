use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tracing::{error, info, warn};
use wtransport::{Endpoint, Identity, ServerConfig};

mod bitrate;
mod bootstrap;
mod egress;
mod ingress;
mod policy;
mod request;
mod session;
mod session_task;
mod tasks;

/// Maximum number of concurrent WebTransport sessions.
const MAX_CONCURRENT_SESSIONS: u64 = 100;

use self::request::{validate_request_path, RequestValidationError};
use self::session_task::handle_session;
use crate::auth::AuthValidator;
use crate::recording_lifecycle::RecordingLifecycleManager;
use crate::session_access::SessionConnectTicketManager;
use crate::session_control::SessionStore;
use crate::session_manager::SessionManager;
use crate::session_registry::SessionRegistry;
use crate::workspaces::WorkspaceFileStore;

pub struct TransportServerConfig {
    pub bind_addr: SocketAddr,
    pub identity: Identity,
    pub session_manager: Arc<SessionManager>,
    pub auth_validator: Arc<AuthValidator>,
    pub connect_ticket_manager: Arc<SessionConnectTicketManager>,
    pub session_store: SessionStore,
    pub workspace_file_store: Arc<WorkspaceFileStore>,
    pub recording_lifecycle: Arc<RecordingLifecycleManager>,
    pub idle_stop_timeout: Duration,
    pub heartbeat_timeout: Duration,
    pub registry: Arc<SessionRegistry>,
}

pub struct TransportServer {
    bind_addr: SocketAddr,
    identity: Identity,
    session_manager: Arc<SessionManager>,
    auth_validator: Arc<AuthValidator>,
    connect_ticket_manager: Arc<SessionConnectTicketManager>,
    session_store: SessionStore,
    workspace_file_store: Arc<WorkspaceFileStore>,
    recording_lifecycle: Arc<RecordingLifecycleManager>,
    idle_stop_timeout: Duration,
    heartbeat_timeout: Duration,
    registry: Arc<SessionRegistry>,
}

impl TransportServer {
    pub fn new(config: TransportServerConfig) -> Self {
        Self {
            bind_addr: config.bind_addr,
            identity: config.identity,
            session_manager: config.session_manager,
            auth_validator: config.auth_validator,
            connect_ticket_manager: config.connect_ticket_manager,
            session_store: config.session_store,
            workspace_file_store: config.workspace_file_store,
            recording_lifecycle: config.recording_lifecycle,
            idle_stop_timeout: config.idle_stop_timeout,
            heartbeat_timeout: config.heartbeat_timeout,
            registry: config.registry,
        }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let config = ServerConfig::builder()
            .with_bind_address(self.bind_addr)
            .with_identity(self.identity)
            .keep_alive_interval(Some(Duration::from_secs(3)))
            .build();

        let endpoint = Endpoint::server(config)?;
        info!("WebTransport gateway listening on {}", self.bind_addr);

        let mut session_counter: u64 = 0;
        let active_sessions = Arc::new(AtomicU64::new(0));

        loop {
            let incoming = endpoint.accept().await;
            let session_request = match incoming.await {
                Ok(req) => req,
                Err(e) => {
                    warn!("failed to accept incoming connection: {e}");
                    continue;
                }
            };

            // Enforce session limit
            if active_sessions.load(Ordering::Relaxed) >= MAX_CONCURRENT_SESSIONS {
                warn!("max concurrent sessions ({MAX_CONCURRENT_SESSIONS}) reached, rejecting");
                session_request.not_found().await;
                continue;
            }

            let path = session_request.path().to_string();
            let validated_request = match validate_request_path(
                &path,
                &self.auth_validator,
                &self.connect_ticket_manager,
                &self.session_store,
            )
            .await
            {
                Ok(request) => request,
                Err(RequestValidationError::InvalidToken(e)) => {
                    warn!("token validation failed: {e}");
                    session_request.not_found().await;
                    continue;
                }
                Err(RequestValidationError::InvalidSessionTicket(e)) => {
                    warn!("session ticket validation failed: {e}");
                    session_request.not_found().await;
                    continue;
                }
                Err(RequestValidationError::MissingCredential) => {
                    warn!("no credential in request path: {path}");
                    session_request.not_found().await;
                    continue;
                }
                Err(RequestValidationError::MissingSessionId) => {
                    warn!("session_id missing from bearer connect path: {path}");
                    session_request.not_found().await;
                    continue;
                }
                Err(RequestValidationError::SessionNotVisible) => {
                    warn!("session not visible or not connectable for path: {path}");
                    session_request.not_found().await;
                    continue;
                }
                Err(RequestValidationError::SessionLookupFailed) => {
                    warn!("session lookup failed for path: {path}");
                    session_request.not_found().await;
                    continue;
                }
            };

            let runtime = match self
                .session_manager
                .resolve(validated_request.session_id)
                .await
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    warn!(
                        session_id = %validated_request.session_id,
                        "runtime resolution failed: {error}"
                    );
                    session_request.not_found().await;
                    continue;
                }
            };
            self.session_manager
                .mark_session_active(validated_request.session_id)
                .await;
            if let Err(error) = self
                .session_store
                .mark_session_active(validated_request.session_id)
                .await
            {
                warn!(
                    session_id = %validated_request.session_id,
                    "failed to mark session active in store: {error}"
                );
            }

            let connection = match session_request.accept().await {
                Ok(conn) => conn,
                Err(e) => {
                    warn!("failed to accept WebTransport session: {e}");
                    continue;
                }
            };

            session_counter += 1;
            let session_id = session_counter;
            let agent_path = runtime.agent_socket_path.clone();
            let heartbeat_timeout = self.heartbeat_timeout;
            let active_sessions_clone = active_sessions.clone();
            let registry = self.registry.clone();
            let session_manager = self.session_manager.clone();
            let session_store = self.session_store.clone();
            let workspace_file_store = self.workspace_file_store.clone();
            let recording_lifecycle = self.recording_lifecycle.clone();
            let idle_stop_timeout = self.idle_stop_timeout;
            active_sessions.fetch_add(1, Ordering::Relaxed);

            info!(
                session_id,
                active = active_sessions.load(Ordering::Relaxed),
                "new WebTransport session accepted"
            );

            tokio::spawn(async move {
                if let Err(e) = handle_session(session_task::SessionTaskContext {
                    connection,
                    session_id,
                    connect_request: validated_request,
                    session_manager,
                    session_store,
                    workspace_file_store,
                    idle_stop_timeout,
                    agent_socket_path: agent_path,
                    heartbeat_timeout,
                    registry: registry.clone(),
                    recording_lifecycle,
                })
                .await
                {
                    error!(session_id, "session error: {e}");
                }
                active_sessions_clone.fetch_sub(1, Ordering::Relaxed);
                info!(session_id, "session ended");
            });
        }
    }
}

#[cfg(test)]
mod tests;
