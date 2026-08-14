use std::process::Command;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use reqwest::header::HeaderMap;
use reqwest::StatusCode;

use super::support::{openmetrics_gauge, poll_until, ComposeHarness};

pub async fn run(harness: &ComposeHarness) -> Result<()> {
    assert_probe_status(harness, "/healthz", StatusCode::OK, "live").await?;
    assert_probe_status(harness, "/readyz", StatusCode::OK, "ready").await?;

    let sensitive_unknown_path = format!("/missing/{}", uuid::Uuid::now_v7());
    let unknown = harness
        .get_text_outcome_without_bearer(&sensitive_unknown_path)
        .await?;
    if unknown.status != StatusCode::NOT_FOUND {
        bail!(
            "unexpected unknown-route response status: {}",
            unknown.status
        );
    }

    let _restore = harness.compose_service_restore_guard(&["postgres"]);
    run_compose_command(harness, &["stop", "postgres"])?;
    poll_until(
        "gateway readiness failure after postgres stop",
        Duration::from_secs(30),
        || async {
            let outcome = harness
                .get_json_outcome_without_bearer("/readyz", HeaderMap::new())
                .await?;
            if outcome.status == StatusCode::SERVICE_UNAVAILABLE {
                return Ok(Some(outcome.body));
            }
            Ok(None)
        },
    )
    .await?;
    assert_probe_status(harness, "/healthz", StatusCode::OK, "live").await?;

    run_compose_command(harness, &["start", "postgres"])?;
    poll_until(
        "gateway readiness recovery after postgres start",
        Duration::from_secs(30),
        || async {
            let outcome = harness
                .get_json_outcome_without_bearer("/readyz", HeaderMap::new())
                .await?;
            if outcome.status == StatusCode::OK {
                return Ok(Some(outcome.body));
            }
            Ok(None)
        },
    )
    .await?;
    assert_metrics_surface(harness, &sensitive_unknown_path).await?;
    Ok(())
}

async fn assert_metrics_surface(harness: &ComposeHarness, sensitive_value: &str) -> Result<()> {
    let outcome = harness.get_text_outcome_without_bearer("/metrics").await?;
    if outcome.status != StatusCode::OK {
        bail!("unexpected /metrics response status: {}", outcome.status);
    }
    if outcome.content_type.as_deref()
        != Some("application/openmetrics-text; version=1.0.0; charset=utf-8")
    {
        bail!(
            "unexpected /metrics content type: {:?}",
            outcome.content_type
        );
    }
    for expected in [
        "browserpane_gateway_http_requests_total",
        "browserpane_gateway_http_request_duration_seconds",
        "route=\"/healthz\",status_class=\"2xx\"",
        "route=\"/readyz\",status_class=\"2xx\"",
        "route=\"/readyz\",status_class=\"5xx\"",
        "route=\"unmatched\",status_class=\"4xx\"",
        "browserpane_gateway_runtime_active_assignments",
        "browserpane_gateway_runtime_starting_assignments",
        "browserpane_gateway_runtime_assignment_limit",
        "# EOF\n",
    ] {
        if !outcome.body.contains(expected) {
            bail!("/metrics response is missing {expected:?}");
        }
    }
    for metric_name in subsystem_counter_names() {
        for expected in [
            format!("# HELP {metric_name} "),
            format!("# TYPE {metric_name} counter"),
            format!("{metric_name}_total "),
        ] {
            if !outcome.body.contains(&expected) {
                bail!("/metrics response is missing {expected:?}");
            }
        }
        if outcome.body.contains(&format!("{metric_name}_total{{")) {
            bail!("subsystem metric {metric_name} unexpectedly contains labels");
        }
        openmetrics_gauge(&outcome.body, &format!("{metric_name}_total"))?;
    }
    if outcome.body.contains(sensitive_value)
        || outcome.body.contains(harness.bearer_token())
        || outcome.body.contains("route=\"/metrics\"")
    {
        bail!("/metrics response contains a sensitive or self-referential label");
    }
    if openmetrics_gauge(
        &outcome.body,
        "browserpane_gateway_runtime_assignment_limit",
    )? <= 0
    {
        bail!("runtime assignment limit must be positive");
    }
    Ok(())
}

fn subsystem_counter_names() -> [&'static str; 25] {
    [
        "browserpane_gateway_workflow_produced_file_uploads",
        "browserpane_gateway_workflow_produced_file_upload_failures",
        "browserpane_gateway_workflow_event_delivery_attempts",
        "browserpane_gateway_workflow_event_delivery_successes",
        "browserpane_gateway_workflow_event_delivery_retries",
        "browserpane_gateway_workflow_event_delivery_failures",
        "browserpane_gateway_workflow_retention_passes",
        "browserpane_gateway_workflow_retention_log_candidates",
        "browserpane_gateway_workflow_retention_output_candidates",
        "browserpane_gateway_workflow_retention_deleted_logs",
        "browserpane_gateway_workflow_retention_cleared_outputs",
        "browserpane_gateway_workflow_retention_failures",
        "browserpane_gateway_recording_artifact_finalize_requests",
        "browserpane_gateway_recording_artifact_finalize_successes",
        "browserpane_gateway_recording_artifact_finalize_failures",
        "browserpane_gateway_recording_failures",
        "browserpane_gateway_recording_playback_manifest_requests",
        "browserpane_gateway_recording_playback_export_requests",
        "browserpane_gateway_recording_playback_export_successes",
        "browserpane_gateway_recording_playback_export_failures",
        "browserpane_gateway_recording_playback_export_bytes",
        "browserpane_gateway_recording_retention_passes",
        "browserpane_gateway_recording_retention_candidates",
        "browserpane_gateway_recording_retention_deleted_artifacts",
        "browserpane_gateway_recording_retention_failures",
    ]
}

async fn assert_probe_status(
    harness: &ComposeHarness,
    path: &str,
    expected_status: StatusCode,
    expected_body_status: &str,
) -> Result<()> {
    let outcome = harness
        .get_json_outcome_without_bearer(path, HeaderMap::new())
        .await?;
    if outcome.status != expected_status
        || outcome
            .body
            .get("status")
            .and_then(serde_json::Value::as_str)
            != Some(expected_body_status)
    {
        bail!(
            "unexpected {path} response: status={} body={}",
            outcome.status,
            outcome.body
        );
    }
    Ok(())
}

fn run_compose_command(harness: &ComposeHarness, arguments: &[&str]) -> Result<()> {
    let status = Command::new("docker")
        .args(["compose", "-f", "deploy/compose.yml"])
        .args(arguments)
        .current_dir(harness.repo_root())
        .status()
        .context("failed to execute docker compose dependency command")?;
    if !status.success() {
        bail!("docker compose dependency command failed: {arguments:?}");
    }
    Ok(())
}
