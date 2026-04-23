use std::io::Write;

use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;
use zip::write::SimpleFileOptions;

use crate::recording_artifact_store::{RecordingArtifactStore, RecordingArtifactStoreError};
use crate::session_control::{
    SessionRecordingFormat, SessionRecordingState, SessionRecordingTerminationReason,
    StoredSessionRecording,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionRecordingPlaybackState {
    Empty,
    Ready,
    Partial,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionRecordingPlaybackResource {
    pub session_id: Uuid,
    pub state: SessionRecordingPlaybackState,
    pub segment_count: u32,
    pub included_segment_count: u32,
    pub failed_segment_count: u32,
    pub active_segment_count: u32,
    pub missing_artifact_segment_count: u32,
    pub included_bytes: u64,
    pub included_duration_ms: u64,
    pub manifest_path: String,
    pub export_path: String,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionRecordingPlaybackManifest {
    pub format_version: &'static str,
    pub session_id: Uuid,
    pub state: SessionRecordingPlaybackState,
    pub segment_count: u32,
    pub included_segment_count: u32,
    pub failed_segment_count: u32,
    pub active_segment_count: u32,
    pub missing_artifact_segment_count: u32,
    pub included_bytes: u64,
    pub included_duration_ms: u64,
    pub generated_at: DateTime<Utc>,
    pub segments: Vec<SessionRecordingPlaybackSegment>,
    pub omitted_segments: Vec<SessionRecordingPlaybackOmittedSegment>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionRecordingPlaybackSegment {
    pub sequence: u32,
    pub recording_id: Uuid,
    pub previous_recording_id: Option<Uuid>,
    pub file_name: String,
    pub content_path: String,
    pub mime_type: String,
    pub bytes: Option<u64>,
    pub duration_ms: Option<u64>,
    pub termination_reason: Option<SessionRecordingTerminationReason>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionRecordingPlaybackOmittedSegment {
    pub recording_id: Uuid,
    pub previous_recording_id: Option<Uuid>,
    pub state: SessionRecordingState,
    pub artifact_available: bool,
    pub omitted_reason: String,
    pub termination_reason: Option<SessionRecordingTerminationReason>,
    pub error: Option<String>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, thiserror::Error)]
pub enum RecordingPlaybackError {
    #[error("recording playback export is empty")]
    Empty,
    #[error("failed to encode playback manifest: {0}")]
    ManifestEncode(#[from] serde_json::Error),
    #[error("failed to write playback export: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to load recording artifact for playback export: {0}")]
    Artifact(#[from] RecordingArtifactStoreError),
    #[error("failed to package playback export: {0}")]
    Package(#[from] zip::result::ZipError),
}

#[derive(Debug, Clone)]
struct PreparedPlaybackSegmentArtifact {
    file_name: String,
    artifact_ref: String,
}

#[derive(Debug, Clone)]
pub struct PreparedSessionRecordingPlayback {
    pub resource: SessionRecordingPlaybackResource,
    pub manifest: SessionRecordingPlaybackManifest,
    segment_artifacts: Vec<PreparedPlaybackSegmentArtifact>,
}

impl PreparedSessionRecordingPlayback {
    pub async fn export_bundle(
        &self,
        artifact_store: &RecordingArtifactStore,
    ) -> Result<Vec<u8>, RecordingPlaybackError> {
        if self.segment_artifacts.is_empty() {
            return Err(RecordingPlaybackError::Empty);
        }

        let manifest_json = serde_json::to_vec_pretty(&self.manifest)?;
        let player_html = build_player_html(&self.manifest)?;
        let cursor = std::io::Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(cursor);
        let file_options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

        zip.start_file("manifest.json", file_options)?;
        zip.write_all(&manifest_json)?;
        zip.start_file("player.html", file_options)?;
        zip.write_all(player_html.as_bytes())?;

        for segment in &self.segment_artifacts {
            let bytes = artifact_store.read(&segment.artifact_ref).await?;
            zip.start_file(&segment.file_name, file_options)?;
            zip.write_all(&bytes)?;
        }

        let cursor = zip.finish()?;
        Ok(cursor.into_inner())
    }
}

pub fn prepare_session_recording_playback(
    session_id: Uuid,
    recordings: &[StoredSessionRecording],
    generated_at: DateTime<Utc>,
) -> PreparedSessionRecordingPlayback {
    let manifest_path = format!("/api/v1/sessions/{session_id}/recording-playback/manifest");
    let export_path = format!("/api/v1/sessions/{session_id}/recording-playback/export");
    let mut ordered = recordings.to_vec();
    ordered.sort_by(|left, right| {
        left.started_at
            .cmp(&right.started_at)
            .then_with(|| left.created_at.cmp(&right.created_at))
            .then_with(|| left.id.cmp(&right.id))
    });

    let mut included_bytes = 0_u64;
    let mut included_duration_ms = 0_u64;
    let mut included_segments = Vec::new();
    let mut omitted_segments = Vec::new();
    let mut segment_artifacts = Vec::new();
    let mut failed_segment_count = 0_u32;
    let mut active_segment_count = 0_u32;
    let mut missing_artifact_segment_count = 0_u32;

    for recording in ordered {
        let artifact_available = recording.artifact_ref.is_some();
        match recording.state {
            SessionRecordingState::Ready if artifact_available => {
                let sequence = included_segments.len() as u32 + 1;
                let file_name = format!(
                    "segments/{sequence:04}-{}.{}",
                    recording.id,
                    recording_extension(recording.format)
                );
                included_bytes = included_bytes.saturating_add(recording.bytes.unwrap_or(0));
                included_duration_ms =
                    included_duration_ms.saturating_add(recording.duration_ms.unwrap_or(0));
                included_segments.push(SessionRecordingPlaybackSegment {
                    sequence,
                    recording_id: recording.id,
                    previous_recording_id: recording.previous_recording_id,
                    file_name: file_name.clone(),
                    content_path: format!(
                        "/api/v1/sessions/{}/recordings/{}/content",
                        recording.session_id, recording.id
                    ),
                    mime_type: recording
                        .mime_type
                        .clone()
                        .unwrap_or_else(|| recording_mime_type(recording.format).to_string()),
                    bytes: recording.bytes,
                    duration_ms: recording.duration_ms,
                    termination_reason: recording.termination_reason,
                    started_at: recording.started_at,
                    completed_at: recording.completed_at,
                });
                segment_artifacts.push(PreparedPlaybackSegmentArtifact {
                    file_name,
                    artifact_ref: recording.artifact_ref.unwrap_or_default(),
                });
            }
            SessionRecordingState::Ready => {
                missing_artifact_segment_count += 1;
                omitted_segments.push(omitted_segment(recording, "artifact_missing"));
            }
            SessionRecordingState::Failed => {
                failed_segment_count += 1;
                omitted_segments.push(omitted_segment(recording, "failed"));
            }
            _ => {
                active_segment_count += 1;
                omitted_segments.push(omitted_segment(recording, "in_progress"));
            }
        }
    }

    let segment_count = (included_segments.len() + omitted_segments.len()) as u32;
    let state = if segment_count == 0 {
        SessionRecordingPlaybackState::Empty
    } else if omitted_segments.is_empty() {
        SessionRecordingPlaybackState::Ready
    } else {
        SessionRecordingPlaybackState::Partial
    };
    let manifest = SessionRecordingPlaybackManifest {
        format_version: "browserpane_recording_playback_v1",
        session_id,
        state,
        segment_count,
        included_segment_count: included_segments.len() as u32,
        failed_segment_count,
        active_segment_count,
        missing_artifact_segment_count,
        included_bytes,
        included_duration_ms,
        generated_at,
        segments: included_segments,
        omitted_segments,
    };
    let resource = SessionRecordingPlaybackResource {
        session_id,
        state,
        segment_count,
        included_segment_count: manifest.included_segment_count,
        failed_segment_count,
        active_segment_count,
        missing_artifact_segment_count,
        included_bytes,
        included_duration_ms,
        manifest_path,
        export_path,
        generated_at,
    };

    PreparedSessionRecordingPlayback {
        resource,
        manifest,
        segment_artifacts,
    }
}

fn recording_extension(format: SessionRecordingFormat) -> &'static str {
    match format {
        SessionRecordingFormat::Webm => "webm",
    }
}

fn recording_mime_type(format: SessionRecordingFormat) -> &'static str {
    match format {
        SessionRecordingFormat::Webm => "video/webm",
    }
}

fn omitted_segment(
    recording: StoredSessionRecording,
    omitted_reason: &str,
) -> SessionRecordingPlaybackOmittedSegment {
    SessionRecordingPlaybackOmittedSegment {
        recording_id: recording.id,
        previous_recording_id: recording.previous_recording_id,
        state: recording.state,
        artifact_available: recording.artifact_ref.is_some(),
        omitted_reason: omitted_reason.to_string(),
        termination_reason: recording.termination_reason,
        error: recording.error,
        started_at: recording.started_at,
        completed_at: recording.completed_at,
    }
}

fn build_player_html(
    manifest: &SessionRecordingPlaybackManifest,
) -> Result<String, RecordingPlaybackError> {
    let manifest_json = serde_json::to_string(manifest)?;
    Ok(format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>BrowserPane Recording Playback</title>
  <style>
    :root {{
      color-scheme: light;
      font-family: "SF Mono", "Menlo", monospace;
      background: linear-gradient(160deg, #f2efe7, #ddd7cb);
      color: #1b1a17;
    }}
    body {{
      margin: 0;
      min-height: 100vh;
      display: grid;
      place-items: center;
      padding: 32px;
    }}
    main {{
      width: min(960px, 100%);
      background: rgba(255, 255, 255, 0.82);
      backdrop-filter: blur(12px);
      border: 1px solid rgba(0, 0, 0, 0.08);
      border-radius: 24px;
      box-shadow: 0 28px 80px rgba(38, 32, 21, 0.14);
      padding: 24px;
    }}
    h1 {{
      margin-top: 0;
      font-size: 20px;
      letter-spacing: 0.04em;
      text-transform: uppercase;
    }}
    video {{
      width: 100%;
      border-radius: 16px;
      background: #111;
    }}
    .meta {{
      display: flex;
      gap: 12px;
      flex-wrap: wrap;
      margin: 16px 0;
      font-size: 13px;
    }}
    .meta span {{
      padding: 6px 10px;
      background: rgba(0, 0, 0, 0.05);
      border-radius: 999px;
    }}
    ol {{
      padding-left: 20px;
      margin: 0;
      display: grid;
      gap: 6px;
      font-size: 13px;
    }}
    button {{
      margin-right: 8px;
    }}
  </style>
</head>
<body>
  <main>
    <h1>BrowserPane Recording Playback</h1>
    <video id="player" controls autoplay></video>
    <div class="meta" id="summary"></div>
    <p>
      <button id="prev">Previous</button>
      <button id="next">Next</button>
    </p>
    <ol id="segments"></ol>
  </main>
  <script>
    const manifest = {manifest_json};
    const player = document.getElementById('player');
    const summary = document.getElementById('summary');
    const segmentList = document.getElementById('segments');
    const prev = document.getElementById('prev');
    const next = document.getElementById('next');
    let index = 0;

    function renderSummary() {{
      summary.innerHTML = '';
      [
        `state: ${{manifest.state}}`,
        `segments: ${{manifest.included_segment_count}} / ${{manifest.segment_count}}`,
        `duration_ms: ${{manifest.included_duration_ms}}`,
        `bytes: ${{manifest.included_bytes}}`
      ].forEach((value) => {{
        const node = document.createElement('span');
        node.textContent = value;
        summary.appendChild(node);
      }});
    }}

    function renderSegments() {{
      segmentList.innerHTML = '';
      manifest.segments.forEach((segment, segmentIndex) => {{
        const item = document.createElement('li');
        item.textContent = `${{segment.sequence}}. ${{segment.recording_id}}`;
        if (segmentIndex === index) {{
          item.style.fontWeight = '700';
        }}
        segmentList.appendChild(item);
      }});
    }}

    function loadSegment(segmentIndex) {{
      if (!manifest.segments.length) {{
        return;
      }}
      index = Math.max(0, Math.min(segmentIndex, manifest.segments.length - 1));
      player.src = manifest.segments[index].file_name;
      renderSegments();
    }}

    player.addEventListener('ended', () => {{
      if (index + 1 < manifest.segments.length) {{
        loadSegment(index + 1);
        player.play().catch(() => {{}});
      }}
    }});
    prev.addEventListener('click', () => loadSegment(index - 1));
    next.addEventListener('click', () => loadSegment(index + 1));

    renderSummary();
    renderSegments();
    loadSegment(0);
  </script>
</body>
</html>
"#
    ))
}

#[cfg(test)]
mod tests {
    use chrono::TimeDelta;

    use super::*;

    fn ready_recording(
        session_id: Uuid,
        recording_id: Uuid,
        previous_recording_id: Option<Uuid>,
        started_at: DateTime<Utc>,
    ) -> StoredSessionRecording {
        StoredSessionRecording {
            id: recording_id,
            session_id,
            previous_recording_id,
            state: SessionRecordingState::Ready,
            format: SessionRecordingFormat::Webm,
            mime_type: Some("video/webm".to_string()),
            bytes: Some(1024),
            duration_ms: Some(500),
            error: None,
            termination_reason: Some(SessionRecordingTerminationReason::ManualStop),
            artifact_ref: Some(format!("local_fs:{session_id}/{recording_id}.webm")),
            started_at,
            completed_at: Some(started_at + TimeDelta::milliseconds(500)),
            created_at: started_at,
            updated_at: started_at + TimeDelta::milliseconds(500),
        }
    }

    #[test]
    fn prepare_playback_marks_partial_when_segments_are_missing() {
        let session_id = Uuid::now_v7();
        let first_id = Uuid::now_v7();
        let second_id = Uuid::now_v7();
        let now = Utc::now();
        let ready = ready_recording(session_id, first_id, None, now);
        let failed = StoredSessionRecording {
            id: second_id,
            session_id,
            previous_recording_id: Some(first_id),
            state: SessionRecordingState::Failed,
            format: SessionRecordingFormat::Webm,
            mime_type: Some("video/webm".to_string()),
            bytes: None,
            duration_ms: None,
            error: Some("worker exited".to_string()),
            termination_reason: Some(SessionRecordingTerminationReason::WorkerExit),
            artifact_ref: None,
            started_at: now + TimeDelta::seconds(1),
            completed_at: Some(now + TimeDelta::seconds(2)),
            created_at: now + TimeDelta::seconds(1),
            updated_at: now + TimeDelta::seconds(2),
        };

        let playback =
            prepare_session_recording_playback(session_id, &[failed, ready.clone()], now);

        assert_eq!(
            playback.resource.state,
            SessionRecordingPlaybackState::Partial
        );
        assert_eq!(playback.resource.segment_count, 2);
        assert_eq!(playback.resource.included_segment_count, 1);
        assert_eq!(playback.resource.failed_segment_count, 1);
        assert_eq!(playback.resource.missing_artifact_segment_count, 0);
        assert_eq!(playback.manifest.segments.len(), 1);
        assert_eq!(playback.manifest.segments[0].recording_id, ready.id);
        assert_eq!(playback.manifest.omitted_segments.len(), 1);
        assert_eq!(
            playback.manifest.omitted_segments[0].omitted_reason,
            "failed"
        );
    }
}
