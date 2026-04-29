mod api;
mod app;
mod auth;
mod automation_tasks;
mod config;
mod credentials;
mod extensions;
mod idle_stop;
mod recording;
mod recording_lifecycle;
mod relay;
mod runtime_manager;
mod session_access;
mod session_control;
mod session_hub;
mod session_manager;
mod session_registry;
mod transport;
mod workflow;
mod workflow_event_delivery;
mod workflow_lifecycle;
mod workspaces;

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
