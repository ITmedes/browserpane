mod api;
mod app;
mod auth;
mod automation_access_token;
mod automation_task;
mod config;
mod connect_ticket;
mod credential_binding;
mod credential_provider;
mod extension;
mod file_workspace;
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
mod workflow;
mod workflow_event_delivery;
mod workflow_lifecycle;
mod workflow_observability;
mod workflow_retention;
mod workflow_source;
mod workspace_file_store;

use app::GatewayApp;
use clap::Parser;
use config::Config;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = Config::parse();
    GatewayApp::build(config).await?.run().await
}
