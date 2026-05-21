use anyhow::{anyhow, Context, Result};
use reqwest::StatusCode;
use serde_json::{json, Value};
use tokio::process::Command;
use uuid::Uuid;

use super::support::{json_array, json_id, label_map, ComposeHarness};

pub async fn run(harness: &ComposeHarness) -> Result<()> {
    let context_name = harness.unique_name("compose-browser-context");
    let context = harness
        .post_json(
            "/api/v1/browser-contexts",
            json!({
                "name": context_name,
                "description": "Compose e2e browser context",
                "labels": label_map("browser-contexts"),
                "retention_sec": 86400,
                "max_profile_storage_bytes": 10737418240_u64
            }),
        )
        .await?;
    let context_id = json_id(&context, "id")?;
    if context["persistence_mode"] != json!("reusable")
        || context["retention_sec"] != json!(86400)
        || context["retention_expires_at"].is_null()
        || context["max_profile_storage_bytes"] != json!(10737418240_u64)
        || context["state"] != json!("ready")
        || !context["last_used_at"].is_null()
    {
        return Err(anyhow!(
            "browser context create returned unexpected data: {context}"
        ));
    }

    let duplicate = harness
        .post_json_outcome(
            "/api/v1/browser-contexts",
            json!({
                "name": context_name
            }),
        )
        .await?;
    if duplicate.status != StatusCode::CONFLICT {
        return Err(anyhow!(
            "duplicate browser context create returned {} instead of 409: {}",
            duplicate.status,
            duplicate.body
        ));
    }

    let invalid = harness
        .post_json_outcome(
            "/api/v1/browser-contexts",
            json!({
                "name": "",
                "labels": {
                    "suite": "browser-contexts"
                }
            }),
        )
        .await?;
    if invalid.status != StatusCode::BAD_REQUEST {
        return Err(anyhow!(
            "invalid browser context create returned {} instead of 400: {}",
            invalid.status,
            invalid.body
        ));
    }

    let fetched = harness
        .get_json(&format!("/api/v1/browser-contexts/{context_id}"))
        .await?;
    if fetched["id"] != json!(context_id) || fetched["labels"]["scope"] != json!("browser-contexts")
    {
        return Err(anyhow!("browser context lookup returned unexpected data"));
    }

    let contexts = harness.get_json("/api/v1/browser-contexts").await?;
    let contexts = json_array(&contexts, "contexts")?;
    if !contexts
        .iter()
        .any(|candidate| candidate.get("id") == Some(&json!(context_id)))
    {
        return Err(anyhow!("browser context list did not include {context_id}"));
    }

    let missing_context = Uuid::now_v7();
    let missing_create = harness
        .post_json_outcome(
            "/api/v1/sessions",
            json!({
                "browser_context": {
                    "mode": "reusable",
                    "context_id": missing_context.to_string()
                }
            }),
        )
        .await?;
    if missing_create.status != StatusCode::NOT_FOUND {
        return Err(anyhow!(
            "missing browser context session create returned {} instead of 404: {}",
            missing_create.status,
            missing_create.body
        ));
    }

    let session = harness
        .post_json(
            "/api/v1/sessions",
            json!({
                "browser_context": {
                    "mode": "reusable",
                    "context_id": context_id
                },
                "labels": {
                    "suite": "browser-contexts"
                }
            }),
        )
        .await?;
    let session_id = json_id(&session, "id")?;
    if session["browser_context"]["mode"] != json!("reusable")
        || session["browser_context"]["context_id"] != json!(context_id)
    {
        return Err(anyhow!(
            "session create did not persist browser_context binding: {session}"
        ));
    }

    let used_context = harness
        .get_json(&format!("/api/v1/browser-contexts/{context_id}"))
        .await?;
    if used_context["last_used_at"].is_null() {
        return Err(anyhow!(
            "browser context last_used_at was not updated after session create"
        ));
    }

    let competing_session = harness
        .post_json(
            "/api/v1/sessions",
            json!({
                "browser_context": {
                    "mode": "reusable",
                    "context_id": context_id
                },
                "labels": {
                    "suite": "browser-contexts-competing"
                }
            }),
        )
        .await?;
    let competing_session_id = json_id(&competing_session, "id")?;
    let profile_key = format!("bpane_context_{context_id}");
    let profile_value = format!("profile-state-{session_id}");
    let cookie_name = "bpane_context_probe";
    let cookie_value = format!("cookie-state-{session_id}");
    let first_runtime_access = harness
        .post_json(
            &format!("/api/v1/sessions/{session_id}/automation-access"),
            json!({}),
        )
        .await?;
    let first_cdp_endpoint = automation_cdp_endpoint(&first_runtime_access)?;
    let write_probe = run_profile_state_probe(
        harness,
        first_cdp_endpoint,
        "set",
        &profile_key,
        &profile_value,
        cookie_name,
        &cookie_value,
    )
    .await?;
    if write_probe["localStorageValue"] != json!(profile_value) {
        return Err(anyhow!(
            "reusable browser context CDP write probe returned unexpected state: {write_probe}"
        ));
    }

    let competing_runtime_access = harness
        .post_json_outcome(
            &format!("/api/v1/sessions/{competing_session_id}/automation-access"),
            json!({}),
        )
        .await?;
    if competing_runtime_access.status != StatusCode::CONFLICT {
        return Err(anyhow!(
            "parallel reusable browser context access returned {} instead of 409: {}",
            competing_runtime_access.status,
            competing_runtime_access.body
        ));
    }
    let competing_error = competing_runtime_access.body["error"]
        .as_str()
        .unwrap_or_default();
    if !competing_error.contains("browser context")
        || !competing_error.contains("already used by active session")
    {
        return Err(anyhow!(
            "parallel reusable browser context access returned unexpected error: {}",
            competing_runtime_access.body
        ));
    }

    let deleted_session = harness
        .delete_json(&format!("/api/v1/sessions/{session_id}"))
        .await?;
    if deleted_session["state"] != json!("stopped") {
        return Err(anyhow!("browser-context session cleanup did not stop"));
    }
    let competing_deleted_session = harness
        .delete_json(&format!("/api/v1/sessions/{competing_session_id}"))
        .await?;
    if competing_deleted_session["state"] != json!("stopped") {
        return Err(anyhow!(
            "browser-context competing session cleanup did not stop"
        ));
    }

    let restored_session = harness
        .post_json(
            "/api/v1/sessions",
            json!({
                "browser_context": {
                    "mode": "reusable",
                    "context_id": context_id
                },
                "labels": {
                    "suite": "browser-contexts-restored"
                }
            }),
        )
        .await?;
    let restored_session_id = json_id(&restored_session, "id")?;
    let restored_runtime_access = harness
        .post_json(
            &format!("/api/v1/sessions/{restored_session_id}/automation-access"),
            json!({}),
        )
        .await?;
    let restored_cdp_endpoint = automation_cdp_endpoint(&restored_runtime_access)?;
    let read_probe = run_profile_state_probe(
        harness,
        restored_cdp_endpoint,
        "get",
        &profile_key,
        &profile_value,
        cookie_name,
        &cookie_value,
    )
    .await?;
    if read_probe["localStorageValue"] != json!(profile_value) {
        return Err(anyhow!(
            "reusable browser context did not restore profile-backed state: {read_probe}"
        ));
    }
    let storage_context = harness
        .get_json(&format!("/api/v1/browser-contexts/{context_id}"))
        .await?;
    let storage_bytes = storage_context["usage"]["profile_storage_bytes"]
        .as_u64()
        .ok_or_else(|| {
            anyhow!(
                "browser context did not report docker profile storage bytes: {storage_context}"
            )
        })?;
    if storage_bytes == 0 {
        return Err(anyhow!(
            "browser context profile storage bytes did not increase after browser use: {storage_context}"
        ));
    }
    if storage_context["usage"]["profile_storage_limit_exceeded"] != json!(false) {
        return Err(anyhow!(
            "browser context profile storage limit should not be exceeded: {storage_context}"
        ));
    }
    let restored_deleted_session = harness
        .delete_json(&format!("/api/v1/sessions/{restored_session_id}"))
        .await?;
    if restored_deleted_session["state"] != json!("stopped") {
        return Err(anyhow!(
            "browser-context restored session cleanup did not stop"
        ));
    }

    let cloned_context = harness
        .post_json(
            &format!("/api/v1/browser-contexts/{context_id}/clone"),
            json!({
                "name": harness.unique_name("compose-browser-context-clone"),
                "description": "Compose e2e browser context clone",
                "labels": label_map("browser-context-clone"),
                "retention_sec": 43200,
                "max_profile_storage_bytes": 10737418240_u64
            }),
        )
        .await?;
    let cloned_context_id = json_id(&cloned_context, "id")?;
    if cloned_context_id == context_id
        || cloned_context["persistence_mode"] != json!("reusable")
        || cloned_context["retention_sec"] != json!(43200)
        || cloned_context["max_profile_storage_bytes"] != json!(10737418240_u64)
    {
        return Err(anyhow!(
            "browser context clone returned unexpected data: {cloned_context}"
        ));
    }

    let cloned_session = harness
        .post_json(
            "/api/v1/sessions",
            json!({
                "browser_context": {
                    "mode": "reusable",
                    "context_id": cloned_context_id
                },
                "labels": {
                    "suite": "browser-contexts-clone"
                }
            }),
        )
        .await?;
    let cloned_session_id = json_id(&cloned_session, "id")?;
    let cloned_runtime_access = harness
        .post_json(
            &format!("/api/v1/sessions/{cloned_session_id}/automation-access"),
            json!({}),
        )
        .await?;
    let cloned_cdp_endpoint = automation_cdp_endpoint(&cloned_runtime_access)?;
    let clone_probe = run_profile_state_probe(
        harness,
        cloned_cdp_endpoint,
        "get",
        &profile_key,
        &profile_value,
        cookie_name,
        &cookie_value,
    )
    .await?;
    if clone_probe["localStorageValue"] != json!(profile_value) {
        return Err(anyhow!(
            "cloned browser context did not copy profile-backed state: {clone_probe}"
        ));
    }
    let cloned_deleted_session = harness
        .delete_json(&format!("/api/v1/sessions/{cloned_session_id}"))
        .await?;
    if cloned_deleted_session["state"] != json!("stopped") {
        return Err(anyhow!(
            "browser-context clone session cleanup did not stop"
        ));
    }
    let deleted_cloned_context = harness
        .delete_json(&format!("/api/v1/browser-contexts/{cloned_context_id}"))
        .await?;
    if deleted_cloned_context["state"] != json!("deleted") {
        return Err(anyhow!(
            "browser-context clone cleanup did not delete context: {deleted_cloned_context}"
        ));
    }

    let deleted_context = harness
        .delete_json(&format!("/api/v1/browser-contexts/{context_id}"))
        .await?;
    if deleted_context["state"] != json!("deleted") || deleted_context["deleted_at"].is_null() {
        return Err(anyhow!(
            "browser context delete did not mark the context deleted: {deleted_context}"
        ));
    }

    let deleted_context_create = harness
        .post_json_outcome(
            "/api/v1/sessions",
            json!({
                "browser_context": {
                    "mode": "reusable",
                    "context_id": context_id
                }
            }),
        )
        .await?;
    if deleted_context_create.status != StatusCode::CONFLICT {
        return Err(anyhow!(
            "deleted browser context session create returned {} instead of 409: {}",
            deleted_context_create.status,
            deleted_context_create.body
        ));
    }

    verify_browser_context_storage_quota(harness).await?;

    Ok(())
}

