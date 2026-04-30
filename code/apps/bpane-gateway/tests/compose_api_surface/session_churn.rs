use std::time::Duration;

use anyhow::{anyhow, Result};
use reqwest::StatusCode;
use serde_json::json;

use super::support::{json_array, json_id, label_map, poll_until, ComposeHarness};

pub async fn run(harness: &ComposeHarness) -> Result<()> {
    for iteration in 0..5 {
        let created = harness
            .post_json(
                "/api/v1/sessions",
                json!({
                    "labels": label_map("session-churn-sequential"),
                    "integration_context": {
                        "suite": "bpane-gateway-compose-e2e",
                        "case": "session-churn-sequential",
                        "iteration": iteration,
                    }
                }),
            )
            .await?;
        let session_id = json_id(&created, "id")?;

        let fetched = harness
            .get_json(&format!("/api/v1/sessions/{session_id}"))
            .await?;
        if fetched["id"] != json!(session_id) {
            return Err(anyhow!(
                "session lookup returned the wrong resource during churn"
            ));
        }

        let status = harness
            .get_json(&format!("/api/v1/sessions/{session_id}/status"))
            .await?;
        if !status["recording"].is_object() {
            return Err(anyhow!(
                "session status did not remain readable during churn"
            ));
        }

        let _connect = harness
            .post_json(
                &format!("/api/v1/sessions/{session_id}/access-tokens"),
                json!({}),
            )
            .await?;
        let deleted = harness
            .delete_json(&format!("/api/v1/sessions/{session_id}"))
            .await?;
        if deleted["state"] != json!("stopped") {
            return Err(anyhow!(
                "session delete did not stop the session during churn: {deleted}"
            ));
        }

        let stopped_resource = harness
            .get_json(&format!("/api/v1/sessions/{session_id}"))
            .await?;
        if stopped_resource["state"] != json!("stopped") {
            return Err(anyhow!(
                "stopped session resource did not remain readable during churn: {stopped_resource}"
            ));
        }

        let stopped_status = harness
            .get_json_outcome(&format!("/api/v1/sessions/{session_id}/status"))
            .await?;
        if stopped_status.status != StatusCode::OK {
            return Err(anyhow!(
                "stopped session status returned unexpected status {} {}",
                stopped_status.status,
                stopped_status.body
            ));
        }
        if stopped_status.body["state"] != json!("stopped")
            || stopped_status.body["runtime_state"] != json!("stopped")
            || stopped_status.body["presence_state"] != json!("empty")
            || stopped_status.body["connection_counts"]["total_clients"] != json!(0)
        {
            return Err(anyhow!(
                "stopped session status did not expose the expected lifecycle snapshot: {}",
                stopped_status.body
            ));
        }

        let resumed_ticket = harness
            .post_json(
                &format!("/api/v1/sessions/{session_id}/access-tokens"),
                json!({}),
            )
            .await?;
        if resumed_ticket["token"]
            .as_str()
            .unwrap_or_default()
            .is_empty()
        {
            return Err(anyhow!(
                "stopped session did not issue a fresh connect ticket during churn"
            ));
        }

        let re_deleted = harness
            .delete_json(&format!("/api/v1/sessions/{session_id}"))
            .await?;
        if re_deleted["state"] != json!("stopped") {
            return Err(anyhow!(
                "re-stopped session did not return stopped state during churn: {re_deleted}"
            ));
        }
    }

    let created = harness
        .post_json(
            "/api/v1/sessions",
            json!({
                "labels": label_map("session-churn-concurrent"),
                "integration_context": {
                    "suite": "bpane-gateway-compose-e2e",
                    "case": "session-churn-concurrent",
                }
            }),
        )
        .await?;
    let session_id = json_id(&created, "id")?;

    let delete_harness = harness.clone();
    let create_harness = harness.clone();
    let delete_path = format!("/api/v1/sessions/{session_id}");
    let delete_future =
        tokio::spawn(async move { delete_harness.delete_json_outcome(&delete_path).await });
    let create_future = tokio::spawn(async move {
        create_harness
            .post_json_outcome(
                "/api/v1/sessions",
                json!({
                    "labels": label_map("session-churn-concurrent-recreate"),
                    "integration_context": {
                        "suite": "bpane-gateway-compose-e2e",
                        "case": "session-churn-concurrent-recreate",
                    }
                }),
            )
            .await
    });

    let delete_outcome = delete_future.await??;
    if delete_outcome.status != StatusCode::OK {
        return Err(anyhow!(
            "concurrent session delete returned unexpected status {} {}",
            delete_outcome.status,
            delete_outcome.body
        ));
    }
    if delete_outcome.body["state"] != json!("stopped") {
        return Err(anyhow!(
            "concurrent session delete did not stop the session: {}",
            delete_outcome.body
        ));
    }

    let create_outcome = create_future.await??;
    let recreated = match create_outcome.status {
        StatusCode::CREATED => create_outcome.body,
        StatusCode::CONFLICT => {
            poll_until(
                "session recreation after concurrent stop",
                Duration::from_secs(10),
                || {
                    let retry_harness = harness.clone();
                    async move {
                        let retry = retry_harness
                            .post_json_outcome(
                                "/api/v1/sessions",
                                json!({
                                    "labels": label_map("session-churn-concurrent-recreate"),
                                    "integration_context": {
                                        "suite": "bpane-gateway-compose-e2e",
                                        "case": "session-churn-concurrent-recreate-retry",
                                    }
                                }),
                            )
                            .await?;
                        match retry.status {
                            StatusCode::CREATED => Ok(Some(retry.body)),
                            StatusCode::CONFLICT => Ok(None),
                            status => Err(anyhow!(
                                "unexpected status {status} while retrying session recreation: {}",
                                retry.body
                            )),
                        }
                    }
                },
            )
            .await?
        }
        status => {
            return Err(anyhow!(
                "concurrent session recreation returned unexpected status {} {}",
                status,
                create_outcome.body
            ));
        }
    };
    let recreated_id = json_id(&recreated, "id")?;

    let _connect = harness
        .post_json(
            &format!("/api/v1/sessions/{recreated_id}/access-tokens"),
            json!({}),
        )
        .await?;
    let _automation_access = harness
        .post_json(
            &format!("/api/v1/sessions/{recreated_id}/automation-access"),
            json!({}),
        )
        .await?;

    let recreated_deleted = harness
        .delete_json(&format!("/api/v1/sessions/{recreated_id}"))
        .await?;
    if recreated_deleted["state"] != json!("stopped") {
        return Err(anyhow!(
            "recreated session delete did not stop the session: {recreated_deleted}"
        ));
    }

    let sessions = harness.get_json("/api/v1/sessions").await?;
    if !json_array(&sessions, "sessions")?
        .iter()
        .all(|session| session["state"] == json!("stopped"))
    {
        return Err(anyhow!(
            "session churn left non-stopped sessions behind: {sessions}"
        ));
    }

    Ok(())
}
