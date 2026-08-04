use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use tokio::time::sleep;
use tracing::warn;

use crate::session_control::{SessionStore, SessionStoreError};
use crate::workflow::WorkflowObservability;

use super::helpers::{
    backoff_for_attempt, should_retry_http_status, sign_workflow_event_delivery,
    sort_workflow_event_deliveries,
};
use super::model::{
    RecordWorkflowEventDeliveryAttemptRequest, StoredWorkflowEventDelivery,
    WorkflowEventDeliveryState,
};
use super::WorkflowEventDestinationPolicy;

#[derive(Debug, Clone)]
pub struct WorkflowEventDeliveryConfig {
    pub poll_interval: Duration,
    pub request_timeout: Duration,
    pub max_attempts: u32,
    pub batch_size: usize,
    pub base_backoff: Duration,
}

#[derive(Clone)]
pub struct WorkflowEventDeliveryManager {
    session_store: SessionStore,
    observability: Arc<WorkflowObservability>,
    destination_policy: Arc<WorkflowEventDestinationPolicy>,
    config: WorkflowEventDeliveryConfig,
}

impl WorkflowEventDeliveryManager {
    pub fn new(
        session_store: SessionStore,
        observability: Arc<WorkflowObservability>,
        config: WorkflowEventDeliveryConfig,
        destination_policy: Arc<WorkflowEventDestinationPolicy>,
    ) -> Result<Self, SessionStoreError> {
        Ok(Self {
            session_store,
            observability,
            destination_policy,
            config,
        })
    }

    pub async fn reconcile_persisted_state(&self) -> Result<(), SessionStoreError> {
        self.session_store
            .requeue_inflight_workflow_event_deliveries()
            .await
    }

    pub fn start(self: Arc<Self>) {
        tokio::spawn(async move {
            loop {
                sleep(self.config.poll_interval).await;
                if let Err(error) = self.run_dispatch_pass().await {
                    warn!("workflow event delivery pass failed: {error}");
                }
            }
        });
    }

    pub async fn run_dispatch_pass(&self) -> Result<(), SessionStoreError> {
        let mut deliveries = self
            .session_store
            .claim_due_workflow_event_deliveries(self.config.batch_size, Utc::now())
            .await?;
        sort_workflow_event_deliveries(&mut deliveries);
        for delivery in deliveries {
            self.deliver(delivery).await?;
        }
        Ok(())
    }

    async fn deliver(
        &self,
        delivery: StoredWorkflowEventDelivery,
    ) -> Result<(), SessionStoreError> {
        let body = serde_json::to_vec(&delivery.payload).map_err(|error| {
            SessionStoreError::Backend(format!(
                "failed to serialize workflow event delivery payload {}: {error}",
                delivery.id
            ))
        })?;
        let timestamp = Utc::now().timestamp().to_string();
        let signature = sign_workflow_event_delivery(&delivery.signing_secret, &timestamp, &body)?;
        let attempt_number = delivery.attempt_count.saturating_add(1);

        self.observability.record_event_delivery_attempt();

        let destination = match self
            .destination_policy
            .authorize(&delivery.target_url)
            .await
        {
            Ok(destination) => destination,
            Err(error) => {
                self.record_failure(
                    delivery,
                    attempt_number,
                    Utc::now(),
                    None,
                    Some(error.to_string()),
                    false,
                )
                .await?;
                return Ok(());
            }
        };
        let client = match destination.build_client(self.config.request_timeout) {
            Ok(client) => client,
            Err(error) => {
                self.record_failure(
                    delivery,
                    attempt_number,
                    Utc::now(),
                    None,
                    Some(error.to_string()),
                    false,
                )
                .await?;
                return Ok(());
            }
        };

        let result = client
            .post(destination.canonical_url())
            .header("content-type", "application/json")
            .header("x-bpane-event-id", delivery.event_id.to_string())
            .header("x-bpane-event-type", delivery.event_type.as_str())
            .header("x-bpane-delivery-id", delivery.id.to_string())
            .header(
                "x-bpane-subscription-id",
                delivery.subscription_id.to_string(),
            )
            .header("x-bpane-signature-timestamp", &timestamp)
            .header("x-bpane-signature-v1", signature)
            .body(body)
            .send()
            .await;

        let attempted_at = Utc::now();
        match result {
            Ok(response) if response.status().is_success() => {
                self.session_store
                    .record_workflow_event_delivery_attempt(
                        delivery.id,
                        RecordWorkflowEventDeliveryAttemptRequest {
                            attempt_number,
                            response_status: Some(response.status().as_u16()),
                            error: None,
                            attempted_at,
                            next_attempt_at: None,
                            delivered_at: Some(attempted_at),
                            state: WorkflowEventDeliveryState::Delivered,
                        },
                    )
                    .await?;
                self.observability
                    .record_event_delivery_success(attempted_at)
                    .await;
            }
            Ok(response) => {
                let status = response.status().as_u16();
                self.record_failure(
                    delivery,
                    attempt_number,
                    attempted_at,
                    Some(status),
                    Some(format!("received HTTP {status}")),
                    should_retry_http_status(status),
                )
                .await?;
            }
            Err(error) => {
                let error = if error.is_timeout() {
                    "workflow event delivery request timed out"
                } else if error.is_connect() {
                    "workflow event delivery connection failed"
                } else {
                    "workflow event delivery request failed"
                };
                self.record_failure(
                    delivery,
                    attempt_number,
                    attempted_at,
                    None,
                    Some(error.to_string()),
                    true,
                )
                .await?;
            }
        }
        Ok(())
    }

    async fn record_failure(
        &self,
        delivery: StoredWorkflowEventDelivery,
        attempt_number: u32,
        attempted_at: DateTime<Utc>,
        response_status: Option<u16>,
        error: Option<String>,
        retryable: bool,
    ) -> Result<(), SessionStoreError> {
        let retry_allowed = retryable && attempt_number < self.config.max_attempts;
        let next_attempt_at = retry_allowed.then(|| {
            attempted_at
                + chrono::Duration::from_std(backoff_for_attempt(
                    self.config.base_backoff,
                    attempt_number,
                ))
                .unwrap_or_else(|_| chrono::Duration::seconds(300))
        });
        let state = if retry_allowed {
            WorkflowEventDeliveryState::Pending
        } else {
            WorkflowEventDeliveryState::Failed
        };
        self.session_store
            .record_workflow_event_delivery_attempt(
                delivery.id,
                RecordWorkflowEventDeliveryAttemptRequest {
                    attempt_number,
                    response_status,
                    error: error.clone(),
                    attempted_at,
                    next_attempt_at,
                    delivered_at: None,
                    state,
                },
            )
            .await?;
        if retry_allowed {
            self.observability.record_event_delivery_retry();
        } else {
            self.observability.record_event_delivery_failure();
        }
        Ok(())
    }
}
