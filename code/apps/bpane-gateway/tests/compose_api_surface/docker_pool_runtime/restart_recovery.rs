use std::time::Duration;

use anyhow::{anyhow, Result};
use serde_json::json;

use crate::support::{json_id, ComposeHarness};

use super::{
    configure_gateway, create_docker_pool_workflow_repo, create_workflow_run,
    publish_docker_pool_workflow, wait_for_workflow_run_state, workflow_run_event_types,
};

pub async fn run(harness: &ComposeHarness) -> Result<()> {
    let _restore = harness.compose_service_restore_guard(&["gateway"]);
    configure_gateway(
        harness,
        &[
            ("BPANE_GATEWAY_MAX_ACTIVE_RUNTIMES", "4"),
            ("BPANE_WORKFLOW_WORKER_MAX_ACTIVE", "1"),
        ],
    )
    .await?;
    harness.cleanup_active_sessions().await?;
    harness.ensure_workflow_worker_image().await?;

    let workflow_repo = create_docker_pool_workflow_repo(harness).await?;
    let workflow_id = publish_docker_pool_workflow(
        harness,
        "compose-docker-pool-restart-recovery",
        "Exercise queued workflow restart recovery in docker_pool mode",
        "docker-pool-workflow-restart-recovery",
        &workflow_repo.repository_url,
        &workflow_repo.commit,
    )
    .await?;

    let request_prefix = format!("docker-pool-restart-recovery-{}", uuid::Uuid::now_v7());
    let active_run = create_workflow_run(
        harness,
        &workflow_id,
        json!({ "hold_ms": 8000 }),
        &format!("{request_prefix}-active"),
        "docker-pool-workflow-restart-recovery",
    )
    .await?;
    let active_run_id = json_id(&active_run, "id")?;

    wait_for_workflow_run_state(
        harness,
        &active_run_id,
        &["running", "starting"],
        Duration::from_secs(20),
        "docker_pool active workflow run start before restart",
    )
    .await?;

    let queued_run = create_workflow_run(
        harness,
        &workflow_id,
        json!({ "hold_ms": 0 }),
        &format!("{request_prefix}-queued"),
        "docker-pool-workflow-restart-recovery",
    )
    .await?;
    let queued_run_id = json_id(&queued_run, "id")?;
    if queued_run["state"] != json!("queued") {
        return Err(anyhow!(
            "docker_pool restart-recovery queued run was not created in queued state: {queued_run}"
        ));
    }

    configure_gateway(
        harness,
        &[
            ("BPANE_GATEWAY_MAX_ACTIVE_RUNTIMES", "4"),
            ("BPANE_WORKFLOW_WORKER_MAX_ACTIVE", "1"),
        ],
    )
    .await?;

    let _active_terminal = wait_for_workflow_run_state(
        harness,
        &active_run_id,
        &["succeeded", "failed", "cancelled", "timed_out"],
        Duration::from_secs(40),
        "docker_pool active workflow run terminal state after restart",
    )
    .await?;

    let resumed_queued = wait_for_workflow_run_state(
        harness,
        &queued_run_id,
        &["succeeded", "failed", "cancelled", "timed_out"],
        Duration::from_secs(40),
        "docker_pool queued workflow completion after restart",
    )
    .await?;
    if resumed_queued["state"] != json!("succeeded") {
        return Err(anyhow!(
            "docker_pool queued run did not succeed after gateway restart: {resumed_queued}"
        ));
    }

    let queued_events = workflow_run_event_types(harness, &queued_run_id).await?;
    for expected in [
        "workflow_run.queued",
        "workflow_run.running",
        "workflow_run.succeeded",
    ] {
        if !queued_events.iter().any(|event| event == expected) {
            return Err(anyhow!(
                "docker_pool queued restart-recovery run is missing expected event {expected}: {queued_events:?}"
            ));
        }
    }

    if queued_events
        .iter()
        .filter(|event| event.as_str() == "workflow_run.running")
        .count()
        != 1
    {
        return Err(anyhow!(
            "docker_pool queued restart-recovery run emitted workflow_run.running multiple times: {queued_events:?}"
        ));
    }
    if queued_events
        .iter()
        .filter(|event| event.as_str() == "automation_task.running")
        .count()
        != 1
    {
        return Err(anyhow!(
            "docker_pool queued restart-recovery run emitted automation_task.running multiple times: {queued_events:?}"
        ));
    }

    harness.cleanup_active_sessions().await?;
    Ok(())
}
