use anyhow::{anyhow, Result};
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::StatusCode;
use serde_json::json;

use super::support::{json_id, ComposeHarness, JsonOutcome};

pub async fn run(harness: &ComposeHarness) -> Result<()> {
    let first = harness.post_json("/api/v1/sessions", json!({})).await?;
    let first_session_id = json_id(&first, "id")?;
    let automation_access = harness
        .post_json(
            &format!("/api/v1/sessions/{first_session_id}/automation-access"),
            json!({}),
        )
        .await?;
    let automation_token = json_id(&automation_access, "token")?;
    harness.stop_session_eventually(&first_session_id).await?;
    let principal = harness
        .create_service_principal("compose-e2e-automation-boundary")
        .await?;
    let foreign_harness = harness.as_service_principal(&principal).await?;
    let second = foreign_harness
        .post_json("/api/v1/sessions", json!({}))
        .await?;
    let second_session_id = json_id(&second, "id")?;
    if first_session_id == second_session_id {
        return Err(anyhow!(
            "automation boundary setup unexpectedly reused session id {first_session_id}"
        ));
    }
    let headers = automation_headers(&automation_token)?;

    let result: Result<()> = async {
        assert_unauthorized(
            harness
                .get_json_outcome_without_bearer(
                    &format!("/api/v1/sessions/{second_session_id}/status"),
                    headers.clone(),
                )
                .await?,
            "wrong-session status lookup",
            Some("does not match the requested session"),
        )?;
        assert_unauthorized(
            harness
                .post_json_outcome_without_bearer(
                    &format!("/api/v1/sessions/{second_session_id}/mcp-owner"),
                    json!({
                        "width": 1280,
                        "height": 720,
                    }),
                    headers.clone(),
                )
                .await?,
            "wrong-session mcp-owner claim",
            Some("does not match the requested session"),
        )?;
        assert_unauthorized(
            harness
                .delete_json_outcome_without_bearer(
                    &format!("/api/v1/sessions/{first_session_id}"),
                    headers,
                )
                .await?,
            "owner-only delete with automation token",
            Some("missing bearer token"),
        )?;
        Ok(())
    }
    .await;

    let second_stop = foreign_harness
        .stop_session_eventually(&second_session_id)
        .await;
    let delete_principal = harness.delete_service_principal(&principal).await;

    result?;
    second_stop?;
    delete_principal?;
    Ok(())
}

fn automation_headers(token: &str) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-bpane-automation-access-token",
        HeaderValue::from_str(token)
            .map_err(|error| anyhow!("invalid automation token header: {error}"))?,
    );
    Ok(headers)
}

fn assert_unauthorized(
    outcome: JsonOutcome,
    context: &str,
    expected_error_fragment: Option<&str>,
) -> Result<()> {
    if outcome.status != StatusCode::UNAUTHORIZED {
        return Err(anyhow!(
            "{context} returned {} instead of 401: {}",
            outcome.status,
            outcome.body
        ));
    }
    if let Some(fragment) = expected_error_fragment {
        let error = outcome
            .body
            .get("error")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                anyhow!(
                    "{context} did not return an error message: {}",
                    outcome.body
                )
            })?;
        if !error.contains(fragment) {
            return Err(anyhow!(
                "{context} returned unexpected error {error:?}; expected fragment {fragment:?}"
            ));
        }
    }
    Ok(())
}
