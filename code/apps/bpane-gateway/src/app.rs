use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, bail};
use tracing::{info, warn};

use crate::api::{
    self, ApiServerConfig, BrowserContextImportArchiveLimits, BrowserContextImportService,
    McpBridgeControlConfig,
};
use crate::config::Config;
use crate::lifecycle::GatewayLifecycle;
use crate::readiness::GatewayReadiness;
use crate::transport::{TransportServer, TransportServerConfig};
use crate::worker_runtime_control::WorkerRuntimeControl;
use crate::workspaces::WorkspaceFileStore;

mod builders;

use builders::{
    build_credential_provider, default_owner_mode, resolve_optional_secret,
    start_browser_context_retention, start_session_file_retention, AuthServices, RecordingServices,
    RuntimeServices, SecretFilePermissions, WorkflowServices,
};

pub(crate) struct GatewayApp {
    transport_server: TransportServer,
    api_server_config: ApiServerConfig,
    lifecycle: Arc<GatewayLifecycle>,
    readiness: Arc<GatewayReadiness>,
    shutdown_drain_timeout: Duration,
    shutdown_readiness_grace: Duration,
}

impl GatewayApp {
    pub(crate) async fn build(config: Config) -> anyhow::Result<Self> {
        validate_operational_timeouts(&config)?;
        let browser_context_import = build_browser_context_import_service(&config)?;
        let lifecycle = Arc::new(GatewayLifecycle::new());
        let auth_services = AuthServices::build(&config).await?;
        let runtime_services = RuntimeServices::build(&config).await?;
        let workspace_file_store = Arc::new(WorkspaceFileStore::local_fs(
            config.storage.file_workspace_local_root.clone(),
        ));
        runtime_services
            .session_manager
            .attach_workspace_file_store(workspace_file_store.clone())
            .await;
        runtime_services
            .registry
            .attach_session_file_recording(
                runtime_services.session_store.clone(),
                workspace_file_store.clone(),
            )
            .await;
        start_session_file_retention(
            &config,
            runtime_services.session_store.clone(),
            workspace_file_store.clone(),
        )
        .await?;
        start_browser_context_retention(
            &config,
            runtime_services.session_store.clone(),
            runtime_services.session_manager.clone(),
        )
        .await?;
        let credential_provider = build_credential_provider(&config)?;
        runtime_services
            .session_manager
            .attach_credential_provider(credential_provider.clone())
            .await;
        let worker_control = WorkerRuntimeControl::from_broker(
            runtime_services.session_manager.runtime_broker_client(),
        );
        let recording_services = RecordingServices::build(
            &config,
            auth_services.auth_validator.clone(),
            auth_services.connect_ticket_manager.clone(),
            auth_services.automation_access_token_manager.clone(),
            auth_services.recording_worker_access_token_manager.clone(),
            runtime_services.session_store.clone(),
            worker_control.clone(),
        )
        .await?;
        let workflow_services = WorkflowServices::build(
            &config,
            auth_services.auth_validator.clone(),
            auth_services.automation_access_token_manager.clone(),
            runtime_services.session_store.clone(),
            runtime_services.session_manager.clone(),
            runtime_services.registry.clone(),
            worker_control,
        )
        .await?;
        let RuntimeServices {
            bind_addr,
            api_bind_addr,
            identity,
            registry,
            session_manager,
            session_store,
        } = runtime_services;

        let transport_server = TransportServer::new(TransportServerConfig {
            bind_addr,
            identity,
            session_manager: session_manager.clone(),
            auth_validator: auth_services.auth_validator.clone(),
            connect_ticket_manager: auth_services.connect_ticket_manager.clone(),
            session_store: session_store.clone(),
            workspace_file_store: workspace_file_store.clone(),
            recording_lifecycle: recording_services.lifecycle.clone(),
            idle_stop_timeout: Duration::from_secs(config.runtime.idle_timeout_secs),
            heartbeat_timeout: Duration::from_secs(config.gateway.heartbeat_timeout_secs),
            registry: registry.clone(),
            lifecycle: lifecycle.clone(),
        });

        let readiness = Arc::new(GatewayReadiness::new(
            lifecycle.clone(),
            session_store.clone(),
            session_manager.clone(),
            credential_provider.clone(),
            recording_services.artifact_store.clone(),
            workspace_file_store.clone(),
            Duration::from_secs(config.gateway.readiness_check_timeout_secs),
        ));

        let api_server_config = ApiServerConfig {
            bind_addr: api_bind_addr,
            registry,
            auth_validator: auth_services.auth_validator,
            admin_event_access_token_manager: auth_services.admin_event_access_token_manager,
            connect_ticket_manager: auth_services.connect_ticket_manager,
            automation_access_token_manager: auth_services.automation_access_token_manager,
            recording_worker_access_token_manager: auth_services
                .recording_worker_access_token_manager,
            session_store,
            session_manager,
            credential_provider,
            recording_artifact_store: recording_services.artifact_store,
            workspace_file_store,
            workflow_source_resolver: workflow_services.source_resolver,
            recording_observability: recording_services.observability,
            recording_lifecycle: recording_services.lifecycle,
            workflow_lifecycle: workflow_services.lifecycle,
            workflow_observability: workflow_services.observability,
            workflow_event_destination_policy: workflow_services.event_destination_policy,
            workflow_log_retention: workflow_services.log_retention,
            workflow_output_retention: workflow_services.output_retention,
            idle_stop_timeout: Duration::from_secs(config.runtime.idle_timeout_secs),
            public_gateway_url: config.gateway.public_gateway_url.clone(),
            default_owner_mode: default_owner_mode(&config),
            browser_context_import,
            mcp_bridge_control: mcp_bridge_control_config(&config)?,
        };

        Ok(Self {
            transport_server,
            api_server_config,
            lifecycle,
            readiness,
            shutdown_drain_timeout: Duration::from_secs(config.gateway.shutdown_drain_timeout_secs),
            shutdown_readiness_grace: Duration::from_secs(
                config.gateway.shutdown_readiness_grace_secs,
            ),
        })
    }

