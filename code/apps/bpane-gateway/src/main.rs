mod api;
mod auth;
mod config;
mod connect_ticket;
mod idle_stop;
mod relay;
mod runtime_manager;
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
use runtime_manager::{DockerRuntimeConfig, RuntimeManagerConfig, SessionRuntimeManager};
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

    let agent_socket_str = config.agent_socket.to_str().unwrap().to_string();
    let runtime_manager = Arc::new(SessionRuntimeManager::new(
        match config.runtime_backend.as_str() {
            "static_single" => RuntimeManagerConfig::StaticSingle {
                agent_socket_path: agent_socket_str,
                idle_timeout: Duration::from_secs(config.runtime_idle_timeout_secs),
            },
            "docker_single" => RuntimeManagerConfig::DockerSingle(DockerRuntimeConfig {
                docker_bin: config.docker_runtime_bin.clone(),
                image: config.docker_runtime_image.clone().ok_or_else(|| {
                    anyhow::anyhow!(
                        "--docker-runtime-image is required for --runtime-backend docker_single"
                    )
                })?,
                network: config.docker_runtime_network.clone().ok_or_else(|| {
                    anyhow::anyhow!(
                        "--docker-runtime-network is required for --runtime-backend docker_single"
                    )
                })?,
                shared_run_volume: config.docker_runtime_volume.clone().ok_or_else(|| {
                    anyhow::anyhow!(
                        "--docker-runtime-volume is required for --runtime-backend docker_single"
                    )
                })?,
                container_name_prefix: config.docker_runtime_container_name_prefix.clone(),
                socket_root: config.docker_runtime_socket_root.clone(),
                shm_size: config.docker_runtime_shm_size.clone(),
                start_timeout: Duration::from_secs(config.docker_runtime_start_timeout_secs),
                idle_timeout: Duration::from_secs(config.runtime_idle_timeout_secs),
                max_active_runtimes: 1,
                max_starting_runtimes: 1,
                seccomp_unconfined: config.docker_runtime_seccomp_unconfined,
                env_file: config.docker_runtime_env_file.clone(),
            }),
            "docker_pool" => RuntimeManagerConfig::DockerPool(DockerRuntimeConfig {
                docker_bin: config.docker_runtime_bin.clone(),
                image: config.docker_runtime_image.clone().ok_or_else(|| {
                    anyhow::anyhow!(
                        "--docker-runtime-image is required for --runtime-backend docker_pool"
                    )
                })?,
                network: config.docker_runtime_network.clone().ok_or_else(|| {
                    anyhow::anyhow!(
                        "--docker-runtime-network is required for --runtime-backend docker_pool"
                    )
                })?,
                shared_run_volume: config.docker_runtime_volume.clone().ok_or_else(|| {
                    anyhow::anyhow!(
                        "--docker-runtime-volume is required for --runtime-backend docker_pool"
                    )
                })?,
                container_name_prefix: config.docker_runtime_container_name_prefix.clone(),
                socket_root: config.docker_runtime_socket_root.clone(),
                shm_size: config.docker_runtime_shm_size.clone(),
                start_timeout: Duration::from_secs(config.docker_runtime_start_timeout_secs),
                idle_timeout: Duration::from_secs(config.runtime_idle_timeout_secs),
                max_active_runtimes: config.max_active_runtimes,
                max_starting_runtimes: config.max_starting_runtimes,
                seccomp_unconfined: config.docker_runtime_seccomp_unconfined,
                env_file: config.docker_runtime_env_file.clone(),
            }),
            other => anyhow::bail!("unknown --runtime-backend value: {other}"),
        },
    )?);

    let session_store = if let Some(database_url) = &config.database_url {
        info!("using postgres-backed session control store");
        SessionStore::from_database_url_with_config(database_url, runtime_manager.profile().clone())
            .await?
    } else {
        warn!("no --database-url configured; /api/v1 sessions will use an in-memory store");
        SessionStore::in_memory_with_config(runtime_manager.profile().clone())
    };

    let server = TransportServer::new(
        bind_addr,
        identity,
        runtime_manager.clone(),
        auth_validator.clone(),
        connect_ticket_manager.clone(),
        session_store.clone(),
        Duration::from_secs(config.runtime_idle_timeout_secs),
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
            runtime_manager,
            Duration::from_secs(config.runtime_idle_timeout_secs),
            config.public_gateway_url,
            if config.exclusive_browser_owner {
                SessionOwnerMode::ExclusiveBrowserOwner
            } else {
                SessionOwnerMode::Collaborative
            },
        ) => result,
    }
}
