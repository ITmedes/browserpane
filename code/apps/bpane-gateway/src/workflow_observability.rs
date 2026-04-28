use std::sync::atomic::{AtomicU64, Ordering};

use chrono::{DateTime, Utc};
use serde::Serialize;
use tokio::sync::Mutex;

#[derive(Default)]
pub struct WorkflowObservability {
    produced_file_uploads_total: AtomicU64,
    produced_file_upload_failures_total: AtomicU64,
    event_delivery_attempts_total: AtomicU64,
    event_delivery_successes_total: AtomicU64,
    event_delivery_retries_total: AtomicU64,
    event_delivery_failures_total: AtomicU64,
    retention_passes_total: AtomicU64,
    log_retention_candidates_total: AtomicU64,
    output_retention_candidates_total: AtomicU64,
    retention_deleted_logs_total: AtomicU64,
    retention_cleared_outputs_total: AtomicU64,
    retention_failures_total: AtomicU64,
    last_event_delivery_at: Mutex<Option<DateTime<Utc>>>,
    last_retention_pass_at: Mutex<Option<DateTime<Utc>>>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkflowObservabilitySnapshot {
    pub produced_file_uploads_total: u64,
    pub produced_file_upload_failures_total: u64,
    pub event_delivery_attempts_total: u64,
    pub event_delivery_successes_total: u64,
    pub event_delivery_retries_total: u64,
    pub event_delivery_failures_total: u64,
    pub retention_passes_total: u64,
    pub log_retention_candidates_total: u64,
    pub output_retention_candidates_total: u64,
    pub retention_deleted_logs_total: u64,
    pub retention_cleared_outputs_total: u64,
    pub retention_failures_total: u64,
    pub last_event_delivery_at: Option<DateTime<Utc>>,
    pub last_retention_pass_at: Option<DateTime<Utc>>,
}

impl WorkflowObservability {
    pub fn record_produced_file_upload(&self) {
        self.produced_file_uploads_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_produced_file_upload_failure(&self) {
        self.produced_file_upload_failures_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_event_delivery_attempt(&self) {
        self.event_delivery_attempts_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub async fn record_event_delivery_success(&self, at: DateTime<Utc>) {
        self.event_delivery_successes_total
            .fetch_add(1, Ordering::Relaxed);
        *self.last_event_delivery_at.lock().await = Some(at);
    }

    pub fn record_event_delivery_retry(&self) {
        self.event_delivery_retries_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_event_delivery_failure(&self) {
        self.event_delivery_failures_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub async fn record_retention_pass(
        &self,
        at: DateTime<Utc>,
        log_candidate_count: usize,
        output_candidate_count: usize,
    ) {
        self.retention_passes_total.fetch_add(1, Ordering::Relaxed);
        self.log_retention_candidates_total
            .fetch_add(log_candidate_count as u64, Ordering::Relaxed);
        self.output_retention_candidates_total
            .fetch_add(output_candidate_count as u64, Ordering::Relaxed);
        *self.last_retention_pass_at.lock().await = Some(at);
    }

    pub fn record_retention_deleted_logs(&self, deleted: usize) {
        self.retention_deleted_logs_total
            .fetch_add(deleted as u64, Ordering::Relaxed);
    }

    pub fn record_retention_cleared_output(&self) {
        self.retention_cleared_outputs_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_retention_failure(&self) {
        self.retention_failures_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub async fn snapshot(&self) -> WorkflowObservabilitySnapshot {
        WorkflowObservabilitySnapshot {
            produced_file_uploads_total: self.produced_file_uploads_total.load(Ordering::Relaxed),
            produced_file_upload_failures_total: self
                .produced_file_upload_failures_total
                .load(Ordering::Relaxed),
            event_delivery_attempts_total: self
                .event_delivery_attempts_total
                .load(Ordering::Relaxed),
            event_delivery_successes_total: self
                .event_delivery_successes_total
                .load(Ordering::Relaxed),
            event_delivery_retries_total: self.event_delivery_retries_total.load(Ordering::Relaxed),
            event_delivery_failures_total: self
                .event_delivery_failures_total
                .load(Ordering::Relaxed),
            retention_passes_total: self.retention_passes_total.load(Ordering::Relaxed),
            log_retention_candidates_total: self
                .log_retention_candidates_total
                .load(Ordering::Relaxed),
            output_retention_candidates_total: self
                .output_retention_candidates_total
                .load(Ordering::Relaxed),
            retention_deleted_logs_total: self.retention_deleted_logs_total.load(Ordering::Relaxed),
            retention_cleared_outputs_total: self
                .retention_cleared_outputs_total
                .load(Ordering::Relaxed),
            retention_failures_total: self.retention_failures_total.load(Ordering::Relaxed),
            last_event_delivery_at: *self.last_event_delivery_at.lock().await,
            last_retention_pass_at: *self.last_retention_pass_at.lock().await,
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;

    #[tokio::test]
    async fn snapshot_tracks_upload_and_retention_counters() {
        let observability = WorkflowObservability::default();
        let now = Utc::now();

        observability.record_produced_file_upload();
        observability.record_produced_file_upload_failure();
        observability.record_event_delivery_attempt();
        observability.record_event_delivery_retry();
        observability.record_event_delivery_failure();
        observability.record_event_delivery_success(now).await;
        observability.record_retention_deleted_logs(3);
        observability.record_retention_cleared_output();
        observability.record_retention_failure();
        observability.record_retention_pass(now, 2, 1).await;

        let snapshot = observability.snapshot().await;
        assert_eq!(snapshot.produced_file_uploads_total, 1);
        assert_eq!(snapshot.produced_file_upload_failures_total, 1);
        assert_eq!(snapshot.event_delivery_attempts_total, 1);
        assert_eq!(snapshot.event_delivery_successes_total, 1);
        assert_eq!(snapshot.event_delivery_retries_total, 1);
        assert_eq!(snapshot.event_delivery_failures_total, 1);
        assert_eq!(snapshot.retention_passes_total, 1);
        assert_eq!(snapshot.log_retention_candidates_total, 2);
        assert_eq!(snapshot.output_retention_candidates_total, 1);
        assert_eq!(snapshot.retention_deleted_logs_total, 3);
        assert_eq!(snapshot.retention_cleared_outputs_total, 1);
        assert_eq!(snapshot.retention_failures_total, 1);
        assert_eq!(snapshot.last_event_delivery_at, Some(now));
        assert_eq!(snapshot.last_retention_pass_at, Some(now));
    }
}
