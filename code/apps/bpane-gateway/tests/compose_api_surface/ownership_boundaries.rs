use anyhow::{anyhow, Result};
use reqwest::StatusCode;
use serde_json::json;

use super::support::{json_array, json_id, ComposeHarness};

pub async fn run(harness: &ComposeHarness) -> Result<()> {
    let principal = harness
        .create_service_principal("compose-e2e-foreign-owner")
        .await?;
    let foreign_harness = harness.as_service_principal(&principal).await?;
    let session = harness.post_json("/api/v1/sessions", json!({})).await?;
    let session_id = json_id(&session, "id")?;

    let result = async {
        let listed = foreign_harness.get_json("/api/v1/sessions").await?;
        let listed = json_array(&listed, "sessions")?;
        if listed
            .iter()
            .any(|candidate| candidate.get("id") == Some(&json!(session_id)))
        {
            return Err(anyhow!(
                "foreign principal unexpectedly observed owner session {session_id} in list output"
            ));
        }

        assert_not_found(
            foreign_harness
                .get_json_outcome(&format!("/api/v1/sessions/{session_id}"))
                .await?,
            "foreign session lookup",
        )?;
        assert_not_found(
            foreign_harness
                .get_json_outcome(&format!("/api/v1/sessions/{session_id}/status"))
                .await?,
            "foreign session status lookup",
        )?;
        assert_not_found(
            foreign_harness
                .post_json_outcome(
                    &format!("/api/v1/sessions/{session_id}/automation-access"),
                    json!({}),
                )
                .await?,
            "foreign automation access issue",
        )?;
        assert_not_found(
            foreign_harness
                .post_json_outcome(
                    &format!("/api/v1/sessions/{session_id}/automation-owner"),
                    json!({
                        "client_id": principal.client_id,
                        "issuer": "http://localhost:8091/realms/browserpane-dev",
                        "display_name": "Foreign owner principal",
                    }),
                )
                .await?,
            "foreign automation delegate set",
        )?;
        assert_not_found(
            foreign_harness
                .post_json_outcome(
                    &format!("/api/v1/sessions/{session_id}/mcp-owner"),
                    json!({
                        "width": 1280,
                        "height": 720,
                    }),
                )
                .await?,
            "foreign mcp owner claim",
        )?;
        assert_not_found(
            foreign_harness
                .delete_json_outcome(&format!("/api/v1/sessions/{session_id}"))
                .await?,
            "foreign session delete",
        )?;

        let owner_lookup = harness
            .get_json(&format!("/api/v1/sessions/{session_id}"))
            .await?;
        if owner_lookup["id"] != json!(session_id) {
            return Err(anyhow!(
                "owner lookup stopped resolving session {session_id} after foreign access checks"
            ));
        }

        Ok(())
    }
    .await;

    let stop_result = harness.stop_session_eventually(&session_id).await;
    let delete_principal_result = harness.delete_service_principal(&principal).await;

    result?;
    stop_result?;
    delete_principal_result?;
    Ok(())
}

fn assert_not_found(outcome: super::support::JsonOutcome, context: &str) -> Result<()> {
    if outcome.status != StatusCode::NOT_FOUND {
        return Err(anyhow!(
            "{context} returned {} instead of 404: {}",
            outcome.status,
            outcome.body
        ));
    }
    Ok(())
}
