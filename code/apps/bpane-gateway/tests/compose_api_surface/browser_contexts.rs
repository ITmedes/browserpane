use anyhow::{anyhow, Result};
use reqwest::StatusCode;
use serde_json::json;
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
                "labels": label_map("browser-contexts")
            }),
        )
        .await?;
    let context_id = json_id(&context, "id")?;
    if context["persistence_mode"] != json!("reusable")
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

    let deleted_session = harness
        .delete_json(&format!("/api/v1/sessions/{session_id}"))
        .await?;
    if deleted_session["state"] != json!("stopped") {
        return Err(anyhow!("browser-context session cleanup did not stop"));
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

    Ok(())
}
