use std::time::Duration;

use anyhow::{anyhow, Result};
use reqwest::StatusCode;
use serde_json::{json, Value};

use crate::support::{json_id, label_map, poll_until, ComposeHarness};

const DOCKER_POOL_BACKEND: &str = "docker_pool";

pub async fn run_session_capacity(harness: &ComposeHarness) -> Result<()> {
    let _restore = harness.compose_service_restore_guard(&["gateway"]);
    configure_gateway(
        harness,
        &[
            ("BPANE_GATEWAY_MAX_ACTIVE_RUNTIMES", "2"),
            ("BPANE_WORKFLOW_WORKER_MAX_ACTIVE", "0"),
        ],
    )
    .await?;
    harness.cleanup_active_sessions().await?;

    let legacy_status = harness.get_json_outcome("/api/session/status").await?;
    if legacy_status.status != StatusCode::CONFLICT {
        return Err(anyhow!(
            "docker_pool legacy status route returned unexpected status {} {}",
            legacy_status.status,
            legacy_status.body
        ));
    }

    let first = create_session(harness, "docker-pool-capacity-first").await?;
    let first_id = json_id(&first, "id")?;
    let second = create_session(harness, "docker-pool-capacity-second").await?;
    let second_id = json_id(&second, "id")?;

    for session_id in [&first_id, &second_id] {
        let status = harness
            .get_json(&format!("/api/v1/sessions/{session_id}/status"))
            .await?;
        if !status["recording"].is_object() {
            return Err(anyhow!(
                "docker_pool session status did not remain readable for {session_id}: {status}"
            ));
        }
    }

    let third_attempt = harness
        .post_json_outcome(
            "/api/v1/sessions",
            json!({
                "labels": label_map("docker-pool-capacity-third"),
                "integration_context": {
                    "suite": "bpane-gateway-compose-e2e",
                    "case": "docker-pool-capacity-third",
                }
            }),
        )
        .await?;
    if third_attempt.status != StatusCode::CONFLICT {
        return Err(anyhow!(
            "docker_pool capacity conflict returned unexpected status {} {}",
            third_attempt.status,
            third_attempt.body
        ));
    }
    let error_text = third_attempt.body["error"].as_str().unwrap_or_default();
    if !error_text.contains("2 active runtime-backed sessions") {
        return Err(anyhow!(
            "docker_pool capacity conflict returned unexpected error payload: {}",
            third_attempt.body
        ));
    }

    let stopped = harness.stop_session_eventually(&first_id).await?;
    if stopped["state"] != json!("stopped") {
        return Err(anyhow!(
            "docker_pool session stop did not yield stopped state: {stopped}"
        ));
    }

    let replacement = create_session(harness, "docker-pool-capacity-replacement").await?;
    let replacement_id = json_id(&replacement, "id")?;
    let replacement_status = harness
        .get_json(&format!("/api/v1/sessions/{replacement_id}/status"))
        .await?;
    if !replacement_status["recording"].is_object() {
        return Err(anyhow!(
            "replacement docker_pool session status is malformed: {replacement_status}"
        ));
    }

    let _ = harness.stop_session_eventually(&second_id).await?;
    let _ = harness.stop_session_eventually(&replacement_id).await?;
    Ok(())
}

