use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tracing::{debug, error, info, warn};
use wtransport::{Endpoint, Identity, ServerConfig};

mod bitrate;
mod bootstrap;
mod egress;
mod ingress;
mod policy;
mod request;
mod tasks;

/// Maximum number of concurrent WebTransport sessions.
const MAX_CONCURRENT_SESSIONS: u64 = 100;

use self::bitrate::DatagramStats;
use self::bootstrap::send_initial_frames;
use self::egress::{spawn_agent_to_browser_task, EgressTaskContext};
use self::ingress::spawn_browser_to_agent_task;
use self::request::{validate_request_path, RequestValidationError};
use self::tasks::{spawn_bitrate_hint_task, spawn_gateway_pinger};
use crate::auth::TokenValidator;
use crate::session::Session;
use crate::session_registry::SessionRegistry;

pub struct TransportServer {
    bind_addr: SocketAddr,
    identity: Identity,
    agent_socket_path: String,
    token_validator: Arc<TokenValidator>,
    heartbeat_timeout: Duration,
    registry: Arc<SessionRegistry>,
}

impl TransportServer {
    pub fn new(
        bind_addr: SocketAddr,
        identity: Identity,
        agent_socket_path: String,
        token_validator: Arc<TokenValidator>,
        heartbeat_timeout: Duration,
        registry: Arc<SessionRegistry>,
    ) -> Self {
        Self {
            bind_addr,
            identity,
            agent_socket_path,
            token_validator,
            heartbeat_timeout,
            registry,
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
            match validate_request_path(&path, &self.token_validator) {
                Ok(()) => {}
                Err(RequestValidationError::InvalidToken(e)) => {
                    warn!("token validation failed: {e}");
                    session_request.not_found().await;
                    continue;
                }
                Err(RequestValidationError::MissingToken) => {
                    warn!("no token in request path: {path}");
                    session_request.not_found().await;
                    continue;
                }
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
            let agent_path = self.agent_socket_path.clone();
            let heartbeat_timeout = self.heartbeat_timeout;
            let active_sessions_clone = active_sessions.clone();
            let registry = self.registry.clone();
            active_sessions.fetch_add(1, Ordering::Relaxed);

            info!(
                session_id,
                active = active_sessions.load(Ordering::Relaxed),
                "new WebTransport session accepted"
            );

            tokio::spawn(async move {
                if let Err(e) = handle_session(
                    connection,
                    session_id,
                    &agent_path,
                    heartbeat_timeout,
                    registry.clone(),
                )
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

async fn handle_session(
    connection: wtransport::Connection,
    session_id: u64,
    agent_socket_path: &str,
    heartbeat_timeout: Duration,
    registry: Arc<SessionRegistry>,
) -> anyhow::Result<()> {
    // Join the shared session hub (or create a new one)
    let (client_handle, hub) = registry.join(agent_socket_path).await?;
    let client_id = client_handle.client_id;
    let joined_as_owner = client_handle.is_owner;
    let locked_resolution = client_handle.locked_resolution;
    let from_host = client_handle.from_host;
    let to_host = client_handle.to_host;
    let initial_frames = client_handle.initial_frames;

    debug!(
        session_id,
        client_id,
        is_owner = joined_as_owner,
        "client joined session hub"
    );

    let session = Arc::new(Session::new(session_id, heartbeat_timeout));

    // Start heartbeat monitor
    let session_clone = session.clone();
    tokio::spawn(async move {
        session_clone.run_heartbeat_monitor().await;
    });

    // Open a bidirectional stream for control
    let (send_stream, recv_stream) = connection.open_bi().await?.await?;
    let send_stream = Arc::new(tokio::sync::Mutex::new(send_stream));

    send_initial_frames(
        &send_stream,
        &initial_frames,
        joined_as_owner,
        locked_resolution,
        session_id,
        client_id,
    )
    .await?;

    let dgram_stats = Arc::new(DatagramStats::new());
    let agent_to_browser = spawn_agent_to_browser_task(
        EgressTaskContext {
            session: session.clone(),
            hub: hub.clone(),
            session_id,
            client_id,
            send_stream: send_stream.clone(),
            connection: connection.clone(),
            dgram_stats: dgram_stats.clone(),
        },
        from_host,
    );

    let bitrate_hint_task = spawn_bitrate_hint_task(
        session_id,
        client_id,
        session.clone(),
        dgram_stats.clone(),
        send_stream.clone(),
    );

    let browser_to_agent = spawn_browser_to_agent_task(
        session.clone(),
        hub.clone(),
        client_id,
        recv_stream,
        send_stream.clone(),
        to_host,
    );

    let gateway_pinger = spawn_gateway_pinger(session.clone(), send_stream.clone());

    tokio::select! {
        _ = agent_to_browser => {}
        _ = browser_to_agent => {}
        _ = gateway_pinger => {}
        _ = bitrate_hint_task => {}
    }

    session.deactivate();
    registry.leave(agent_socket_path, client_id).await;

    // Explicitly close the QUIC connection so Chrome doesn't try to reuse it
    // for subsequent WebTransport sessions. Without this, Chrome's HTTP/3
    // connection pooling sends new CONNECT requests on the stale connection,
    // which wtransport can't handle (one session per QUIC connection).
    connection.close(wtransport::VarInt::from_u32(0), b"session ended");

    Ok(())
}
#[cfg(test)]
mod tests;
