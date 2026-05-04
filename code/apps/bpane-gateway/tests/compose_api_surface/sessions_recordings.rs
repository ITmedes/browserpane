use anyhow::{anyhow, Result};
use serde_json::json;

use super::support::{json_array, json_id, label_map, recording_policy, ComposeHarness};

pub async fn run(harness: &ComposeHarness) -> Result<()> {
    let created = harness
        .post_json(
            "/api/v1/sessions",
            json!({
                "labels": label_map("sessions-recordings"),
                "integration_context": {
                    "suite": "bpane-gateway-compose-e2e",
                    "case": "sessions-recordings",
                },
                "recording": recording_policy("manual"),
            }),
        )
        .await?;
    let session_id = json_id(&created, "id")?;

    let sessions = harness.get_json("/api/v1/sessions").await?;
    let sessions = json_array(&sessions, "sessions")?;
    if !sessions
        .iter()
        .any(|session| session.get("id") == Some(&json!(session_id)))
    {
        return Err(anyhow!(
            "created session {session_id} is missing from session list"
        ));
    }

    let fetched = harness
        .get_json(&format!("/api/v1/sessions/{session_id}"))
        .await?;
    if fetched["recording"]["mode"] != json!("manual") {
        return Err(anyhow!("session recording policy was not persisted"));
    }

    let status = harness
        .get_json(&format!("/api/v1/sessions/{session_id}/status"))
        .await?;
    if !status["recording"]["configured_mode"].is_string() {
        return Err(anyhow!("session status is missing recording state"));
    }

    let connect_ticket = harness
        .post_json(
            &format!("/api/v1/sessions/{session_id}/access-tokens"),
            json!({}),
        )
        .await?;
    if connect_ticket["token"]
        .as_str()
        .unwrap_or_default()
        .is_empty()
    {
        return Err(anyhow!("session connect ticket was not issued"));
    }

    let automation_owner = harness
        .post_json(
            &format!("/api/v1/sessions/{session_id}/automation-owner"),
            json!({
                "client_id": "bpane-mcp-bridge",
                "issuer": "http://localhost:8091/realms/browserpane-dev",
                "display_name": "BrowserPane MCP bridge",
            }),
        )
        .await?;
    if automation_owner["automation_delegate"]["client_id"] != json!("bpane-mcp-bridge") {
        return Err(anyhow!("automation owner assignment was not persisted"));
    }

    let cleared = harness
        .delete_json(&format!("/api/v1/sessions/{session_id}/automation-owner"))
        .await?;
    if !cleared["automation_delegate"].is_null() {
        return Err(anyhow!("automation owner was not cleared"));
    }

    let automation_access = harness
        .post_json(
            &format!("/api/v1/sessions/{session_id}/automation-access"),
            json!({}),
        )
        .await?;
    if automation_access["token"]
        .as_str()
        .unwrap_or_default()
        .is_empty()
    {
        return Err(anyhow!("session automation access token was not issued"));
    }

    let recording = harness
        .post_json(
            &format!("/api/v1/sessions/{session_id}/recordings"),
            json!({}),
        )
        .await?;
    let recording_id = json_id(&recording, "id")?;

    let recordings = harness
        .get_json(&format!("/api/v1/sessions/{session_id}/recordings"))
        .await?;
    let recordings = json_array(&recordings, "recordings")?;
    if recordings.is_empty() {
        return Err(anyhow!(
            "session recording list is empty after recording creation"
        ));
    }

    let fetched_recording = harness
        .get_json(&format!(
            "/api/v1/sessions/{session_id}/recordings/{recording_id}"
        ))
        .await?;
    if fetched_recording["id"] != json!(recording_id) {
        return Err(anyhow!("recording lookup returned the wrong resource"));
    }

    let stopped_recording = harness
        .post_json(
            &format!("/api/v1/sessions/{session_id}/recordings/{recording_id}/stop"),
            json!({}),
        )
        .await?;
    if stopped_recording["state"] != json!("finalizing") {
        return Err(anyhow!(
            "recording stop did not transition to finalizing: {stopped_recording}"
        ));
    }

    let failed_recording = harness
        .post_json(
            &format!("/api/v1/sessions/{session_id}/recordings/{recording_id}/fail"),
            json!({
                "error": "compose e2e synthetic recorder finalization",
                "termination_reason": "worker_exit",
            }),
        )
        .await?;
    if failed_recording["state"] != json!("failed") {
        return Err(anyhow!(
            "recording fail did not transition to failed: {failed_recording}"
        ));
    }

    let operations = harness.get_json("/api/v1/recording/operations").await?;
    if !operations.is_object() {
        return Err(anyhow!(
            "recording operations endpoint did not return an object"
        ));
    }

    let deleted = harness
        .delete_json(&format!("/api/v1/sessions/{session_id}"))
        .await?;
    if deleted["state"] != json!("stopped") {
        return Err(anyhow!("session delete did not stop the session resource"));
    }

    let refreshed_sessions = harness.get_json("/api/v1/sessions").await?;
    let refreshed_sessions = json_array(&refreshed_sessions, "sessions")?;
    let stopped_session = refreshed_sessions
        .iter()
        .find(|session| session.get("id") == Some(&json!(session_id)))
        .ok_or_else(|| anyhow!("stopped session {session_id} disappeared from session list"))?;
    if stopped_session["state"] != json!("stopped") {
        return Err(anyhow!(
            "stopped session {session_id} did not remain visible with stopped state"
        ));
    }

    Ok(())
}
