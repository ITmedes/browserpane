use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, bail};
use chrono::{Duration as ChronoDuration, Utc};
use tracing::{info, warn};
use wtransport::Identity;

use crate::auth::{AuthValidator, OidcConfig};
use crate::config::Config;
use crate::credentials::{CredentialProvider, VaultKvV2CredentialProvider};
use crate::recording::{RecordingArtifactStore, RecordingObservability, RecordingRetentionManager};
use crate::recording_lifecycle::{RecordingLifecycleManager, RecordingWorkerConfig};
use crate::session_access::{SessionAutomationAccessTokenManager, SessionConnectTicketManager};
use crate::session_control::{SessionOwnerMode, SessionStore};
use crate::session_manager::{SessionManager, SessionManagerConfig, SessionManagerDockerConfig};
use crate::session_registry::SessionRegistry;
use crate::workflow::{WorkflowObservability, WorkflowRetentionManager, WorkflowSourceResolver};
use crate::workflow_event_delivery::{WorkflowEventDeliveryConfig, WorkflowEventDeliveryManager};
use crate::workflow_lifecycle::{WorkflowLifecycleManager, WorkflowWorkerConfig};

pub(super) struct AuthServices {
    pub(super) auth_validator: Arc<AuthValidator>,
    pub(super) connect_ticket_manager: Arc<SessionConnectTicketManager>,
    pub(super) automation_access_token_manager: Arc<SessionAutomationAccessTokenManager>,
}

pub(super) struct RuntimeServices {
    pub(super) bind_addr: SocketAddr,
    pub(super) api_bind_addr: SocketAddr,
    pub(super) identity: Identity,
    pub(super) registry: Arc<SessionRegistry>,
    pub(super) session_manager: Arc<SessionManager>,
    pub(super) session_store: SessionStore,
}

pub(super) struct RecordingServices {
    pub(super) lifecycle: Arc<RecordingLifecycleManager>,
    pub(super) artifact_store: Arc<RecordingArtifactStore>,
    pub(super) observability: Arc<RecordingObservability>,
}

pub(super) struct WorkflowServices {
    pub(super) source_resolver: Arc<WorkflowSourceResolver>,
    pub(super) lifecycle: Arc<WorkflowLifecycleManager>,
    pub(super) observability: Arc<WorkflowObservability>,
    pub(super) log_retention: Option<ChronoDuration>,
    pub(super) output_retention: Option<ChronoDuration>,
}

impl AuthServices {
    pub(super) async fn build(config: &Config) -> anyhow::Result<Self> {
        let shared_secret = load_or_generate_shared_secret(config)?;
        let auth_validator = Arc::new(build_auth_validator(config, &shared_secret).await?);
        Ok(Self {
            connect_ticket_manager: Arc::new(SessionConnectTicketManager::new(
                shared_secret.clone(),
                Duration::from_secs(config.session_ticket_ttl_secs),
            )),
            automation_access_token_manager: Arc::new(SessionAutomationAccessTokenManager::new(
                shared_secret,
                Duration::from_secs(config.session_ticket_ttl_secs),
            )),
            auth_validator,
        })
    }
}

