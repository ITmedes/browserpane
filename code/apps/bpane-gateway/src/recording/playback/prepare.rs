use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::model::{
    omitted_segment, PreparedPlaybackSegmentArtifact, PreparedSessionRecordingPlayback,
    SessionRecordingPlaybackManifest, SessionRecordingPlaybackResource,
    SessionRecordingPlaybackSegment, SessionRecordingPlaybackState,
};
use crate::session_control::{
    SessionRecordingFormat, SessionRecordingState, StoredSessionRecording,
};

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
