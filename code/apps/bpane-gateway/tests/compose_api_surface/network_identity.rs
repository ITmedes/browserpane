use anyhow::{anyhow, Result};
use reqwest::StatusCode;
use serde_json::json;
use uuid::Uuid;

use super::support::{json_array, json_id, label_map, ComposeHarness};

pub async fn run(harness: &ComposeHarness) -> Result<()> {
    let invalid_profile = harness
        .post_json_outcome(
            "/api/v1/egress-profiles",
            json!({
                "name": harness.unique_name("bad-egress"),
                "proxy": { "url": "https://user:pass@proxy.example:8443" }
            }),
        )
        .await?;
    if invalid_profile.status != StatusCode::BAD_REQUEST {
        return Err(anyhow!(
            "invalid egress profile returned {} instead of 400: {}",
            invalid_profile.status,
            invalid_profile.body
        ));
    }

    let disabled_profile = harness
        .post_json(
            "/api/v1/egress-profiles",
            json!({
                "name": harness.unique_name("disabled-egress"),
                "state": "disabled"
            }),
        )
        .await?;
    let disabled_profile_id = json_id(&disabled_profile, "id")?;
    let disabled_session = harness
        .post_json_outcome(
            "/api/v1/sessions",
            json!({
                "network_identity": {
                    "egress_profile_id": disabled_profile_id
                }
            }),
        )
        .await?;
    if disabled_session.status != StatusCode::CONFLICT {
        return Err(anyhow!(
            "disabled egress profile session create returned {} instead of 409: {}",
            disabled_session.status,
            disabled_session.body
        ));
    }

    let missing_profile = Uuid::now_v7();
    let missing_session = harness
        .post_json_outcome(
            "/api/v1/sessions",
            json!({
                "network_identity": {
                    "egress_profile_id": missing_profile
                }
            }),
        )
        .await?;
    if missing_session.status != StatusCode::NOT_FOUND {
        return Err(anyhow!(
            "missing egress profile session create returned {} instead of 404: {}",
            missing_session.status,
            missing_session.body
        ));
    }

    let invalid_identity = harness
        .post_json_outcome(
            "/api/v1/sessions",
            json!({
                "network_identity": {
                    "timezone": "not a timezone",
                    "geolocation": {
                        "latitude": 91.0,
                        "longitude": 13.405
                    }
                }
            }),
        )
        .await?;
    if invalid_identity.status != StatusCode::BAD_REQUEST {
        return Err(anyhow!(
            "invalid network identity returned {} instead of 400: {}",
            invalid_identity.status,
            invalid_identity.body
        ));
    }

    let profile = harness
        .post_json(
            "/api/v1/egress-profiles",
            json!({
                "name": harness.unique_name("eu-support-egress"),
                "description": "Compose e2e support egress",
                "labels": label_map("network-identity"),
                "proxy": { "url": "https://proxy.example:8443" },
                "bypass_rules": ["localhost", "*.internal.example"],
                "custom_ca": {
                    "certificate_ref": "vault://pki/browserpane/eu-support",
                    "display_name": "EU support CA"
                }
            }),
        )
        .await?;
    let profile_id = json_id(&profile, "id")?;
    if profile["effective"]["proxy_configured"] != json!(true)
        || profile["effective"]["bypass_rule_count"] != json!(2)
        || profile["effective"]["custom_ca_configured"] != json!(true)
    {
        return Err(anyhow!(
            "egress profile effective status was unexpected: {profile}"
        ));
    }

    let fetched_profile = harness
        .get_json(&format!("/api/v1/egress-profiles/{profile_id}"))
        .await?;
    if fetched_profile["id"] != json!(profile_id) {
        return Err(anyhow!("egress profile lookup returned unexpected data"));
    }
    let profiles = harness.get_json("/api/v1/egress-profiles").await?;
    if !json_array(&profiles, "profiles")?
        .iter()
        .any(|candidate| candidate.get("id") == Some(&json!(profile_id)))
    {
        return Err(anyhow!("egress profile list did not include {profile_id}"));
    }

    let template = harness
        .post_json(
            "/api/v1/session-templates",
            json!({
                "name": harness.unique_name("network-template"),
                "defaults": {
                    "network_identity": {
                        "locale": "de-DE",
                        "languages": ["de-DE", "en-US"],
                        "timezone": "Europe/Berlin",
                        "geolocation": {
                            "latitude": 52.52,
                            "longitude": 13.405,
                            "accuracy_meters": 100.0
                        },
                        "browser_identity": "desktop-chromium-stable",
                        "egress_profile_id": profile_id
                    },
                    "labels": {
                        "network": "eu-support"
                    }
                }
            }),
        )
        .await?;
    let template_id = json_id(&template, "id")?;
    let session = harness
        .post_json(
            "/api/v1/sessions",
            json!({
                "template_id": template_id,
                "network_identity": {
                    "timezone": "UTC",
                    "user_agent": "BrowserPaneComposeTest/1.0"
                }
            }),
        )
        .await?;
    let session_id = json_id(&session, "id")?;
    if session["network_identity"]["locale"] != json!("de-DE")
        || session["network_identity"]["timezone"] != json!("UTC")
        || session["network_identity"]["egress_profile_id"] != json!(profile_id)
        || session["effective_egress"]["profile_name"].is_null()
        || session["effective_egress"]["bypass_rule_count"] != json!(2)
    {
        return Err(anyhow!(
            "session did not expose inherited network identity and effective egress: {session}"
        ));
    }

    let status = harness
        .get_json(&format!("/api/v1/sessions/{session_id}/status"))
        .await?;
    if status["network_identity"]["timezone"] != json!("UTC")
        || status["effective_egress"]["profile_id"] != json!(profile_id)
    {
        return Err(anyhow!(
            "session status did not include effective network identity: {status}"
        ));
    }

    let stopped = harness
        .delete_json(&format!("/api/v1/sessions/{session_id}"))
        .await?;
    if stopped["state"] != json!("stopped") {
        return Err(anyhow!(
            "network identity session cleanup did not stop session"
        ));
    }

    Ok(())
}
