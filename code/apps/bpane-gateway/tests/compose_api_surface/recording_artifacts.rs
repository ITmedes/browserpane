use std::io::Read;

use anyhow::{anyhow, Result};
use serde_json::json;

use super::support::{json_array, json_id, label_map, recording_policy, ComposeHarness};

pub async fn run(harness: &ComposeHarness) -> Result<()> {
    let session = harness
        .post_json(
            "/api/v1/sessions",
            json!({
                "labels": label_map("recording-artifacts"),
                "recording": recording_policy("manual"),
            }),
        )
        .await?;
    let session_id = json_id(&session, "id")?;

    let first_recording = harness
        .post_json(
            &format!("/api/v1/sessions/{session_id}/recordings"),
            json!({}),
        )
        .await?;
    let first_recording_id = json_id(&first_recording, "id")?;
    if first_recording["state"] != json!("recording") {
        return Err(anyhow!("first recording did not start in recording state"));
    }

    let stopped_first_recording = harness
        .post_json(
            &format!("/api/v1/sessions/{session_id}/recordings/{first_recording_id}/stop"),
            json!({}),
        )
        .await?;
    if stopped_first_recording["state"] != json!("finalizing") {
        return Err(anyhow!("recording stop did not transition to finalizing"));
    }

    let segment = harness.create_compose_visible_file("segment-1.webm", b"segment-one")?;
    let completed_first_recording = harness
        .post_json(
            &format!("/api/v1/sessions/{session_id}/recordings/{first_recording_id}/complete"),
            json!({
                "source_path": segment.container_path,
                "mime_type": "video/webm",
                "bytes": 11,
                "duration_ms": 900,
            }),
        )
        .await?;
    if completed_first_recording["state"] != json!("ready") {
        return Err(anyhow!("recording complete did not transition to ready"));
    }
    if completed_first_recording["artifact_available"] != json!(true) {
        return Err(anyhow!(
            "completed recording did not expose artifact availability"
        ));
    }

    let content_bytes = harness
        .get_bytes(&format!(
            "/api/v1/sessions/{session_id}/recordings/{first_recording_id}/content"
        ))
        .await?;
    if content_bytes.as_slice() != b"segment-one" {
        return Err(anyhow!(
            "recording content endpoint returned unexpected bytes"
        ));
    }

    let second_recording = harness
        .post_json(
            &format!("/api/v1/sessions/{session_id}/recordings"),
            json!({}),
        )
        .await?;
    let second_recording_id = json_id(&second_recording, "id")?;

    let failed_second_recording = harness
        .post_json(
            &format!("/api/v1/sessions/{session_id}/recordings/{second_recording_id}/fail"),
            json!({
                "error": "recorder worker crashed",
                "termination_reason": "worker_exit",
            }),
        )
        .await?;
    if failed_second_recording["state"] != json!("failed") {
        return Err(anyhow!("recording fail did not transition to failed"));
    }

    let playback = harness
        .get_json(&format!("/api/v1/sessions/{session_id}/recording-playback"))
        .await?;
    if playback["state"] != json!("partial") {
        return Err(anyhow!("recording playback did not expose partial state"));
    }
    if playback["segment_count"] != json!(2) {
        return Err(anyhow!("recording playback did not count both segments"));
    }
    if playback["included_segment_count"] != json!(1) {
        return Err(anyhow!(
            "recording playback did not keep one included segment"
        ));
    }
    if playback["failed_segment_count"] != json!(1) {
        return Err(anyhow!(
            "recording playback did not count one failed segment"
        ));
    }
    if playback["active_segment_count"] != json!(0) {
        return Err(anyhow!(
            "recording playback reported unexpected active segments"
        ));
    }
    if playback["missing_artifact_segment_count"] != json!(0) {
        return Err(anyhow!(
            "recording playback reported unexpected missing artifact segments"
        ));
    }

    let manifest = harness
        .get_json(&format!(
            "/api/v1/sessions/{session_id}/recording-playback/manifest"
        ))
        .await?;
    if manifest["format_version"] != json!("browserpane_recording_playback_v1") {
        return Err(anyhow!(
            "recording playback manifest format version mismatched"
        ));
    }
    let manifest_segments = json_array(&manifest, "segments")?;
    if manifest_segments.len() != 1 {
        return Err(anyhow!(
            "recording playback manifest did not keep one segment"
        ));
    }
    let omitted_segments = json_array(&manifest, "omitted_segments")?;
    if omitted_segments.len() != 1 {
        return Err(anyhow!(
            "recording playback manifest did not omit one segment"
        ));
    }
    if omitted_segments[0]["omitted_reason"] != json!("failed") {
        return Err(anyhow!(
            "recording playback omitted segment reason mismatched"
        ));
    }

    let export_bytes = harness
        .get_bytes(&format!(
            "/api/v1/sessions/{session_id}/recording-playback/export"
        ))
        .await?;
    let cursor = std::io::Cursor::new(export_bytes);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|error| anyhow!("failed to open playback export zip: {error}"))?;
    let mut manifest_file = archive
        .by_name("manifest.json")
        .map_err(|error| anyhow!("playback export is missing manifest.json: {error}"))?;
    let mut manifest_bytes = Vec::new();
    manifest_file
        .read_to_end(&mut manifest_bytes)
        .map_err(|error| anyhow!("failed to read playback manifest from export: {error}"))?;
    drop(manifest_file);
    let manifest_json: serde_json::Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| anyhow!("failed to decode playback export manifest: {error}"))?;
    if manifest_json["segment_count"] != json!(2) {
        return Err(anyhow!(
            "playback export manifest did not preserve segment count"
        ));
    }
    archive
        .by_name("player.html")
        .map_err(|error| anyhow!("playback export is missing player.html: {error}"))?;
    let segment_name = manifest_json["segments"][0]["file_name"]
        .as_str()
        .ok_or_else(|| anyhow!("playback export manifest segment is missing file_name"))?
        .to_string();
    let mut segment_file = archive
        .by_name(&segment_name)
        .map_err(|error| anyhow!("playback export is missing {segment_name}: {error}"))?;
    let mut segment_bytes = Vec::new();
    segment_file
        .read_to_end(&mut segment_bytes)
        .map_err(|error| anyhow!("failed to read playback segment from export: {error}"))?;
    if segment_bytes.as_slice() != b"segment-one" {
        return Err(anyhow!("playback export segment bytes mismatched"));
    }

    let recordings = harness
        .get_json(&format!("/api/v1/sessions/{session_id}/recordings"))
        .await?;
    let recordings = json_array(&recordings, "recordings")?;
    if recordings.len() != 2 {
        return Err(anyhow!(
            "recording list did not expose both recording segments"
        ));
    }

    let _deleted_session = harness
        .delete_json(&format!("/api/v1/sessions/{session_id}"))
        .await?;

    Ok(())
}
