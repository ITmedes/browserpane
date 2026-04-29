use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, bail};
use tracing::{info, warn};
use wtransport::Identity;

use crate::config::Config;
use crate::session_control::SessionStore;
use crate::session_manager::{SessionManager, SessionManagerConfig, SessionManagerDockerConfig};
use crate::session_registry::SessionRegistry;

use super::{required_string, RuntimeServices};

impl RuntimeServices {
    pub(in crate::app) async fn build(config: &Config) -> anyhow::Result<Self> {
        let bind_addr = parse_socket_addr(&config.bind, config.port, "gateway bind")?;
        let api_bind_addr = parse_socket_addr(&config.bind, config.api_port, "gateway API bind")?;
        let identity = build_identity(config).await?;
        let registry = Arc::new(SessionRegistry::new(
            config.max_viewers,
            config.exclusive_browser_owner,
        ));
        let session_manager = Arc::new(SessionManager::new(build_session_manager_config(config)?)?);
        let session_store = build_session_store(config, &session_manager).await?;
        session_manager
            .attach_session_store(session_store.clone())
            .await;
        session_manager.reconcile_persisted_state().await?;

        Ok(Self {
            bind_addr,
            api_bind_addr,
            identity,
            registry,
            session_manager,
            session_store,
        })
    }
}

async fn build_identity(config: &Config) -> anyhow::Result<Identity> {
    match (&config.cert, &config.key) {
        (Some(cert_path), Some(key_path)) => Identity::load_pemfiles(cert_path, key_path)
            .await
            .map_err(Into::into),
        _ => {
            info!("generating self-signed certificate for development");
            Identity::self_signed(["localhost", "127.0.0.1"]).map_err(Into::into)
        }
    }
}

fn parse_socket_addr(bind: &str, port: u16, label: &str) -> anyhow::Result<SocketAddr> {
    format!("{bind}:{port}")
        .parse()
        .map_err(|error| anyhow!("invalid {label} address '{bind}:{port}': {error}"))
}

pub(in crate::app) fn build_session_manager_config(
    config: &Config,
) -> anyhow::Result<SessionManagerConfig> {
    let agent_socket_path = config.agent_socket.to_string_lossy().into_owned();
    match config.runtime_backend.as_str() {
        "static_single" => Ok(SessionManagerConfig::StaticSingle {
            agent_socket_path,
            cdp_endpoint: config.runtime_cdp_endpoint.clone(),
            idle_timeout: Duration::from_secs(config.runtime_idle_timeout_secs),
        }),
        "docker_single" => Ok(SessionManagerConfig::DockerSingle(
            build_docker_runtime_config(config, 1, 1)?,
        )),
        "docker_pool" => Ok(SessionManagerConfig::DockerPool(
            build_docker_runtime_config(
                config,
                config.max_active_runtimes,
                config.max_starting_runtimes,
            )?,
        )),
        other => bail!("unknown --runtime-backend value: {other}"),
    }
}

fn build_docker_runtime_config(
    config: &Config,
    max_active_runtimes: usize,
    max_starting_runtimes: usize,
) -> anyhow::Result<SessionManagerDockerConfig> {
    Ok(SessionManagerDockerConfig {
        docker_bin: config.docker_runtime_bin.clone(),
        image: required_string(
            &config.docker_runtime_image,
            "--docker-runtime-image",
            &config.runtime_backend,
        )?,
        network: required_string(
            &config.docker_runtime_network,
            "--docker-runtime-network",
            &config.runtime_backend,
        )?,
        shared_run_volume: required_string(
            &config.docker_runtime_volume,
            "--docker-runtime-volume",
            &config.runtime_backend,
        )?,
        container_name_prefix: config.docker_runtime_container_name_prefix.clone(),
        socket_root: config.docker_runtime_socket_root.clone(),
        cdp_proxy_port: config.docker_runtime_cdp_proxy_port,
        shm_size: config.docker_runtime_shm_size.clone(),
        start_timeout: Duration::from_secs(config.docker_runtime_start_timeout_secs),
        idle_timeout: Duration::from_secs(config.runtime_idle_timeout_secs),
        max_active_runtimes,
        max_starting_runtimes,
        seccomp_unconfined: config.docker_runtime_seccomp_unconfined,
        env_file: config.docker_runtime_env_file.clone(),
    })
}

async fn build_session_store(
    config: &Config,
    session_manager: &SessionManager,
) -> anyhow::Result<SessionStore> {
    if let Some(database_url) = &config.database_url {
        info!("using postgres-backed session control store");
        SessionStore::from_database_url_with_config(database_url, session_manager.profile().clone())
            .await
            .map_err(Into::into)
    } else {
        warn!("no --database-url configured; /api/v1 sessions will use an in-memory store");
        Ok(SessionStore::in_memory_with_config(
            session_manager.profile().clone(),
        ))
    }
}