impl RuntimeServices {
    pub(super) async fn build(config: &Config) -> anyhow::Result<Self> {
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

impl RecordingServices {
    pub(super) async fn build(
        config: &Config,
        auth_validator: Arc<AuthValidator>,
        session_store: SessionStore,
    ) -> anyhow::Result<Self> {
        let lifecycle = Arc::new(RecordingLifecycleManager::new(
            build_recording_worker_config(config)?,
            auth_validator,
            session_store.clone(),
        )?);
        lifecycle.reconcile_persisted_state().await?;

        let observability = Arc::new(RecordingObservability::default());
        let artifact_store = Arc::new(RecordingArtifactStore::local_fs(
            config.recording_artifact_local_root.clone(),
        ));

        if config.recording_artifact_cleanup_interval_secs > 0 {
            let retention = Arc::new(RecordingRetentionManager::new(
                session_store,
                artifact_store.clone(),
                observability.clone(),
                Duration::from_secs(config.recording_artifact_cleanup_interval_secs),
            ));
            retention.run_cleanup_pass(Utc::now()).await?;
            retention.start();
        }

        Ok(Self {
            lifecycle,
            artifact_store,
            observability,
        })
    }
}

impl WorkflowServices {
    pub(super) async fn build(
        config: &Config,
        auth_validator: Arc<AuthValidator>,
        automation_access_token_manager: Arc<SessionAutomationAccessTokenManager>,
        session_store: SessionStore,
        session_manager: Arc<SessionManager>,
        registry: Arc<SessionRegistry>,
    ) -> anyhow::Result<Self> {
        let source_resolver =
            Arc::new(WorkflowSourceResolver::new(config.workflow_git_bin.clone()));
        let lifecycle = Arc::new(WorkflowLifecycleManager::new(
            build_workflow_worker_config(config),
            auth_validator,
            automation_access_token_manager,
            session_store.clone(),
            session_manager,
            registry,
        )?);
        lifecycle.reconcile_persisted_state().await?;

        let observability = Arc::new(WorkflowObservability::default());
        let event_delivery = Arc::new(WorkflowEventDeliveryManager::new(
            session_store.clone(),
            observability.clone(),
            WorkflowEventDeliveryConfig {
                poll_interval: Duration::from_millis(
                    config.workflow_event_delivery_poll_interval_ms,
                ),
                request_timeout: Duration::from_secs(config.workflow_event_delivery_timeout_secs),
                max_attempts: config.workflow_event_delivery_max_attempts,
                batch_size: config.workflow_event_delivery_batch_size,
                base_backoff: Duration::from_secs(config.workflow_event_delivery_base_backoff_secs),
            },
        )?);
        event_delivery.reconcile_persisted_state().await?;
        event_delivery.start();

        let log_retention = workflow_retention_window(
            "workflow-log-retention-secs",
            config.workflow_log_retention_secs,
        )?;
        let output_retention = workflow_retention_window(
            "workflow-output-retention-secs",
            config.workflow_output_retention_secs,
        )?;
        if config.workflow_retention_cleanup_interval_secs > 0
            && (log_retention.is_some() || output_retention.is_some())
        {
            let retention = Arc::new(WorkflowRetentionManager::new(
                session_store,
                observability.clone(),
                Duration::from_secs(config.workflow_retention_cleanup_interval_secs),
                log_retention,
                output_retention,
            ));
            retention.run_cleanup_pass(Utc::now()).await?;
            retention.start();
        }

        Ok(Self {
            source_resolver,
            lifecycle,
            observability,
            log_retention,
            output_retention,
        })
    }
}

pub(super) fn build_credential_provider(
    config: &Config,
) -> anyhow::Result<Option<Arc<CredentialProvider>>> {
    match (
        config.credential_vault_addr.clone(),
        config.credential_vault_token.clone(),
    ) {
        (Some(addr), Some(token)) => Ok(Some(Arc::new(CredentialProvider::new(Arc::new(
            VaultKvV2CredentialProvider::new(
                addr,
                token,
                config.credential_vault_mount_path.clone(),
                Some(config.credential_vault_prefix.clone()),
            )?,
        ))))),
        (None, None) => Ok(None),
        _ => bail!("--credential-vault-addr and --credential-vault-token must be set together"),
    }
}

pub(super) fn default_owner_mode(config: &Config) -> SessionOwnerMode {
    if config.exclusive_browser_owner {
        SessionOwnerMode::ExclusiveBrowserOwner
    } else {
        SessionOwnerMode::Collaborative
    }
}

pub(super) fn load_or_generate_shared_secret(config: &Config) -> anyhow::Result<Vec<u8>> {
    match &config.hmac_secret {
        Some(hex_secret) => {
            let decoded = hex::decode(hex_secret)?;
            if decoded.len() < 16 {
                bail!(
                    "HMAC secret must be at least 16 bytes (32 hex chars), got {}",
                    decoded.len()
                );
            }
            Ok(decoded)
        }
        None => {
            let mut secret = vec![0u8; 32];
            rand::fill(&mut secret[..]);
            Ok(secret)
        }
    }
}

async fn build_auth_validator(
    config: &Config,
    shared_secret: &[u8],
) -> anyhow::Result<AuthValidator> {
    if let Some(issuer) = &config.oidc_issuer {
        let audience = config
            .oidc_audience
            .clone()
            .ok_or_else(|| anyhow!("--oidc-audience is required when --oidc-issuer is set"))?;
        info!("using OIDC/JWT auth with issuer {}", issuer);
        if config.token_file.is_some() {
            info!("ignoring --token-file because OIDC auth is enabled");
        }
        AuthValidator::from_oidc(OidcConfig {
            issuer: issuer.clone(),
            audience,
            jwks_url: config.oidc_jwks_url.clone(),
        })
        .await
    } else {
        let validator = AuthValidator::from_hmac_secret(shared_secret.to_vec());
        if let Some(token) = validator.generate_token() {
            info!("generated dev token: {token}");
            if let Some(path) = &config.token_file {
                std::fs::write(path, &token)?;
                info!("wrote token to {}", path.display());
            }
        }
        Ok(validator)
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

pub(super) fn build_session_manager_config(
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

pub(super) fn build_recording_worker_config(
    config: &Config,
) -> anyhow::Result<Option<RecordingWorkerConfig>> {
    let Some(bin) = config.recording_worker_bin.clone() else {
        return Ok(None);
    };
    let chrome_executable = config.recording_worker_chrome.clone().ok_or_else(|| {
        anyhow!("--recording-worker-chrome is required when --recording-worker-bin is set")
    })?;
    Ok(Some(RecordingWorkerConfig {
        bin,
        args: config.recording_worker_args.clone(),
        chrome_executable,
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
    }))
}

fn build_workflow_worker_config(config: &Config) -> Option<WorkflowWorkerConfig> {
    config
        .workflow_worker_image
        .clone()
        .map(|image| WorkflowWorkerConfig {
            docker_bin: config.workflow_worker_docker_bin.clone(),
            image,
            max_active_workers: config.workflow_worker_max_active,
            network: config.workflow_worker_network.clone(),
            container_name_prefix: config.workflow_worker_container_name_prefix.clone(),
            gateway_api_url: config.workflow_worker_api_url.clone(),
            work_root: config.workflow_worker_work_root.clone(),
            bearer_token: config.workflow_worker_bearer_token.clone(),
            oidc_token_url: config.workflow_worker_oidc_token_url.clone(),
            oidc_client_id: config.workflow_worker_oidc_client_id.clone(),
            oidc_client_secret: config.workflow_worker_oidc_client_secret.clone(),
            oidc_scopes: config.workflow_worker_oidc_scopes.clone(),
        })
}

pub(super) fn workflow_retention_window(
    flag_name: &str,
    retention_secs: u64,
) -> anyhow::Result<Option<ChronoDuration>> {
    if retention_secs == 0 {
        return Ok(None);
    }

    Ok(Some(ChronoDuration::seconds(
        i64::try_from(retention_secs).map_err(|error| {
            anyhow!("--{flag_name} is out of range for chrono duration: {error}")
        })?,
    )))
}

fn required_string(
    value: &Option<String>,
    flag_name: &str,
    runtime_backend: &str,
) -> anyhow::Result<String> {
    value
        .clone()
        .ok_or_else(|| anyhow!("{flag_name} is required for --runtime-backend {runtime_backend}"))
}
