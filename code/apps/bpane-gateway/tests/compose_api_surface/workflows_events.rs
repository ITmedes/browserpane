use std::time::Duration;

use anyhow::{anyhow, Result};
use serde_json::json;

use super::support::{
    json_array, json_id, label_map, recording_policy, ComposeHarness,
};

pub async fn run(harness: &ComposeHarness) -> Result<()> {
    harness.ensure_workflow_worker_image().await?;
    let source = harness.create_local_workflow_repo().await?;

    let subscription = harness
        .post_json(
            "/api/v1/workflow-event-subscriptions",
            json!({
                "name": harness.unique_name("compose-e2e-workflow-events"),
                "target_url": "http://127.0.0.1:1/workflow-events",
                "event_types": [
                    "workflow_run.created",
                    "workflow_run.running",
                    "workflow_run.succeeded"
                ],
                "signing_secret": "compose-e2e-signing-secret",
            }),
        )
        .await?;
    let subscription_id = json_id(&subscription, "id")?;

    let listed_subscriptions = harness
        .get_json("/api/v1/workflow-event-subscriptions")
        .await?;
    let listed_subscriptions = json_array(&listed_subscriptions, "subscriptions")?;
    if !listed_subscriptions
        .iter()
        .any(|candidate| candidate.get("id") == Some(&json!(subscription_id)))
    {
        return Err(anyhow!(
            "workflow event subscription {subscription_id} missing from list endpoint"
        ));
    }

    let output_workspace = harness
        .post_json(
            "/api/v1/file-workspaces",
            json!({
                "name": harness.unique_name("compose-e2e-workflow-output"),
                "description": "Compose-backed workflow outputs",
                "labels": label_map("workflow-output-workspace"),
            }),
        )
        .await?;
    let output_workspace_id = json_id(&output_workspace, "id")?;

    let input_workspace = harness
        .post_json(
            "/api/v1/file-workspaces",
            json!({
                "name": harness.unique_name("compose-e2e-workflow-input"),
                "description": "Compose-backed workflow inputs",
                "labels": label_map("workflow-input-workspace"),
            }),
        )
        .await?;
    let input_workspace_id = json_id(&input_workspace, "id")?;

    let input_file = harness
        .post_bytes(
            &format!("/api/v1/file-workspaces/{input_workspace_id}/files"),
            b"compose workflow input bytes".to_vec(),
            "text/plain; charset=utf-8",
            &[("x-bpane-file-name", "input.txt")],
        )
        .await?;
    let input_file_id = json_id(&input_file, "id")?;

    let workflow = harness
        .post_json(
            "/api/v1/workflows",
            json!({
                "name": harness.unique_name("compose-e2e-workflow"),
                "description": "Compose-backed workflow API e2e",
                "labels": label_map("workflow-definition"),
            }),
        )
        .await?;
    let workflow_id = json_id(&workflow, "id")?;

    let version = harness
        .post_json(
            &format!("/api/v1/workflows/{workflow_id}/versions"),
            json!({
                "version": "v1",
                "executor": "playwright",
                "entrypoint": "workflows/smoke/run.mjs",
                "source": {
                    "kind": "git",
                    "repository_url": source.repository_url,
                    "ref": "refs/heads/main",
                    "root_path": "workflows",
                },
                "input_schema": {
                    "type": "object",
                    "required": ["target_url", "output_workspace_id"],
                },
                "output_schema": {
                    "type": "object",
                    "required": ["title", "final_url", "output_file_name"],
                },
                "default_session": {
                    "labels": label_map("workflow-default-session"),
                    "recording": recording_policy("manual"),
                },
                "allowed_file_workspace_ids": [output_workspace_id, input_workspace_id],
            }),
        )
        .await?;
    if version["source"]["resolved_commit"] != json!(source.commit) {
        return Err(anyhow!("workflow version did not resolve the expected commit"));
    }

    let fetched_version = harness
        .get_json(&format!("/api/v1/workflows/{workflow_id}/versions/v1"))
        .await?;
    if fetched_version["entrypoint"] != json!("workflows/smoke/run.mjs") {
        return Err(anyhow!("workflow definition version lookup did not round-trip"));
    }

    let run = harness
        .post_json(
            "/api/v1/workflow-runs",
            json!({
                "workflow_id": workflow_id,
                "version": "v1",
                "client_request_id": harness.unique_name("compose-e2e-client-request"),
                "source_system": "bpane-gateway-compose-e2e",
                "source_reference": "workflow-run-surface",
                "input": {
                    "target_url": "http://web:8080",
                    "output_workspace_id": output_workspace_id,
                },
                "workspace_inputs": [{
                    "workspace_id": input_workspace_id,
                    "file_id": input_file_id,
                    "mount_path": "inputs/input.txt",
                }],
                "labels": label_map("workflow-run"),
            }),
        )
        .await?;
    let run_id = json_id(&run, "id")?;
    let session_id = json_id(&run, "session_id")?;

    let completed_run = harness
        .poll_json(
            "workflow run completion",
            Duration::from_secs(60),
            |value| value["state"] == json!("succeeded"),
            &format!("/api/v1/workflow-runs/{run_id}"),
        )
        .await?;

    let produced_files = json_array(&completed_run, "produced_files")?;
    if produced_files.is_empty() {
        return Err(anyhow!("workflow run did not expose produced files"));
    }

    let events = harness
        .get_json(&format!("/api/v1/workflow-runs/{run_id}/events"))
        .await?;
    let events = json_array(&events, "events")?;
    if events.len() < 3 {
        return Err(anyhow!("workflow run events did not capture lifecycle transitions"));
    }

    let logs = harness
        .get_json(&format!("/api/v1/workflow-runs/{run_id}/logs"))
        .await?;
    let logs = json_array(&logs, "logs")?;
    if logs.is_empty() {
        return Err(anyhow!("workflow run logs are unexpectedly empty"));
    }

    let automation_access = harness
        .post_json(
            &format!("/api/v1/sessions/{session_id}/automation-access"),
            json!({}),
        )
        .await?;
    let automation_token = json_id(&automation_access, "token")?;

    let snapshot_path = completed_run["source_snapshot"]["content_path"]
        .as_str()
        .ok_or_else(|| anyhow!("workflow run is missing a source snapshot content path"))?;
    let snapshot_bytes = harness
        .get_bytes_with_automation_token(snapshot_path, &automation_token)
        .await?;
    if snapshot_bytes.is_empty() {
        return Err(anyhow!("workflow source snapshot content endpoint returned empty bytes"));
    }

    let workspace_inputs = json_array(&completed_run, "workspace_inputs")?;
    let workspace_input = workspace_inputs
        .first()
        .ok_or_else(|| anyhow!("workflow run did not expose workspace input metadata"))?;
    let workspace_input_id = json_id(workspace_input, "id")?;
    let workspace_input_bytes = harness
        .get_bytes_with_automation_token(
            &format!("/api/v1/workflow-runs/{run_id}/workspace-inputs/{workspace_input_id}/content"),
            &automation_token,
        )
        .await?;
    if workspace_input_bytes != b"compose workflow input bytes" {
        return Err(anyhow!("workflow run workspace input content endpoint returned wrong bytes"));
    }

    let deliveries = super::support::poll_until(
        "workflow event deliveries",
        Duration::from_secs(30),
        || async {
            let response = harness
                .get_json(&format!(
                    "/api/v1/workflow-event-subscriptions/{subscription_id}/deliveries"
                ))
                .await?;
            let deliveries = json_array(&response, "deliveries")?;
            if deliveries.is_empty() {
                return Ok(None);
            }
            Ok(Some(response))
        },
    )
    .await?;
    let deliveries = json_array(&deliveries, "deliveries")?;
    if deliveries.is_empty() {
        return Err(anyhow!("workflow event deliveries endpoint remained empty"));
    }

    let deleted_subscription = harness
        .delete_json(&format!(
            "/api/v1/workflow-event-subscriptions/{subscription_id}"
        ))
        .await?;
    if deleted_subscription["id"] != json!(subscription_id) {
        return Err(anyhow!("workflow event subscription delete returned the wrong resource"));
    }

    let _deleted_session = harness
        .delete_json(&format!("/api/v1/sessions/{session_id}"))
        .await?;

    Ok(())
}
