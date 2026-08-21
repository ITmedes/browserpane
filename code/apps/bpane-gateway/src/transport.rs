use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::task::JoinSet;
use tracing::{error, info, warn};
use wtransport::{Endpoint, Identity, ServerConfig};

mod bitrate;
mod bootstrap;
mod egress;
mod ingress;
mod negotiation;
mod policy;
mod request;
mod session;
mod session_task;
mod tasks;

/// Maximum number of concurrent WebTransport sessions.
const MAX_CONCURRENT_SESSIONS: u64 = 100;

use self::request::{
    sanitized_request_path_for_log, validate_request_path, RequestValidationError,
};
use self::session_task::handle_session;
use crate::auth::AuthValidator;
use crate::lifecycle::GatewayLifecycle;
use crate::metrics::GatewayMetrics;
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
    pub protocol_handshake_timeout: Duration,
    pub protocol_legacy_compatibility: bool,
    pub runtime_startup_capacity_wait: Duration,
    pub registry: Arc<SessionRegistry>,
    pub lifecycle: Arc<GatewayLifecycle>,
    pub metrics: Arc<GatewayMetrics>,
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
    protocol_handshake_timeout: Duration,
    protocol_legacy_compatibility: bool,
    runtime_startup_capacity_wait: Duration,
    registry: Arc<SessionRegistry>,
    lifecycle: Arc<GatewayLifecycle>,
    metrics: Arc<GatewayMetrics>,
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
            protocol_handshake_timeout: config.protocol_handshake_timeout,
            protocol_legacy_compatibility: config.protocol_legacy_compatibility,
            runtime_startup_capacity_wait: config.runtime_startup_capacity_wait,
            registry: config.registry,
            lifecycle: config.lifecycle,
            metrics: config.metrics,
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
        let mut session_tasks = JoinSet::new();

        loop {
            let incoming = tokio::select! {
                _ = self.lifecycle.wait_for_draining() => break,
                completed = session_tasks.join_next(), if !session_tasks.is_empty() => {
                    log_completed_session_task(completed);
                    continue;
                }
                incoming = endpoint.accept() => incoming,
            };
            let session_request = match tokio::select! {
                _ = self.lifecycle.wait_for_draining() => break,
                request = incoming => request,
            } {
                Ok(req) => req,
                Err(e) => {
                    warn!("failed to accept incoming connection: {e}");
                    continue;
                }
            };

            if !self.lifecycle.accepts_new_work() {
                session_request.not_found().await;
                break;
            }

            // Enforce session limit
            if active_sessions.load(Ordering::Relaxed) >= MAX_CONCURRENT_SESSIONS {
                warn!("max concurrent sessions ({MAX_CONCURRENT_SESSIONS}) reached, rejecting");
                session_request.not_found().await;
                continue;
            }

            let path = session_request.path().to_string();
            let safe_path = sanitized_request_path_for_log(&path);
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
                    warn!("no credential in request path: {safe_path}");
                    session_request.not_found().await;
                    continue;
                }
                Err(RequestValidationError::MissingSessionId) => {
                    warn!("session_id missing from bearer connect path: {safe_path}");
                    session_request.not_found().await;
                    continue;
                }
                Err(RequestValidationError::SessionNotVisible) => {
                    warn!("session not visible or not connectable for path: {safe_path}");
                    session_request.not_found().await;
                    continue;
                }
                Err(RequestValidationError::SessionLookupFailed) => {
                    warn!("session lookup failed for path: {safe_path}");
                    session_request.not_found().await;
                    continue;
                }
            };

            let connection = match session_request.accept().await {
                Ok(conn) => conn,
                Err(e) => {
                    warn!("failed to accept WebTransport session: {e}");
                    continue;
                }
            };

            session_counter += 1;
            let session_id = session_counter;
            let heartbeat_timeout = self.heartbeat_timeout;
            let protocol_handshake_timeout = self.protocol_handshake_timeout;
            let protocol_legacy_compatibility = self.protocol_legacy_compatibility;
            let runtime_startup_capacity_wait = self.runtime_startup_capacity_wait;
            let active_sessions_clone = active_sessions.clone();
            let registry = self.registry.clone();
            let session_manager = self.session_manager.clone();
            let session_store = self.session_store.clone();
            let workspace_file_store = self.workspace_file_store.clone();
            let recording_lifecycle = self.recording_lifecycle.clone();
            let metrics = self.metrics.clone();
            let idle_stop_timeout = self.idle_stop_timeout;
            active_sessions.fetch_add(1, Ordering::Relaxed);

            info!(
                session_id,
                active = active_sessions.load(Ordering::Relaxed),
                "new WebTransport session accepted"
            );

            session_tasks.spawn(async move {
                if let Err(e) = handle_session(session_task::SessionTaskContext {
                    connection,
                    session_id,
                    connect_request: validated_request,
                    session_manager,
                    session_store,
                    workspace_file_store,
                    idle_stop_timeout,
                    heartbeat_timeout,
                    protocol_handshake_timeout,
                    protocol_legacy_compatibility,
                    runtime_startup_capacity_wait,
                    registry: registry.clone(),
                    recording_lifecycle,
                    metrics,
                })
                .await
                {
                    error!(session_id, "session error: {e}");
                }
                active_sessions_clone.fetch_sub(1, Ordering::Relaxed);
                info!(session_id, "session ended");
            });
        }

        info!(
            active = active_sessions.load(Ordering::Relaxed),
            "WebTransport listener stopped accepting new sessions"
        );
        while let Some(completed) = session_tasks.join_next().await {
            log_completed_session_task(Some(completed));
        }
        endpoint.close(wtransport::VarInt::from_u32(0), b"gateway shutdown");
        endpoint.wait_idle().await;
        info!("WebTransport sessions drained");
        Ok(())
    }
}

fn log_completed_session_task(completed: Option<Result<(), tokio::task::JoinError>>) {
    if let Some(Err(error)) = completed {
        if error.is_cancelled() {
            warn!("WebTransport session task cancelled during shutdown");
        } else {
            error!("WebTransport session task failed: {error}");
        }
    }
}

#[cfg(test)]
mod tests;
