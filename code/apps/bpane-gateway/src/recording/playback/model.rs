use std::io::Write;

use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;
use zip::write::SimpleFileOptions;

use super::player::build_player_html;
use crate::recording::{RecordingArtifactStore, RecordingArtifactStoreError};
use crate::session_control::{
    SessionRecordingState, SessionRecordingTerminationReason, StoredSessionRecording,
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
pub(super) struct PreparedPlaybackSegmentArtifact {
    pub(super) file_name: String,
    pub(super) artifact_ref: String,
}

#[derive(Debug, Clone)]
pub struct PreparedSessionRecordingPlayback {
    pub resource: SessionRecordingPlaybackResource,
    pub manifest: SessionRecordingPlaybackManifest,
    pub(super) segment_artifacts: Vec<PreparedPlaybackSegmentArtifact>,
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

pub(super) fn omitted_segment(
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
