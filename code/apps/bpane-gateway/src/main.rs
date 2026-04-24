mod api;
mod auth;
mod automation_access_token;
mod config;
mod connect_ticket;
mod idle_stop;
mod recording_artifact_store;
mod recording_lifecycle;
mod recording_observability;
mod recording_playback;
mod recording_retention;
mod relay;
mod runtime_manager;
mod session;
mod session_control;
mod session_hub;
mod session_manager;
mod session_registry;
mod transport;

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use clap::Parser;
use tracing::info;
use tracing::warn;
use tracing_subscriber::EnvFilter;
use wtransport::Identity;

use auth::{AuthValidator, OidcConfig};
use automation_access_token::SessionAutomationAccessTokenManager;
use config::Config;
use connect_ticket::SessionConnectTicketManager;
use recording_artifact_store::RecordingArtifactStore;
use recording_lifecycle::{RecordingLifecycleManager, RecordingWorkerConfig};
use recording_observability::RecordingObservability;
use recording_retention::RecordingRetentionManager;
use session_control::{SessionOwnerMode, SessionStore};
use session_manager::{SessionManager, SessionManagerConfig, SessionManagerDockerConfig};
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
        shared_secret.clone(),
        Duration::from_secs(config.session_ticket_ttl_secs),
    ));
    let automation_access_token_manager = Arc::new(SessionAutomationAccessTokenManager::new(
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
    let session_manager = Arc::new(SessionManager::new(
        match config.runtime_backend.as_str() {
            "static_single" => SessionManagerConfig::StaticSingle {
                agent_socket_path: agent_socket_str,
                cdp_endpoint: config.runtime_cdp_endpoint.clone(),
                idle_timeout: Duration::from_secs(config.runtime_idle_timeout_secs),
            },
            "docker_single" => SessionManagerConfig::DockerSingle(SessionManagerDockerConfig {
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
                cdp_proxy_port: config.docker_runtime_cdp_proxy_port,
                shm_size: config.docker_runtime_shm_size.clone(),
                start_timeout: Duration::from_secs(config.docker_runtime_start_timeout_secs),
                idle_timeout: Duration::from_secs(config.runtime_idle_timeout_secs),
                max_active_runtimes: 1,
                max_starting_runtimes: 1,
                seccomp_unconfined: config.docker_runtime_seccomp_unconfined,
                env_file: config.docker_runtime_env_file.clone(),
            }),
            "docker_pool" => SessionManagerConfig::DockerPool(SessionManagerDockerConfig {
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
                cdp_proxy_port: config.docker_runtime_cdp_proxy_port,
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
        SessionStore::from_database_url_with_config(database_url, session_manager.profile().clone())
            .await?
    } else {
        warn!("no --database-url configured; /api/v1 sessions will use an in-memory store");
        SessionStore::in_memory_with_config(session_manager.profile().clone())
    };

    session_manager
        .attach_session_store(session_store.clone())
        .await;
    session_manager.reconcile_persisted_state().await?;
    let recording_worker_config = if let Some(bin) = config.recording_worker_bin.clone() {
        Some(RecordingWorkerConfig {
            bin,
            args: config.recording_worker_args.clone(),
            chrome_executable: config.recording_worker_chrome.clone().ok_or_else(|| {
                anyhow::anyhow!(
                    "--recording-worker-chrome is required when --recording-worker-bin is set"
                )
            })?,
            gateway_api_url: config.recording_worker_api_url.clone(),
            page_url: config.recording_worker_page_url.clone(),
            output_root: config.recording_worker_output_root.clone(),
            cert_spki: config.recording_worker_cert_spki.clone(),
            headless: config.recording_worker_headless,
            connect_timeout: Duration::from_secs(config.recording_worker_connect_timeout_secs),
            poll_interval: Duration::from_millis(config.recording_worker_poll_interval_ms),
            finalize_timeout: Duration::from_secs(config.recording_worker_finalize_timeout_secs),
            bearer_token: config.recording_worker_bearer_token.clone(),
            oidc_token_url: config.recording_worker_oidc_token_url.clone(),
            oidc_client_id: config.recording_worker_oidc_client_id.clone(),
            oidc_client_secret: config.recording_worker_oidc_client_secret.clone(),
            oidc_scopes: config.recording_worker_oidc_scopes.clone(),
        })
    } else {
        None
    };
    let recording_lifecycle = Arc::new(RecordingLifecycleManager::new(
        recording_worker_config,
        auth_validator.clone(),
        session_store.clone(),
    )?);
    recording_lifecycle.reconcile_persisted_state().await?;
    let recording_artifact_store = Arc::new(RecordingArtifactStore::local_fs(
        config.recording_artifact_local_root.clone(),
    ));
    let recording_observability = Arc::new(RecordingObservability::default());
    if config.recording_artifact_cleanup_interval_secs > 0 {
        let recording_retention = Arc::new(RecordingRetentionManager::new(
            session_store.clone(),
            recording_artifact_store.clone(),
            recording_observability.clone(),
            Duration::from_secs(config.recording_artifact_cleanup_interval_secs),
        ));
        recording_retention.run_cleanup_pass(Utc::now()).await?;
        recording_retention.start();
    }

    let server = TransportServer::new(
        bind_addr,
        identity,
        session_manager.clone(),
        auth_validator.clone(),
        connect_ticket_manager.clone(),
        session_store.clone(),
        recording_lifecycle.clone(),
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
            automation_access_token_manager,
            session_store,
            session_manager,
            recording_artifact_store,
            recording_observability,
            recording_lifecycle,
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
