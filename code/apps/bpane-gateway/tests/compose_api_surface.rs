#[path = "compose_api_surface/mod.rs"]
mod suite;

use suite::{
    credentials_extensions, sessions_recordings, support, workflows_events, workspaces_automation,
};

#[tokio::test]
#[ignore = "requires running local compose stack"]
async fn compose_sessions_and_recordings_api_surface() -> anyhow::Result<()> {
    let _guard = support::suite_lock().lock().await;
    let harness = support::ComposeHarness::connect().await?;
    harness.cleanup_active_sessions().await?;
    sessions_recordings::run(&harness).await
}

#[tokio::test]
#[ignore = "requires running local compose stack"]
async fn compose_file_workspaces_and_automation_api_surface() -> anyhow::Result<()> {
    let _guard = support::suite_lock().lock().await;
    let harness = support::ComposeHarness::connect().await?;
    harness.cleanup_active_sessions().await?;
    workspaces_automation::run(&harness).await
}

#[tokio::test]
#[ignore = "requires running local compose stack"]
async fn compose_workflows_and_event_delivery_api_surface() -> anyhow::Result<()> {
    let _guard = support::suite_lock().lock().await;
    let harness = support::ComposeHarness::connect().await?;
    harness.cleanup_active_sessions().await?;
    workflows_events::run(&harness).await
}

#[tokio::test]
#[ignore = "requires running local compose stack"]
async fn compose_credentials_and_extensions_api_surface() -> anyhow::Result<()> {
    let _guard = support::suite_lock().lock().await;
    let harness = support::ComposeHarness::connect().await?;
    harness.cleanup_active_sessions().await?;
    credentials_extensions::run(&harness).await
}