async fn verify_browser_context_storage_quota(harness: &ComposeHarness) -> Result<()> {
    let context_name = harness.unique_name("compose-browser-context-quota");
    let context = harness
        .post_json(
            "/api/v1/browser-contexts",
            json!({
                "name": context_name,
                "description": "Compose e2e browser context with a tiny storage limit",
                "labels": label_map("browser-context-quota"),
                "max_profile_storage_bytes": 1_u64
            }),
        )
        .await?;
    let context_id = json_id(&context, "id")?;
    if context["max_profile_storage_bytes"] != json!(1_u64) {
        return Err(anyhow!(
            "browser context quota create returned unexpected data: {context}"
        ));
    }

    let session = harness
        .post_json(
            "/api/v1/sessions",
            json!({
                "browser_context": {
                    "mode": "reusable",
                    "context_id": context_id
                },
                "labels": {
                    "suite": "browser-context-quota"
                }
            }),
        )
        .await?;
    let session_id = json_id(&session, "id")?;
    let runtime_access = harness
        .post_json(
            &format!("/api/v1/sessions/{session_id}/automation-access"),
            json!({}),
        )
        .await?;
    let cdp_endpoint = automation_cdp_endpoint(&runtime_access)?;
    let probe_key = format!("bpane_quota_{context_id}");
    let probe_value = format!("quota-state-{session_id}");
    let cookie_name = "bpane_quota_probe";
    let cookie_value = format!("quota-cookie-{session_id}");
    run_profile_state_probe(
        harness,
        cdp_endpoint,
        "set",
        &probe_key,
        &probe_value,
        cookie_name,
        &cookie_value,
    )
    .await?;

    let stopped = harness
        .delete_json(&format!("/api/v1/sessions/{session_id}"))
        .await?;
    if stopped["state"] != json!("stopped") {
        return Err(anyhow!(
            "browser-context quota session cleanup did not stop"
        ));
    }

    let over_limit_context = harness
        .get_json(&format!("/api/v1/browser-contexts/{context_id}"))
        .await?;
    if over_limit_context["usage"]["profile_storage_limit_exceeded"] != json!(true) {
        return Err(anyhow!(
            "browser context quota did not report an exceeded profile limit: {over_limit_context}"
        ));
    }

    let rejected = harness
        .post_json_outcome(
            "/api/v1/sessions",
            json!({
                "browser_context": {
                    "mode": "reusable",
                    "context_id": context_id
                },
                "labels": {
                    "suite": "browser-context-quota-rejected"
                }
            }),
        )
        .await?;
    if rejected.status != StatusCode::CONFLICT {
        return Err(anyhow!(
            "over-limit browser context session create returned {} instead of 409: {}",
            rejected.status,
            rejected.body
        ));
    }
    let rejected_error = rejected.body["error"].as_str().unwrap_or_default();
    if !rejected_error.contains("profile storage")
        || !rejected_error.contains("exceeds configured limit")
    {
        return Err(anyhow!(
            "over-limit browser context returned unexpected error: {}",
            rejected.body
        ));
    }

    let deleted = harness
        .delete_json(&format!("/api/v1/browser-contexts/{context_id}"))
        .await?;
    if deleted["state"] != json!("deleted") {
        return Err(anyhow!(
            "browser-context quota cleanup did not delete context: {deleted}"
        ));
    }
    Ok(())
}

