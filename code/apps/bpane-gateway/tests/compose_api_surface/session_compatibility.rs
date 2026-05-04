use std::time::Duration;

use anyhow::{anyhow, Result};
use serde_json::json;

use super::support::{json_id, label_map, poll_until, ComposeHarness};

const LOCAL_REALM_ISSUER: &str = "http://localhost:8091/realms/browserpane-dev";

pub async fn run(harness: &ComposeHarness) -> Result<()> {
    let _ = harness.delete_bridge_json("/control-session").await;

    let session = harness
        .post_json(
            "/api/v1/sessions",
            json!({
                "labels": label_map("session-compatibility"),
                "integration_context": {
                    "suite": "bpane-gateway-compose-e2e",
                    "case": "session-compatibility",
                }
            }),
        )
        .await?;
    let session_id = json_id(&session, "id")?;
    let compatibility_mode = session["connect"]["compatibility_mode"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    if compatibility_mode == "legacy_single_runtime" {
        let legacy_status = harness.get_json("/api/session/status").await?;
        if legacy_status["mcp_owner"] != json!(false) {
            return Err(anyhow!(
                "legacy session status unexpectedly started with MCP ownership: {legacy_status}"
            ));
        }

        let legacy_claim = harness
            .post_json(
                "/api/session/mcp-owner",
                json!({
                    "width": 1280,
                    "height": 720,
                }),
            )
            .await?;
        if legacy_claim["ok"] != json!(true) {
            return Err(anyhow!("legacy MCP owner claim did not return ok"));
        }

        let owned_legacy_status = harness.get_json("/api/session/status").await?;
        if owned_legacy_status["mcp_owner"] != json!(true) {
            return Err(anyhow!(
                "legacy MCP owner claim did not toggle legacy status: {owned_legacy_status}"
            ));
        }

        let scoped_status = harness
            .get_json(&format!("/api/v1/sessions/{session_id}/status"))
            .await?;
        if scoped_status["mcp_owner"] != json!(true) {
            return Err(anyhow!(
                "legacy MCP owner claim did not reach the concrete session: {scoped_status}"
            ));
        }

        let legacy_clear = harness.delete_json("/api/session/mcp-owner").await?;
        if legacy_clear["ok"] != json!(true) {
            return Err(anyhow!("legacy MCP owner clear did not return ok"));
        }

        let cleared_legacy_status = harness.get_json("/api/session/status").await?;
        if cleared_legacy_status["mcp_owner"] != json!(false) {
            return Err(anyhow!(
                "legacy MCP owner clear did not release ownership: {cleared_legacy_status}"
            ));
        }
    } else {
        let disabled = harness.get_json_outcome("/api/session/status").await?;
        if disabled.status != reqwest::StatusCode::CONFLICT {
            return Err(anyhow!(
                "legacy global route should be disabled for {compatibility_mode}, got {} {}",
                disabled.status,
                disabled.body
            ));
        }
    }

    let delegated = harness
        .post_json(
            &format!("/api/v1/sessions/{session_id}/automation-owner"),
            json!({
                "client_id": "bpane-mcp-bridge",
                "issuer": LOCAL_REALM_ISSUER,
                "display_name": "BrowserPane MCP bridge",
            }),
        )
        .await?;
    if delegated["automation_delegate"]["client_id"] != json!("bpane-mcp-bridge") {
        return Err(anyhow!(
            "session automation delegation did not persist: {delegated}"
        ));
    }

    let delegated_resource = harness
        .get_json(&format!("/api/v1/sessions/{session_id}"))
        .await?;
    let delegated_cdp = delegated_resource["runtime"]["cdp_endpoint"]
        .as_str()
        .ok_or_else(|| anyhow!("delegated session is missing runtime cdp endpoint"))?
        .to_string();

    let selected = harness
        .put_bridge_json("/control-session", json!({ "session_id": session_id }))
        .await?;
    if selected["session"]["id"] != json!(session_id)
        || selected["cdp_endpoint"] != json!(delegated_cdp)
    {
        return Err(anyhow!(
            "MCP bridge did not adopt the delegated session: {selected}"
        ));
    }

    let registered = harness.post_bridge_json("/register", json!({})).await?;
    if registered["cdp_endpoint"] != json!(delegated_cdp) {
        return Err(anyhow!(
            "MCP bridge did not register the delegated runtime endpoint: {registered}"
        ));
    }

    let bridged = poll_until(
        "MCP bridge health for delegated session",
        Duration::from_secs(15),
        || {
            let harness = harness.clone();
            let session_id = session_id.clone();
            let delegated_cdp = delegated_cdp.clone();
            async move {
                let value = harness.get_bridge_json("/health").await?;
                if value["control_session_id"] == json!(session_id)
                    && value["playwright_cdp_endpoint"] == json!(delegated_cdp)
                {
                    Ok(Some(value))
                } else {
                    Ok(None)
                }
            }
        },
    )
    .await?;
    if bridged["control_session_cdp_endpoint"] != json!(delegated_cdp) {
        return Err(anyhow!(
            "MCP bridge health did not surface the delegated CDP endpoint: {bridged}"
        ));
    }

    let owned_status = harness
        .get_json(&format!("/api/v1/sessions/{session_id}/status"))
        .await?;
    if owned_status["mcp_owner"] != json!(true) {
        return Err(anyhow!(
            "delegated session did not show MCP ownership after bridge register: {owned_status}"
        ));
    }

    let cleared_bridge = harness.delete_bridge_json("/control-session").await?;
    if cleared_bridge["ok"] != json!(true) {
        return Err(anyhow!(
            "MCP bridge control session clear did not return ok: {cleared_bridge}"
        ));
    }

    let _bridge_health_cleared = poll_until(
        "MCP bridge control-session clear",
        Duration::from_secs(15),
        || {
            let harness = harness.clone();
            async move {
                let value = harness.get_bridge_json("/health").await?;
                if value["control_session_id"].is_null()
                    && value["playwright_cdp_endpoint"].is_null()
                {
                    return Ok(Some(value));
                }
                Ok(None)
            }
        },
    )
    .await?;

    let status_after_clear = poll_until(
        "delegated session MCP owner clear",
        Duration::from_secs(15),
        || {
            let harness = harness.clone();
            let path = format!("/api/v1/sessions/{session_id}/status");
            async move {
                let value = harness.get_json(&path).await?;
                if value["mcp_owner"] == json!(false) {
                    return Ok(Some(value));
                }
                Ok(None)
            }
        },
    )
    .await?;
    if status_after_clear["mcp_owner"] != json!(false) {
        return Err(anyhow!(
            "MCP bridge clear left MCP ownership on the delegated session: {status_after_clear}"
        ));
    }

    let deleted = harness.stop_session_eventually(&session_id).await?;
    if deleted["state"] != json!("stopped") {
        return Err(anyhow!(
            "delegated session did not stop cleanly after MCP bridge clear: {deleted}"
        ));
    }

    Ok(())
}
