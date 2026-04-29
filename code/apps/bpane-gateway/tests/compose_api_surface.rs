#[path = "compose_api_surface/mod.rs"]
mod suite;

use suite::{
    credentials_extensions, recording_artifacts, session_churn, sessions_recordings, support,
    workflow_run_controls, workflows_events, workspaces_automation,
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

#[tokio::test]
#[ignore = "requires running local compose stack"]
async fn compose_recording_artifacts_and_playback_api_surface() -> anyhow::Result<()> {
    let _guard = support::suite_lock().lock().await;
    let harness = support::ComposeHarness::connect().await?;
    harness.cleanup_active_sessions().await?;
    recording_artifacts::run(&harness).await
}

#[tokio::test]
#[ignore = "requires running local compose stack"]
async fn compose_workflow_run_controls_api_surface() -> anyhow::Result<()> {
    let _guard = support::suite_lock().lock().await;
    let harness = support::ComposeHarness::connect().await?;
    harness.cleanup_active_sessions().await?;
    workflow_run_controls::run(&harness).await
}

#[tokio::test]
#[ignore = "requires running local compose stack"]
async fn compose_session_churn_api_surface() -> anyhow::Result<()> {
    let _guard = support::suite_lock().lock().await;
    let harness = support::ComposeHarness::connect().await?;
    harness.cleanup_active_sessions().await?;
    session_churn::run(&harness).await
}