    pub(crate) async fn run(self) -> anyhow::Result<()> {
        let Self {
            transport_server,
            api_server_config,
            lifecycle,
            readiness,
            shutdown_drain_timeout,
            shutdown_readiness_grace,
        } = self;

        lifecycle.mark_running();
        let mut transport_task = tokio::spawn(transport_server.run());
        let transport_abort = transport_task.abort_handle();
        let api_lifecycle = lifecycle.clone();
        let mut api_task = tokio::spawn(api::run_api_server(
            api_server_config,
            api_lifecycle,
            readiness,
            shutdown_readiness_grace,
        ));
        let api_abort = api_task.abort_handle();

        let mut transport_complete = false;
        let mut api_complete = false;
        let mut server_error = None;
        tokio::select! {
            signal = wait_for_shutdown_signal() => {
                info!(signal = signal?, "gateway shutdown signal received");
            }
            result = &mut transport_task => {
                transport_complete = true;
                server_error = joined_server_error("WebTransport", result);
            }
            result = &mut api_task => {
                api_complete = true;
                server_error = joined_server_error("HTTP API", result);
            }
        }

        if lifecycle.begin_draining() {
            info!(
                drain_timeout_secs = shutdown_drain_timeout.as_secs(),
                "gateway drain started"
            );
        }

        let drain = async {
            let mut drain_error = None;
            if !transport_complete {
                let result = (&mut transport_task).await;
                drain_error = joined_server_error("WebTransport", result);
            }
            if !api_complete {
                let result = (&mut api_task).await;
                drain_error = drain_error.or_else(|| joined_server_error("HTTP API", result));
            }
            drain_error
        };
        tokio::pin!(drain);

        tokio::select! {
            drain_result = &mut drain => {
                server_error = server_error.or(drain_result);
                info!("gateway drain completed");
            }
            signal = wait_for_shutdown_signal() => {
                warn!(signal = signal?, "second shutdown signal forced gateway termination");
                transport_abort.abort();
                api_abort.abort();
            }
            () = tokio::time::sleep(shutdown_drain_timeout) => {
                warn!("gateway drain deadline reached; terminating remaining work");
                transport_abort.abort();
                api_abort.abort();
            }
        }

        server_error.map_or(Ok(()), Err)
    }
}

