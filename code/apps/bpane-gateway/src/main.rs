mod api;
mod auth;
mod config;
mod connect_ticket;
mod relay;
mod session;
mod session_control;
mod session_hub;
mod session_registry;
mod transport;

use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use tracing::info;
use tracing::warn;
use tracing_subscriber::EnvFilter;
use wtransport::Identity;

use auth::{AuthValidator, OidcConfig};
use config::Config;
use connect_ticket::SessionConnectTicketManager;
use session_control::{SessionOwnerMode, SessionStore};
use session_registry::SessionRegistry;
use transport::TransportServer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = Config::parse();

    let shared_secret = match &config.hmac_secret {
        Some(hex_secret) => {
            let decoded = hex::decode(hex_secret)?;
            if decoded.len() < 16 {
                anyhow::bail!(
                    "HMAC secret must be at least 16 bytes (32 hex chars), got {}",
                    decoded.len()
                );
            }
            decoded
        }
        None => {
            let mut secret = vec![0u8; 32];
            rand::fill(&mut secret[..]);
            secret
        }
    };

    let auth_validator = Arc::new(if let Some(issuer) = &config.oidc_issuer {
        let audience = config.oidc_audience.clone().ok_or_else(|| {
            anyhow::anyhow!("--oidc-audience is required when --oidc-issuer is set")
        })?;
        info!("using OIDC/JWT auth with issuer {}", issuer);
        if config.token_file.is_some() {
            info!("ignoring --token-file because OIDC auth is enabled");
        }
        AuthValidator::from_oidc(OidcConfig {
            issuer: issuer.clone(),
            audience,
            jwks_url: config.oidc_jwks_url.clone(),
        })
        .await?
    } else {
        let validator = AuthValidator::from_hmac_secret(shared_secret.clone());
        if let Some(token) = validator.generate_token() {
            info!("generated dev token: {token}");
            if let Some(path) = &config.token_file {
                std::fs::write(path, &token)?;
                info!("wrote token to {}", path.display());
            }
        }
        validator
    });

    let connect_ticket_manager = Arc::new(SessionConnectTicketManager::new(
        shared_secret,
        Duration::from_secs(config.session_ticket_ttl_secs),
    ));

    // Load or generate TLS identity
    let identity = match (&config.cert, &config.key) {
        (Some(cert_path), Some(key_path)) => Identity::load_pemfiles(cert_path, key_path).await?,
        _ => {
            info!("generating self-signed certificate for development");
            Identity::self_signed(["localhost", "127.0.0.1"])?
        }
    };

    let bind_addr: std::net::SocketAddr = format!("{}:{}", config.bind, config.port)
        .parse()
        .map_err(|e| {
            anyhow::anyhow!(
                "invalid bind address '{}:{}': {e}",
                config.bind,
                config.port
            )
        })?;

    let registry = Arc::new(SessionRegistry::new(
        config.max_viewers,
        config.exclusive_browser_owner,
    ));

    let session_store = if let Some(database_url) = &config.database_url {
        info!("using postgres-backed session control store");
        SessionStore::from_database_url(database_url).await?
    } else {
        warn!("no --database-url configured; /api/v1 sessions will use an in-memory store");
        SessionStore::in_memory()
    };

    let agent_socket_str = config.agent_socket.to_str().unwrap().to_string();

    let server = TransportServer::new(
        bind_addr,
        identity,
        agent_socket_str.clone(),
        auth_validator.clone(),
        connect_ticket_manager.clone(),
        Duration::from_secs(config.heartbeat_timeout_secs),
        registry.clone(),
    );

    let api_bind_addr: std::net::SocketAddr = format!("{}:{}", config.bind, config.api_port)
        .parse()
        .map_err(|e| {
            anyhow::anyhow!(
                "invalid API bind address '{}:{}': {e}",
                config.bind,
                config.api_port
            )
        })?;

    tokio::select! {
        result = server.run() => result,
        result = api::run_api_server(
            api_bind_addr,
            registry,
            auth_validator,
            connect_ticket_manager,
            session_store,
            agent_socket_str,
            config.public_gateway_url,
            if config.exclusive_browser_owner {
                SessionOwnerMode::ExclusiveBrowserOwner
            } else {
                SessionOwnerMode::Collaborative
            },
        ) => result,
    }
}
