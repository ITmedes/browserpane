#[path = "compose_api_surface/docker_pool_runtime.rs"]
mod docker_pool_runtime;
#[allow(dead_code)]
#[path = "compose_api_surface/support.rs"]
mod support;

#[tokio::test]
#[ignore = "requires running local compose stack and temporarily switches gateway to docker_pool mode"]
async fn compose_docker_pool_session_capacity_api_surface() -> anyhow::Result<()> {
    let _guard = support::suite_lock().lock().await;
    let harness = support::ComposeHarness::connect().await?;
    docker_pool_runtime::run_session_capacity(&harness).await
}

#[tokio::test]
#[ignore = "requires running local compose stack and temporarily switches gateway to docker_pool mode"]
async fn compose_docker_pool_workflow_admission_api_surface() -> anyhow::Result<()> {
    let _guard = support::suite_lock().lock().await;
    let harness = support::ComposeHarness::connect().await?;
    docker_pool_runtime::run_workflow_admission(&harness).await
}