fn validate_operational_timeouts(config: &Config) -> anyhow::Result<()> {
    if config.gateway.readiness_check_timeout_secs == 0 {
        bail!("--readiness-check-timeout-secs must be greater than zero");
    }
    if config.gateway.shutdown_drain_timeout_secs == 0 {
        bail!("--shutdown-drain-timeout-secs must be greater than zero");
    }
    if config.gateway.shutdown_readiness_grace_secs >= config.gateway.shutdown_drain_timeout_secs {
        bail!("--shutdown-readiness-grace-secs must be less than --shutdown-drain-timeout-secs");
    }
    Ok(())
}

fn build_browser_context_import_service(
    config: &Config,
) -> anyhow::Result<BrowserContextImportService> {
    let gateway = &config.gateway;
    if gateway.browser_context_import_max_archive_bytes == 0 {
        bail!("--browser-context-import-max-archive-bytes must be greater than zero");
    }
    if gateway.browser_context_import_max_profile_archive_bytes == 0 {
        bail!("--browser-context-import-max-profile-archive-bytes must be greater than zero");
    }
    if gateway.browser_context_import_max_profile_uncompressed_bytes == 0 {
        bail!("--browser-context-import-max-profile-uncompressed-bytes must be greater than zero");
    }
    if gateway.browser_context_import_max_profile_entries == 0 {
        bail!("--browser-context-import-max-profile-entries must be greater than zero");
    }
    if gateway.browser_context_import_max_profile_archive_bytes
        > gateway.browser_context_import_max_archive_bytes
    {
        bail!(
            "--browser-context-import-max-profile-archive-bytes must not exceed --browser-context-import-max-archive-bytes"
        );
    }
    usize::try_from(gateway.browser_context_import_max_archive_bytes).map_err(|_| {
        anyhow!(
            "--browser-context-import-max-archive-bytes exceeds this platform's request-body limit"
        )
    })?;
    let max_concurrent = NonZeroUsize::new(gateway.browser_context_import_max_concurrent)
        .ok_or_else(|| {
            anyhow!("--browser-context-import-max-concurrent must be greater than zero")
        })?;

    Ok(BrowserContextImportService::new(
        BrowserContextImportArchiveLimits {
            max_archive_bytes: gateway.browser_context_import_max_archive_bytes,
            max_manifest_bytes: 128 * 1024,
            max_profile_archive_bytes: gateway.browser_context_import_max_profile_archive_bytes,
            max_profile_uncompressed_bytes: gateway
                .browser_context_import_max_profile_uncompressed_bytes,
            max_profile_entries: gateway.browser_context_import_max_profile_entries,
            max_profile_path_bytes: 4096,
        },
        max_concurrent,
    ))
}

fn joined_server_error(
    name: &str,
    result: Result<anyhow::Result<()>, tokio::task::JoinError>,
) -> Option<anyhow::Error> {
    match result {
        Ok(Ok(())) => None,
        Ok(Err(error)) => Some(error.context(format!("{name} server failed"))),
        Err(error) => Some(anyhow!("{name} server task failed: {error}")),
    }
}

#[cfg(unix)]
async fn wait_for_shutdown_signal() -> anyhow::Result<&'static str> {
    use tokio::signal::unix::{signal, SignalKind};

    let mut terminate = signal(SignalKind::terminate())?;
    tokio::select! {
        result = tokio::signal::ctrl_c() => {
            result?;
            Ok("SIGINT")
        }
        _ = terminate.recv() => Ok("SIGTERM"),
    }
}

#[cfg(not(unix))]
async fn wait_for_shutdown_signal() -> anyhow::Result<&'static str> {
    tokio::signal::ctrl_c().await?;
    Ok("SIGINT")
}

fn mcp_bridge_control_config(config: &Config) -> anyhow::Result<Option<McpBridgeControlConfig>> {
    let Some(control_url) = config.gateway.mcp_bridge_control_url.as_deref() else {
        return Ok(None);
    };
    let control_url = control_url.trim();
    if control_url.is_empty() {
        return Ok(None);
    }
    let bearer_token = resolve_optional_secret(
        config.gateway.mcp_bridge_control_token.as_deref(),
        config.gateway.mcp_bridge_control_token_file.as_deref(),
        "--mcp-bridge-control-token",
        "--mcp-bridge-control-token-file",
        SecretFilePermissions::OwnerOnly,
    )?;
    Ok(Some(McpBridgeControlConfig {
        control_url: control_url.to_string(),
        bearer_token,
        timeout: Duration::from_secs(config.gateway.mcp_bridge_control_timeout_secs),
    }))
}

#[cfg(test)]
mod tests;
