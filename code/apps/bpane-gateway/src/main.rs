mod api;
mod app;
mod auth;
mod automation_tasks;
mod browser_contexts;
mod config;
mod credentials;
mod extensions;
mod idle_stop;
mod lifecycle;
mod metrics;
mod readiness;
mod recording;
mod recording_lifecycle;
mod relay;
mod runtime_manager;
mod session_access;
mod session_control;
mod session_files;
mod session_hub;
mod session_manager;
mod session_registry;
mod transport;
mod worker_process_output;
mod worker_runtime_control;
mod workflow;
mod workflow_endpoints;
mod workflow_event_delivery;
mod workflow_lifecycle;
mod workspaces;

use app::GatewayApp;
use clap::Parser;
use config::Config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let telemetry = bpane_telemetry::init("bpane-gateway", "info")?;
    let result = async {
        let config = Config::parse();
        GatewayApp::build(config).await?.run().await
    }
    .await;
    let shutdown_result = telemetry.shutdown();

    result?;
    shutdown_result?;
    Ok(())
}
