use std::process::Command;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use reqwest::header::HeaderMap;
use reqwest::StatusCode;

use super::support::{poll_until, ComposeHarness};

pub async fn run(harness: &ComposeHarness) -> Result<()> {
    assert_probe_status(harness, "/healthz", StatusCode::OK, "live").await?;
    assert_probe_status(harness, "/readyz", StatusCode::OK, "ready").await?;

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
    Ok(())
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
