use anyhow::{anyhow, Result};
use reqwest::StatusCode;
use serde_json::json;

use uuid::Uuid;

use super::support::{json_id, label_map, recording_policy, ComposeHarness};

pub async fn run(harness: &ComposeHarness) -> Result<()> {
    let session = harness
        .post_json(
            "/api/v1/sessions",
            json!({
                "labels": label_map("recording-worker-boundary"),
                "recording": recording_policy("manual"),
            }),
        )
        .await?;
    let session_id = json_id(&session, "id")?;

    // Authorization is evaluated before resource lookup. A synthetic id keeps this
    // boundary check independent of a recorder worker and makes teardown immediate.
    let recording_id = Uuid::now_v7();

    let expected_source_path = format!("/tmp/bpane-recordings/{session_id}/{recording_id}.webm");
    let completion = harness
        .post_json_outcome(
            &format!("/api/v1/sessions/{session_id}/recordings/{recording_id}/complete"),
            json!({
                "source_path": expected_source_path,
                "mime_type": "video/webm",
                "bytes": 11,
                "duration_ms": 900,
            }),
        )
        .await?;
    if completion.status != StatusCode::UNAUTHORIZED
        || !completion.body["error"]
            .as_str()
            .unwrap_or_default()
            .contains("recording worker authorization")
    {
        return Err(anyhow!(
            "owner recording completion was not rejected at the worker boundary: {} {}",
            completion.status,
            completion.body
        ));
    }

    let failure = harness
        .post_json_outcome(
            &format!("/api/v1/sessions/{session_id}/recordings/{recording_id}/fail"),
            json!({
                "error": "owner must not report worker failure",
                "termination_reason": "worker_exit",
            }),
        )
        .await?;
    if failure.status != StatusCode::UNAUTHORIZED {
        return Err(anyhow!(
            "owner recording failure was not rejected at the worker boundary: {} {}",
            failure.status,
            failure.body
        ));
    }

    let _deleted_session = harness
        .delete_json(&format!("/api/v1/sessions/{session_id}"))
        .await?;
    Ok(())
}
