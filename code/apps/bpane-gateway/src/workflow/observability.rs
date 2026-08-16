use chrono::{DateTime, Utc};
use prometheus_client::metrics::counter::Counter;
use prometheus_client::registry::Registry;
use serde::Serialize;
use tokio::sync::Mutex;

#[derive(Default)]
pub struct WorkflowObservability {
    produced_file_uploads_total: Counter,
    produced_file_upload_failures_total: Counter,
    event_delivery_attempts_total: Counter,
    event_delivery_successes_total: Counter,
    event_delivery_retries_total: Counter,
    event_delivery_failures_total: Counter,
    retention_passes_total: Counter,
    log_retention_candidates_total: Counter,
    output_retention_candidates_total: Counter,
    retention_deleted_logs_total: Counter,
    retention_cleared_outputs_total: Counter,
    retention_failures_total: Counter,
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
    pub fn register_metrics(&self, registry: &mut Registry) {
        registry.register(
            "browserpane_gateway_workflow_produced_file_uploads",
            "Successfully stored workflow produced files",
            self.produced_file_uploads_total.clone(),
        );
        registry.register(
            "browserpane_gateway_workflow_produced_file_upload_failures",
            "Failed workflow produced-file storage operations",
            self.produced_file_upload_failures_total.clone(),
        );
        registry.register(
            "browserpane_gateway_workflow_event_delivery_attempts",
            "Workflow event delivery attempts",
            self.event_delivery_attempts_total.clone(),
        );
        registry.register(
            "browserpane_gateway_workflow_event_delivery_successes",
            "Successful workflow event deliveries",
            self.event_delivery_successes_total.clone(),
        );
        registry.register(
            "browserpane_gateway_workflow_event_delivery_retries",
            "Scheduled workflow event delivery retries",
            self.event_delivery_retries_total.clone(),
        );
        registry.register(
            "browserpane_gateway_workflow_event_delivery_failures",
            "Terminal workflow event delivery failures",
            self.event_delivery_failures_total.clone(),
        );
        registry.register(
            "browserpane_gateway_workflow_retention_passes",
            "Completed workflow retention passes",
            self.retention_passes_total.clone(),
        );
        registry.register(
            "browserpane_gateway_workflow_retention_log_candidates",
            "Workflow log entries selected as retention candidates",
            self.log_retention_candidates_total.clone(),
        );
        registry.register(
            "browserpane_gateway_workflow_retention_output_candidates",
            "Workflow outputs selected as retention candidates",
            self.output_retention_candidates_total.clone(),
        );
        registry.register(
            "browserpane_gateway_workflow_retention_deleted_logs",
            "Workflow log entries deleted by retention",
            self.retention_deleted_logs_total.clone(),
        );
        registry.register(
            "browserpane_gateway_workflow_retention_cleared_outputs",
            "Workflow outputs cleared by retention",
            self.retention_cleared_outputs_total.clone(),
        );
        registry.register(
            "browserpane_gateway_workflow_retention_failures",
            "Workflow retention operation failures",
            self.retention_failures_total.clone(),
        );
    }

    pub fn record_produced_file_upload(&self) {
        self.produced_file_uploads_total.inc();
    }

    pub fn record_produced_file_upload_failure(&self) {
        self.produced_file_upload_failures_total.inc();
    }

    pub fn record_event_delivery_attempt(&self) {
        self.event_delivery_attempts_total.inc();
    }

    pub async fn record_event_delivery_success(&self, at: DateTime<Utc>) {
        self.event_delivery_successes_total.inc();
        *self.last_event_delivery_at.lock().await = Some(at);
    }

    pub fn record_event_delivery_retry(&self) {
        self.event_delivery_retries_total.inc();
    }

    pub fn record_event_delivery_failure(&self) {
        self.event_delivery_failures_total.inc();
    }

    pub async fn record_retention_pass(
        &self,
        at: DateTime<Utc>,
        log_candidate_count: usize,
        output_candidate_count: usize,
    ) {
        self.retention_passes_total.inc();
        self.log_retention_candidates_total
            .inc_by(log_candidate_count as u64);
        self.output_retention_candidates_total
            .inc_by(output_candidate_count as u64);
        *self.last_retention_pass_at.lock().await = Some(at);
    }

    pub fn record_retention_deleted_logs(&self, deleted: usize) {
        self.retention_deleted_logs_total.inc_by(deleted as u64);
    }

    pub fn record_retention_cleared_output(&self) {
        self.retention_cleared_outputs_total.inc();
    }

    pub fn record_retention_failure(&self) {
        self.retention_failures_total.inc();
    }

    pub async fn snapshot(&self) -> WorkflowObservabilitySnapshot {
        WorkflowObservabilitySnapshot {
            produced_file_uploads_total: self.produced_file_uploads_total.get(),
            produced_file_upload_failures_total: self.produced_file_upload_failures_total.get(),
            event_delivery_attempts_total: self.event_delivery_attempts_total.get(),
            event_delivery_successes_total: self.event_delivery_successes_total.get(),
            event_delivery_retries_total: self.event_delivery_retries_total.get(),
            event_delivery_failures_total: self.event_delivery_failures_total.get(),
            retention_passes_total: self.retention_passes_total.get(),
            log_retention_candidates_total: self.log_retention_candidates_total.get(),
            output_retention_candidates_total: self.output_retention_candidates_total.get(),
            retention_deleted_logs_total: self.retention_deleted_logs_total.get(),
            retention_cleared_outputs_total: self.retention_cleared_outputs_total.get(),
            retention_failures_total: self.retention_failures_total.get(),
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