fn automation_cdp_endpoint(access: &Value) -> Result<&str> {
    access["automation"]["endpoint_url"]
        .as_str()
        .ok_or_else(|| anyhow!("automation access response did not include endpoint_url: {access}"))
}

async fn run_profile_state_probe(
    harness: &ComposeHarness,
    cdp_endpoint: &str,
    action: &str,
    key: &str,
    value: &str,
    cookie_name: &str,
    cookie_value: &str,
) -> Result<Value> {
    let docker_network =
        std::env::var("BPANE_DOCKER_NETWORK").unwrap_or_else(|_| "deploy_bpane-internal".into());
    let repo_mount = format!("{}:/workspace:ro", harness.repo_root().display());
    let output = Command::new("docker")
        .args([
            "run",
            "--rm",
            "--network",
            docker_network.as_str(),
            "-v",
            repo_mount.as_str(),
            "-w",
            "/workspace/code/web/bpane-client",
            "node:22-slim",
            "node",
            "scripts/cdp-profile-state-probe.mjs",
            "--cdp-endpoint",
            cdp_endpoint,
            "--action",
            action,
            "--key",
            key,
            "--value",
            value,
            "--cookie-name",
            cookie_name,
            "--cookie-value",
            cookie_value,
        ])
        .output()
        .await
        .context("failed to run CDP profile state probe container")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        return Err(anyhow!(
            "CDP profile state probe failed with status {} stdout={} stderr={}",
            output.status,
            stdout.trim(),
            stderr.trim()
        ));
    }
    serde_json::from_str(stdout.trim()).with_context(|| {
        format!(
            "failed to parse CDP profile state probe output as JSON: {}",
            stdout.trim()
        )
    })
}