pub async fn run_workflow_admission(harness: &ComposeHarness) -> Result<()> {
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

    let workflow_repo = harness
        .create_custom_workflow_repo(&[(
            "workflows/pool/run.mjs",
            r#"export default async function run({ page, input, sessionId }) {
  const holdMs =
    input && Number.isFinite(input.hold_ms)
      ? Number(input.hold_ms)
      : 0;
  await page.goto('http://web:8080', { waitUntil: 'networkidle' });
  if (holdMs > 0) {
    await new Promise((resolve) => setTimeout(resolve, holdMs));
  }
  return {
    title: await page.title(),
    final_url: page.url(),
    hold_ms: holdMs,
    session_id: sessionId,
  };
}
"#,
        )])
        .await?;

    let workflow = harness
        .post_json(
            "/api/v1/workflows",
            json!({
                "name": "compose-docker-pool-admission",
                "description": "Exercise queued workflow admission in docker_pool mode",
                "labels": label_map("docker-pool-workflow-admission"),
            }),
        )
        .await?;
    let workflow_id = json_id(&workflow, "id")?;

    let version_response = harness
        .post_json(
            &format!("/api/v1/workflows/{workflow_id}/versions"),
            json!({
                "version": "v1",
                "executor": "playwright",
                "entrypoint": "workflows/pool/run.mjs",
                "source": {
                    "kind": "git",
                    "repository_url": workflow_repo.repository_url,
                    "ref": "refs/heads/main",
                    "root_path": "workflows",
                },
                "default_session": {
                    "labels": {
                        "origin": "bpane-gateway-compose-e2e",
                        "scope": "docker-pool-workflow-admission",
                    }
                }
            }),
        )
        .await?;
    if version_response["source"]["resolved_commit"] != json!(workflow_repo.commit) {
        return Err(anyhow!(
            "docker_pool workflow version did not pin the expected commit: {version_response}"
        ));
    }

    let request_prefix = format!("docker-pool-admission-{}", uuid::Uuid::now_v7());
    let first_run = create_workflow_run(
        harness,
        &workflow_id,
        json!({ "hold_ms": 1800 }),
        &format!("{request_prefix}-run-1"),
    )
    .await?;
    let first_run_id = json_id(&first_run, "id")?;

    poll_until(
        "docker_pool first workflow run start",
        Duration::from_secs(20),
        || {
            let harness = harness.clone();
            let first_run_id = first_run_id.clone();
            async move {
                let run = harness
                    .get_json(&format!("/api/v1/workflow-runs/{first_run_id}"))
                    .await?;
                let state = run["state"].as_str().unwrap_or_default();
                if matches!(state, "running" | "starting") {
                    return Ok(Some(run));
                }
                Ok(None)
            }
        },
    )
    .await?;

    let second_run = create_workflow_run(
        harness,
        &workflow_id,
        json!({ "hold_ms": 0 }),
        &format!("{request_prefix}-run-2"),
    )
    .await?;
    let second_run_id = json_id(&second_run, "id")?;
    if second_run["state"] != json!("queued") {
        return Err(anyhow!(
            "docker_pool queued run was not created in queued state: {second_run}"
        ));
    }
    if second_run["admission"]["reason"] != json!("workflow_worker_capacity") {
        return Err(anyhow!(
            "docker_pool queued run did not expose workflow_worker_capacity admission: {second_run}"
        ));
    }

    let completed_second = poll_until(
        "docker_pool queued workflow completion",
        Duration::from_secs(40),
        || {
            let harness = harness.clone();
            let second_run_id = second_run_id.clone();
            async move {
                let run = harness
                    .get_json(&format!("/api/v1/workflow-runs/{second_run_id}"))
                    .await?;
                let state = run["state"].as_str().unwrap_or_default();
                if matches!(state, "succeeded" | "failed" | "cancelled" | "timed_out") {
                    return Ok(Some(run));
                }
                Ok(None)
            }
        },
    )
    .await?;
    if completed_second["state"] != json!("succeeded") {
        return Err(anyhow!(
            "docker_pool queued run did not eventually succeed: {completed_second}"
        ));
    }
    if !completed_second["admission"].is_null() {
        return Err(anyhow!(
            "docker_pool queued run admission block did not clear after execution: {completed_second}"
        ));
    }

    let second_events = harness
        .get_json(&format!("/api/v1/workflow-runs/{second_run_id}/events"))
        .await?;
    let event_types = second_events["events"]
        .as_array()
        .ok_or_else(|| anyhow!("workflow run events payload is malformed: {second_events}"))?
        .iter()
        .filter_map(|event| event.get("event_type").and_then(Value::as_str))
        .collect::<Vec<_>>();
    for expected in ["workflow_run.queued", "workflow_run.succeeded"] {
        if !event_types.contains(&expected) {
            return Err(anyhow!(
                "docker_pool queued run is missing expected event {expected}: {second_events}"
            ));
        }
    }

    Ok(())
}

async fn configure_gateway(
    harness: &ComposeHarness,
    extra_env_overrides: &[(&str, &str)],
) -> Result<()> {
    let mut env_overrides = vec![
        ("BPANE_GATEWAY_RUNTIME_BACKEND", DOCKER_POOL_BACKEND),
        ("BPANE_GATEWAY_MAX_STARTING_RUNTIMES", "2"),
    ];
    env_overrides.extend_from_slice(extra_env_overrides);
    harness.recreate_compose_services(&["gateway"], &env_overrides)?;
    harness.wait_for_gateway_api_ready().await?;
    Ok(())
}

async fn create_session(harness: &ComposeHarness, scope: &str) -> Result<Value> {
    harness
        .post_json(
            "/api/v1/sessions",
            json!({
                "labels": label_map(scope),
                "integration_context": {
                    "suite": "bpane-gateway-compose-e2e",
                    "case": scope,
                }
            }),
        )
        .await
}

async fn create_workflow_run(
    harness: &ComposeHarness,
    workflow_id: &str,
    input: Value,
    client_request_id: &str,
) -> Result<Value> {
    harness
        .post_json(
            "/api/v1/workflow-runs",
            json!({
                "workflow_id": workflow_id,
                "version": "v1",
                "session": {
                    "create_session": {}
                },
                "client_request_id": client_request_id,
                "input": input,
                "labels": label_map("docker-pool-workflow-admission"),
            }),
        )
        .await
}
