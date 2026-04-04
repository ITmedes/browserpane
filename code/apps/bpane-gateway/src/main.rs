mod api;
mod auth;
mod config;
mod relay;
mod session;
mod session_hub;
mod session_registry;
mod transport;

use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use tracing::info;
use tracing_subscriber::EnvFilter;
use wtransport::Identity;

use auth::TokenValidator;
use config::Config;
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

    // Generate or load HMAC secret
    let hmac_secret = match &config.hmac_secret {
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
            // Use CSPRNG for secret generation
            let mut secret = vec![0u8; 32];
            rand::fill(&mut secret[..]);
            let token_validator = TokenValidator::new(secret.clone());
            let token = token_validator.generate_token();
            info!("generated dev token: {token}");
            if let Some(path) = &config.token_file {
                std::fs::write(path, &token)?;
                info!("wrote token to {}", path.display());
            }
            secret
        }
    };

    let token_validator = Arc::new(TokenValidator::new(hmac_secret));

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

    let agent_socket_str = config.agent_socket.to_str().unwrap().to_string();

    let server = TransportServer::new(
        bind_addr,
        identity,
        agent_socket_str.clone(),
        token_validator.clone(),
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
        result = api::run_api_server(api_bind_addr, registry, token_validator, agent_socket_str) => result,
    }
}
