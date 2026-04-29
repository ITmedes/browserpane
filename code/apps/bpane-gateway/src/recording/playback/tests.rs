use chrono::{DateTime, TimeDelta, Utc};
use uuid::Uuid;

use super::*;
use crate::session_control::{
    SessionRecordingFormat, SessionRecordingState, SessionRecordingTerminationReason,
    StoredSessionRecording,
};

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

    let playback = prepare_session_recording_playback(session_id, &[failed, ready.clone()], now);

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
