use anyhow::{anyhow, Result};
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde_json::json;
use uuid::Uuid;

use super::support::{json_array, json_id, label_map, ComposeHarness};

pub async fn run(harness: &ComposeHarness) -> Result<()> {
    let output_workspace = harness
        .post_json(
            "/api/v1/file-workspaces",
            json!({
                "name": harness.unique_name("compose-e2e-workflow-control-output"),
                "description": "Compose-backed workflow control outputs",
                "labels": label_map("workflow-control-output-workspace"),
            }),
        )
        .await?;
    let output_workspace_id = json_id(&output_workspace, "id")?;

    let workflow = harness
        .post_json(
            "/api/v1/workflows",
            json!({
                "name": harness.unique_name("compose-e2e-workflow-control"),
                "description": "Compose-backed workflow run controls",
                "labels": label_map("workflow-run-controls"),
            }),
        )
        .await?;
    let workflow_id = json_id(&workflow, "id")?;

    harness
        .post_json(
            &format!("/api/v1/workflows/{workflow_id}/versions"),
            json!({
                "version": "v1",
                "executor": "manual",
                "entrypoint": "workflows/manual/run.mjs",
                "allowed_file_workspace_ids": [output_workspace_id],
            }),
        )
        .await?;

    let controlled_run = harness
        .post_json(
            "/api/v1/workflow-runs",
            json!({
                "workflow_id": workflow_id,
                "version": "v1",
                "client_request_id": harness.unique_name("compose-e2e-workflow-control-run"),
                "source_system": "bpane-gateway-compose-e2e",
                "source_reference": "workflow-run-controls",
                "session": {
                    "create_session": {
                        "labels": label_map("workflow-control-session"),
                    }
                },
                "labels": label_map("workflow-control-run"),
            }),
        )
        .await?;
    let controlled_run_id = json_id(&controlled_run, "id")?;
    let controlled_session_id = json_id(&controlled_run, "session_id")?;

    let automation_access = harness
        .post_json(
            &format!("/api/v1/sessions/{controlled_session_id}/automation-access"),
            json!({}),
        )
        .await?;
    let automation_token = json_id(&automation_access, "token")?;
    let automation_headers = automation_headers(&automation_token)?;

    let running = harness
        .post_json_with_headers(
            &format!("/api/v1/workflow-runs/{controlled_run_id}/state"),
            json!({
                "state": "running",
                "message": "compose e2e executor attached",
            }),
            automation_headers.clone(),
        )
        .await?;
    if running["state"] != json!("running") {
        return Err(anyhow!(
            "workflow run did not transition to running: {running}"
        ));
    }

    let appended_log = harness
        .post_json_with_headers(
            &format!("/api/v1/workflow-runs/{controlled_run_id}/logs"),
            json!({
                "stream": "stdout",
                "message": "compose e2e executor wrote a log line",
            }),
            automation_headers.clone(),
        )
        .await?;
    if appended_log["message"] != json!("compose e2e executor wrote a log line") {
        return Err(anyhow!("workflow run log append did not persist"));
    }

    let mut upload_headers = Vec::new();
    upload_headers.push((
        "x-bpane-workflow-workspace-id",
        output_workspace_id.as_str(),
    ));
    upload_headers.push(("x-bpane-file-name", "compose-e2e-produced.txt"));
    upload_headers.push((
        "x-bpane-file-provenance",
        "{\"origin\":\"bpane-gateway-compose-e2e\",\"kind\":\"produced_file\"}",
    ));
    upload_headers.push(("x-bpane-automation-access-token", automation_token.as_str()));
    let uploaded_file = harness
        .post_bytes(
            &format!("/api/v1/workflow-runs/{controlled_run_id}/produced-files"),
            b"compose workflow run control output".to_vec(),
            "text/plain; charset=utf-8",
            &upload_headers,
        )
        .await?;
    let uploaded_file_id = json_id(&uploaded_file, "file_id")?;

    let listed_files = harness
        .get_json_with_automation_token(
            &format!("/api/v1/workflow-runs/{controlled_run_id}/produced-files"),
            &automation_token,
        )
        .await?;
    let listed_files = json_array(&listed_files, "files")?;
    if !listed_files
        .iter()
        .any(|file| file.get("file_id") == Some(&json!(uploaded_file_id)))
    {
        return Err(anyhow!(
            "produced files list did not include uploaded file {uploaded_file_id}"
        ));
    }

    let produced_bytes = harness
        .get_bytes_with_automation_token(
            &format!(
                "/api/v1/workflow-runs/{controlled_run_id}/produced-files/{uploaded_file_id}/content"
            ),
            &automation_token,
        )
        .await?;
    if produced_bytes != b"compose workflow run control output" {
        return Err(anyhow!(
            "produced file content endpoint returned wrong bytes"
        ));
    }

    let first_request_id = Uuid::now_v7();
    let awaiting_input = harness
        .post_json_with_headers(
            &format!("/api/v1/workflow-runs/{controlled_run_id}/state"),
            json!({
                "state": "awaiting_input",
                "message": "operator approval required",
                "data": {
                    "intervention_request": {
                        "request_id": first_request_id,
                        "kind": "approval",
                        "prompt": "Approve compose workflow control run",
                    }
                }
            }),
            automation_headers.clone(),
        )
        .await?;
    if awaiting_input["state"] != json!("awaiting_input") {
        return Err(anyhow!(
            "workflow run did not enter awaiting_input: {awaiting_input}"
        ));
    }

    let submitted = harness
        .post_json(
            &format!("/api/v1/workflow-runs/{controlled_run_id}/submit-input"),
            json!({
                "input": {
                    "approved": true,
                    "reviewed_by": "compose-e2e",
                },
                "comment": "operator approved in compose e2e",
            }),
        )
        .await?;
    if submitted["state"] != json!("running")
        || submitted["intervention"]["last_resolution"]["action"] != json!("submit_input")
        || submitted["intervention"]["last_resolution"]["request_id"]
            != json!(first_request_id.to_string())
    {
        return Err(anyhow!(
            "submit-input did not persist operator resolution: {submitted}"
        ));
    }

    let second_request_id = Uuid::now_v7();
    harness
        .post_json_with_headers(
            &format!("/api/v1/workflow-runs/{controlled_run_id}/state"),
            json!({
                "state": "awaiting_input",
                "message": "resume required",
                "data": {
                    "intervention_request": {
                        "request_id": second_request_id,
                        "kind": "confirmation",
                        "prompt": "Resume the compose workflow control run",
                    }
                }
            }),
            automation_headers.clone(),
        )
        .await?;

    let resumed = harness
        .post_json(
            &format!("/api/v1/workflow-runs/{controlled_run_id}/resume"),
            json!({
                "comment": "operator resumed in compose e2e",
            }),
        )
        .await?;
    if resumed["state"] != json!("running")
        || resumed["intervention"]["last_resolution"]["action"] != json!("resume")
        || resumed["intervention"]["last_resolution"]["request_id"]
            != json!(second_request_id.to_string())
    {
        return Err(anyhow!(
            "resume did not persist operator resolution: {resumed}"
        ));
    }

    let third_request_id = Uuid::now_v7();
    harness
        .post_json_with_headers(
            &format!("/api/v1/workflow-runs/{controlled_run_id}/state"),
            json!({
                "state": "awaiting_input",
                "message": "reject required",
                "data": {
                    "intervention_request": {
                        "request_id": third_request_id,
                        "kind": "approval",
                        "prompt": "Reject the compose workflow control run",
                    }
                }
            }),
            automation_headers.clone(),
        )
        .await?;

    let rejected = harness
        .post_json(
            &format!("/api/v1/workflow-runs/{controlled_run_id}/reject"),
            json!({
                "reason": "compose operator rejected the run",
            }),
        )
        .await?;
    if rejected["state"] != json!("failed")
        || rejected["error"] != json!("compose operator rejected the run")
        || rejected["intervention"]["last_resolution"]["action"] != json!("reject")
        || rejected["intervention"]["last_resolution"]["request_id"]
            != json!(third_request_id.to_string())
    {
        return Err(anyhow!(
            "reject did not persist operator resolution: {rejected}"
        ));
    }

    let events = harness
        .get_json(&format!("/api/v1/workflow-runs/{controlled_run_id}/events"))
        .await?;
    let event_types = json_array(&events, "events")?
        .iter()
        .map(|event| event["event_type"].as_str().unwrap_or_default().to_string())
        .collect::<Vec<_>>();
    for expected in [
        "workflow_run.input_submitted",
        "workflow_run.resumed",
        "workflow_run.rejected",
    ] {
        if !event_types.iter().any(|event_type| event_type == expected) {
            return Err(anyhow!(
                "workflow run events missing expected type {expected}: {event_types:?}"
            ));
        }
    }

    let logs = harness
        .get_json_with_automation_token(
            &format!("/api/v1/workflow-runs/{controlled_run_id}/logs"),
            &automation_token,
        )
        .await?;
    let logs = json_array(&logs, "logs")?;
    if !logs.iter().any(|log| {
        log.get("message").and_then(|value| value.as_str())
            == Some("compose e2e executor wrote a log line")
    }) {
        return Err(anyhow!(
            "workflow run logs did not include automation-appended log line"
        ));
    }

    let cancellable_run = harness
        .post_json(
            "/api/v1/workflow-runs",
            json!({
                "workflow_id": workflow_id,
                "version": "v1",
                "client_request_id": harness.unique_name("compose-e2e-workflow-cancel-run"),
                "source_system": "bpane-gateway-compose-e2e",
                "source_reference": "workflow-run-cancel",
                "session": {
                    "create_session": {
                        "labels": label_map("workflow-control-cancel-session"),
                    }
                },
                "labels": label_map("workflow-control-cancel-run"),
            }),
        )
        .await?;
    let cancellable_run_id = json_id(&cancellable_run, "id")?;
    let cancellable_session_id = json_id(&cancellable_run, "session_id")?;
    let cancellable_access = harness
        .post_json(
            &format!("/api/v1/sessions/{cancellable_session_id}/automation-access"),
            json!({}),
        )
        .await?;
    let cancellable_token = json_id(&cancellable_access, "token")?;

    let cancelled = harness
        .post_json(
            &format!("/api/v1/workflow-runs/{cancellable_run_id}/cancel"),
            json!({}),
        )
        .await?;
    if cancelled["state"] != json!("cancelled") {
        return Err(anyhow!(
            "workflow run cancel endpoint did not cancel the run"
        ));
    }

    let cancel_logs = harness
        .get_json_with_automation_token(
            &format!("/api/v1/workflow-runs/{cancellable_run_id}/logs"),
            &cancellable_token,
        )
        .await?;
    if !json_array(&cancel_logs, "logs")?.iter().any(|log| {
        log.get("message").and_then(|value| value.as_str()) == Some("workflow run cancelled")
    }) {
        return Err(anyhow!(
            "cancelled workflow run logs did not include cancellation log entry"
        ));
    }

    let cancel_events = harness
        .get_json(&format!(
            "/api/v1/workflow-runs/{cancellable_run_id}/events"
        ))
        .await?;
    let cancel_event_types = json_array(&cancel_events, "events")?
        .iter()
        .map(|event| event["event_type"].as_str().unwrap_or_default().to_string())
        .collect::<Vec<_>>();
    for expected in [
        "workflow_run.cancel_requested",
        "workflow_run.cancelled",
        "automation_task.cancelled",
    ] {
        if !cancel_event_types
            .iter()
            .any(|event_type| event_type == expected)
        {
            return Err(anyhow!(
                "cancelled workflow run events missing expected type {expected}: {cancel_event_types:?}"
            ));
        }
    }

    Ok(())
}

fn automation_headers(automation_token: &str) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-bpane-automation-access-token",
        HeaderValue::from_str(automation_token)?,
    );
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    Ok(headers)
}
