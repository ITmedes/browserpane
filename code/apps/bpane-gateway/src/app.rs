use std::sync::Arc;
use std::time::Duration;

use crate::api::{self, ApiServerConfig};
use crate::config::Config;
use crate::transport::{TransportServer, TransportServerConfig};
use crate::workspaces::WorkspaceFileStore;

mod builders;

use builders::{
    build_credential_provider, default_owner_mode, start_session_file_retention, AuthServices,
    RecordingServices, RuntimeServices, WorkflowServices,
};

pub(crate) struct GatewayApp {
    transport_server: TransportServer,
    api_server_config: ApiServerConfig,
}

impl GatewayApp {
    pub(crate) async fn build(config: Config) -> anyhow::Result<Self> {
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
        let credential_provider = build_credential_provider(&config)?;
        let recording_services = RecordingServices::build(
            &config,
            auth_services.auth_validator.clone(),
            runtime_services.session_store.clone(),
        )
        .await?;
        let workflow_services = WorkflowServices::build(
            &config,
            auth_services.auth_validator.clone(),
            auth_services.automation_access_token_manager.clone(),
            runtime_services.session_store.clone(),
            runtime_services.session_manager.clone(),
            runtime_services.registry.clone(),
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
        });

        let api_server_config = ApiServerConfig {
            bind_addr: api_bind_addr,
            registry,
            auth_validator: auth_services.auth_validator,
            connect_ticket_manager: auth_services.connect_ticket_manager,
            automation_access_token_manager: auth_services.automation_access_token_manager,
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
            workflow_log_retention: workflow_services.log_retention,
            workflow_output_retention: workflow_services.output_retention,
            idle_stop_timeout: Duration::from_secs(config.runtime.idle_timeout_secs),
            public_gateway_url: config.gateway.public_gateway_url.clone(),
            default_owner_mode: default_owner_mode(&config),
        };

        Ok(Self {
            transport_server,
            api_server_config,
        })
    }

    pub(crate) async fn run(self) -> anyhow::Result<()> {
        let Self {
            transport_server,
            api_server_config,
        } = self;

        tokio::select! {
            result = transport_server.run() => result,
            result = api::run_api_server(api_server_config) => result,
        }
    }
}

#[cfg(test)]
mod tests;
