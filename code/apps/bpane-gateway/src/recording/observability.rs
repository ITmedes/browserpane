use std::sync::atomic::{AtomicU64, Ordering};

use chrono::{DateTime, Utc};
use serde::Serialize;
use tokio::sync::Mutex;

#[derive(Default)]
pub struct RecordingObservability {
    artifact_finalize_requests_total: AtomicU64,
    artifact_finalize_successes_total: AtomicU64,
    artifact_finalize_failures_total: AtomicU64,
    recording_failures_total: AtomicU64,
    playback_manifest_requests_total: AtomicU64,
    playback_export_requests_total: AtomicU64,
    playback_export_successes_total: AtomicU64,
    playback_export_failures_total: AtomicU64,
    playback_export_bytes_total: AtomicU64,
    retention_passes_total: AtomicU64,
    retention_candidates_total: AtomicU64,
    retention_deleted_artifacts_total: AtomicU64,
    retention_failures_total: AtomicU64,
    last_playback_export_at: Mutex<Option<DateTime<Utc>>>,
    last_retention_pass_at: Mutex<Option<DateTime<Utc>>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecordingObservabilitySnapshot {
    pub artifact_finalize_requests_total: u64,
    pub artifact_finalize_successes_total: u64,
    pub artifact_finalize_failures_total: u64,
    pub recording_failures_total: u64,
    pub playback_manifest_requests_total: u64,
    pub playback_export_requests_total: u64,
    pub playback_export_successes_total: u64,
    pub playback_export_failures_total: u64,
    pub playback_export_bytes_total: u64,
    pub retention_passes_total: u64,
    pub retention_candidates_total: u64,
    pub retention_deleted_artifacts_total: u64,
    pub retention_failures_total: u64,
    pub last_playback_export_at: Option<DateTime<Utc>>,
    pub last_retention_pass_at: Option<DateTime<Utc>>,
}

impl RecordingObservability {
    pub fn record_artifact_finalize_request(&self) {
        self.artifact_finalize_requests_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_artifact_finalize_success(&self) {
        self.artifact_finalize_successes_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_artifact_finalize_failure(&self) {
        self.artifact_finalize_failures_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_recording_failure(&self) {
        self.recording_failures_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_playback_manifest_request(&self) {
        self.playback_manifest_requests_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_playback_export_request(&self) {
        self.playback_export_requests_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub async fn record_playback_export_success(&self, bytes: u64, at: DateTime<Utc>) {
        self.playback_export_successes_total
            .fetch_add(1, Ordering::Relaxed);
        self.playback_export_bytes_total
            .fetch_add(bytes, Ordering::Relaxed);
        *self.last_playback_export_at.lock().await = Some(at);
    }

    pub fn record_playback_export_failure(&self) {
        self.playback_export_failures_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub async fn record_retention_pass(&self, at: DateTime<Utc>, candidate_count: usize) {
        self.retention_passes_total.fetch_add(1, Ordering::Relaxed);
        self.retention_candidates_total
            .fetch_add(candidate_count as u64, Ordering::Relaxed);
        *self.last_retention_pass_at.lock().await = Some(at);
    }

    pub fn record_retention_deleted_artifact(&self) {
        self.retention_deleted_artifacts_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_retention_failure(&self) {
        self.retention_failures_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub async fn snapshot(&self) -> RecordingObservabilitySnapshot {
        RecordingObservabilitySnapshot {
            artifact_finalize_requests_total: self
                .artifact_finalize_requests_total
                .load(Ordering::Relaxed),
            artifact_finalize_successes_total: self
                .artifact_finalize_successes_total
                .load(Ordering::Relaxed),
            artifact_finalize_failures_total: self
                .artifact_finalize_failures_total
                .load(Ordering::Relaxed),
            recording_failures_total: self.recording_failures_total.load(Ordering::Relaxed),
            playback_manifest_requests_total: self
                .playback_manifest_requests_total
                .load(Ordering::Relaxed),
            playback_export_requests_total: self
                .playback_export_requests_total
                .load(Ordering::Relaxed),
            playback_export_successes_total: self
                .playback_export_successes_total
                .load(Ordering::Relaxed),
            playback_export_failures_total: self
                .playback_export_failures_total
                .load(Ordering::Relaxed),
            playback_export_bytes_total: self.playback_export_bytes_total.load(Ordering::Relaxed),
            retention_passes_total: self.retention_passes_total.load(Ordering::Relaxed),
            retention_candidates_total: self.retention_candidates_total.load(Ordering::Relaxed),
            retention_deleted_artifacts_total: self
                .retention_deleted_artifacts_total
                .load(Ordering::Relaxed),
            retention_failures_total: self.retention_failures_total.load(Ordering::Relaxed),
            last_playback_export_at: *self.last_playback_export_at.lock().await,
            last_retention_pass_at: *self.last_retention_pass_at.lock().await,
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeDelta;

    use super::*;

    #[tokio::test]
    async fn snapshot_tracks_recording_operations() {
        let observability = RecordingObservability::default();
        let now = Utc::now();

        observability.record_artifact_finalize_request();
        observability.record_artifact_finalize_success();
        observability.record_artifact_finalize_failure();
        observability.record_recording_failure();
        observability.record_playback_manifest_request();
        observability.record_playback_export_request();
        observability.record_playback_export_failure();
        observability
            .record_playback_export_success(4096, now + TimeDelta::seconds(1))
            .await;
        observability.record_retention_deleted_artifact();
        observability.record_retention_failure();
        observability.record_retention_pass(now, 3).await;

        let snapshot = observability.snapshot().await;
        assert_eq!(snapshot.artifact_finalize_requests_total, 1);
        assert_eq!(snapshot.artifact_finalize_successes_total, 1);
        assert_eq!(snapshot.artifact_finalize_failures_total, 1);
        assert_eq!(snapshot.recording_failures_total, 1);
        assert_eq!(snapshot.playback_manifest_requests_total, 1);
        assert_eq!(snapshot.playback_export_requests_total, 1);
        assert_eq!(snapshot.playback_export_successes_total, 1);
        assert_eq!(snapshot.playback_export_failures_total, 1);
        assert_eq!(snapshot.playback_export_bytes_total, 4096);
        assert_eq!(snapshot.retention_passes_total, 1);
        assert_eq!(snapshot.retention_candidates_total, 3);
        assert_eq!(snapshot.retention_deleted_artifacts_total, 1);
        assert_eq!(snapshot.retention_failures_total, 1);
        assert_eq!(snapshot.last_retention_pass_at, Some(now));
        assert_eq!(
            snapshot.last_playback_export_at,
            Some(now + TimeDelta::seconds(1))
        );
    }
}
