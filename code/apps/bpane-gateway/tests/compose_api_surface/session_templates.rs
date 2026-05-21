use anyhow::{anyhow, Result};
use reqwest::StatusCode;
use serde_json::json;
use uuid::Uuid;

use super::support::{json_array, json_id, label_map, recording_policy, ComposeHarness};

pub async fn run(harness: &ComposeHarness) -> Result<()> {
    let template_name = harness.unique_name("compose-session-template");
    let template = harness
        .post_json(
            "/api/v1/session-templates",
            json!({
                "name": template_name,
                "description": "Compose e2e session template",
                "labels": label_map("session-templates"),
                "defaults": {
                    "owner_mode": "collaborative",
                    "idle_timeout_sec": 1800,
                    "labels": {
                        "team": "support",
                        "purpose": "debug"
                    },
                    "integration_context": {
                        "source": "compose-template"
                    },
                    "recording": recording_policy("manual")
                }
            }),
        )
        .await?;
    let template_id = json_id(&template, "id")?;
    if template["version"] != json!(1) {
        return Err(anyhow!("template create did not initialize version=1"));
    }

    let duplicate = harness
        .post_json_outcome(
            "/api/v1/session-templates",
            json!({
                "name": template_name,
                "defaults": {}
            }),
        )
        .await?;
    if duplicate.status != StatusCode::CONFLICT {
        return Err(anyhow!(
            "duplicate template create returned {} instead of 409: {}",
            duplicate.status,
            duplicate.body
        ));
    }

    let invalid_template = harness
        .post_json_outcome(
            "/api/v1/session-templates",
            json!({
                "name": "invalid-compose-template",
                "defaults": {
                    "idle_timeout_sec": 0
                }
            }),
        )
        .await?;
    if invalid_template.status != StatusCode::BAD_REQUEST {
        return Err(anyhow!(
            "invalid template create returned {} instead of 400: {}",
            invalid_template.status,
            invalid_template.body
        ));
    }

    let fetched_template = harness
        .get_json(&format!("/api/v1/session-templates/{template_id}"))
        .await?;
    if fetched_template["id"] != json!(template_id)
        || fetched_template["defaults"]["labels"]["team"] != json!("support")
    {
        return Err(anyhow!("template lookup returned unexpected data"));
    }

    let templates = harness.get_json("/api/v1/session-templates").await?;
    let templates = json_array(&templates, "templates")?;
    if !templates
        .iter()
        .any(|candidate| candidate.get("id") == Some(&json!(template_id)))
    {
        return Err(anyhow!("template list did not include {template_id}"));
    }

    let updated_template = harness
        .put_json(
            &format!("/api/v1/session-templates/{template_id}"),
            json!({
                "name": template_name,
                "description": "Compose e2e session template updated",
                "labels": label_map("session-templates"),
                "defaults": {
                    "idle_timeout_sec": 1200,
                    "labels": {
                        "team": "support",
                        "purpose": "debug",
                        "tier": "gold"
                    },
                    "integration_context": {
                        "source": "compose-template-updated"
                    },
                    "recording": recording_policy("manual")
                }
            }),
        )
        .await?;
    if updated_template["version"] != json!(2)
        || updated_template["defaults"]["labels"]["tier"] != json!("gold")
    {
        return Err(anyhow!(
            "template update did not increment version or persist defaults"
        ));
    }

    let missing_template = Uuid::now_v7();
    let missing_create = harness
        .post_json_outcome(
            "/api/v1/sessions",
            json!({
                "template_id": missing_template.to_string()
            }),
        )
        .await?;
    if missing_create.status != StatusCode::NOT_FOUND {
        return Err(anyhow!(
            "missing template session create returned {} instead of 404: {}",
            missing_create.status,
            missing_create.body
        ));
    }

    let run_id = harness.unique_name("session-template-e2e");
    let session = harness
        .post_json(
            "/api/v1/sessions",
            json!({
                "template_id": template_id,
                "labels": {
                    "run_id": run_id,
                    "purpose": "case-specific"
                },
                "integration_context": {
                    "ticket": run_id
                }
            }),
        )
        .await?;
    let session_id = json_id(&session, "id")?;
    if session["template_id"] != json!(template_id)
        || session["labels"]["team"] != json!("support")
        || session["labels"]["tier"] != json!("gold")
        || session["labels"]["purpose"] != json!("case-specific")
        || session["integration_context"]["source"] != json!("compose-template-updated")
        || session["integration_context"]["ticket"] != json!(run_id)
        || session["recording"]["mode"] != json!("manual")
    {
        return Err(anyhow!(
            "session create did not merge template defaults and caller overrides: {session}"
        ));
    }

    let filtered = harness
        .get_json(&format!(
            "/api/v1/sessions?template_id={template_id}&label.team=support&integration.ticket={run_id}&runtime_state=not_started&limit=1"
        ))
        .await?;
    let filtered_sessions = json_array(&filtered, "sessions")?;
    if filtered_sessions.len() != 1 || filtered_sessions[0]["id"] != json!(session_id) {
        return Err(anyhow!(
            "catalog filters did not return the templated session: {filtered}"
        ));
    }

    let invalid_query = harness
        .get_json_outcome("/api/v1/sessions?runtime_state=bogus")
        .await?;
    if invalid_query.status != StatusCode::BAD_REQUEST {
        return Err(anyhow!(
            "invalid catalog runtime_state returned {} instead of 400: {}",
            invalid_query.status,
            invalid_query.body
        ));
    }

    let deleted = harness
        .delete_json(&format!("/api/v1/sessions/{session_id}"))
        .await?;
    if deleted["state"] != json!("stopped") {
        return Err(anyhow!(
            "templated session cleanup did not stop the session"
        ));
    }

    Ok(())
}
