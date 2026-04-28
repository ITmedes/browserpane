use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Sha256;
use tokio::time::sleep;
use tracing::warn;
use uuid::Uuid;

use crate::session_control::{SessionStore, SessionStoreError};
use crate::workflow::{StoredWorkflowRun, StoredWorkflowRunEvent};
use crate::workflow_observability::WorkflowObservability;

type HmacSha256 = Hmac<Sha256>;

const SIGNATURE_SCHEME: &str = "v1";

#[derive(Debug, Clone)]
pub struct PersistWorkflowEventSubscriptionRequest {
    pub name: String,
    pub target_url: String,
    pub event_types: Vec<String>,
    pub signing_secret: String,
}

#[derive(Debug, Clone)]
pub struct StoredWorkflowEventSubscription {
    pub id: Uuid,
    pub owner_subject: String,
    pub owner_issuer: String,
    pub name: String,
    pub target_url: String,
    pub event_types: Vec<String>,
    pub signing_secret: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowEventDeliveryState {
    Pending,
    Delivering,
    Delivered,
    Failed,
}

impl WorkflowEventDeliveryState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Delivering => "delivering",
            Self::Delivered => "delivered",
            Self::Failed => "failed",
        }
    }
}

impl std::str::FromStr for WorkflowEventDeliveryState {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "pending" => Ok(Self::Pending),
            "delivering" => Ok(Self::Delivering),
            "delivered" => Ok(Self::Delivered),
            "failed" => Ok(Self::Failed),
            _ => Err("unknown workflow event delivery state"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct StoredWorkflowEventDelivery {
    pub id: Uuid,
    pub subscription_id: Uuid,
    pub run_id: Uuid,
    pub event_id: Uuid,
    pub event_type: String,
    pub target_url: String,
    pub signing_secret: String,
    pub payload: Value,
    pub state: WorkflowEventDeliveryState,
    pub attempt_count: u32,
    pub next_attempt_at: Option<DateTime<Utc>>,
    pub last_attempt_at: Option<DateTime<Utc>>,
    pub delivered_at: Option<DateTime<Utc>>,
    pub last_response_status: Option<u16>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct StoredWorkflowEventDeliveryAttempt {
    pub id: Uuid,
    pub delivery_id: Uuid,
    pub attempt_number: u32,
    pub response_status: Option<u16>,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct RecordWorkflowEventDeliveryAttemptRequest {
    pub attempt_number: u32,
    pub response_status: Option<u16>,
    pub error: Option<String>,
    pub attempted_at: DateTime<Utc>,
    pub next_attempt_at: Option<DateTime<Utc>>,
    pub delivered_at: Option<DateTime<Utc>>,
    pub state: WorkflowEventDeliveryState,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkflowEventSubscriptionResource {
    pub id: Uuid,
    pub name: String,
    pub target_url: String,
    pub event_types: Vec<String>,
    pub has_signing_secret: bool,
    pub deliveries_path: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkflowEventDeliveryAttemptResource {
    pub id: Uuid,
    pub delivery_id: Uuid,
    pub attempt_number: u32,
    pub response_status: Option<u16>,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkflowEventDeliveryResource {
    pub id: Uuid,
    pub subscription_id: Uuid,
    pub run_id: Uuid,
    pub event_id: Uuid,
    pub event_type: String,
    pub state: WorkflowEventDeliveryState,
    pub attempt_count: u32,
    pub next_attempt_at: Option<DateTime<Utc>>,
    pub last_attempt_at: Option<DateTime<Utc>>,
    pub delivered_at: Option<DateTime<Utc>>,
    pub last_response_status: Option<u16>,
    pub last_error: Option<String>,
    pub payload: Value,
    pub attempts: Vec<WorkflowEventDeliveryAttemptResource>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkflowEventSubscriptionListResponse {
    pub subscriptions: Vec<WorkflowEventSubscriptionResource>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkflowEventDeliveryListResponse {
    pub deliveries: Vec<WorkflowEventDeliveryResource>,
}

impl StoredWorkflowEventSubscription {
    pub fn to_resource(&self) -> WorkflowEventSubscriptionResource {
        WorkflowEventSubscriptionResource {
            id: self.id,
            name: self.name.clone(),
            target_url: self.target_url.clone(),
            event_types: self.event_types.clone(),
            has_signing_secret: !self.signing_secret.is_empty(),
            deliveries_path: format!(
                "/api/v1/workflow-event-subscriptions/{}/deliveries",
                self.id
            ),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

impl StoredWorkflowEventDeliveryAttempt {
    pub fn to_resource(&self) -> WorkflowEventDeliveryAttemptResource {
        WorkflowEventDeliveryAttemptResource {
            id: self.id,
            delivery_id: self.delivery_id,
            attempt_number: self.attempt_number,
            response_status: self.response_status,
            error: self.error.clone(),
            created_at: self.created_at,
        }
    }
}

impl StoredWorkflowEventDelivery {
    pub fn to_resource(
        &self,
        attempts: Vec<WorkflowEventDeliveryAttemptResource>,
    ) -> WorkflowEventDeliveryResource {
        WorkflowEventDeliveryResource {
            id: self.id,
            subscription_id: self.subscription_id,
            run_id: self.run_id,
            event_id: self.event_id,
            event_type: self.event_type.clone(),
            state: self.state,
            attempt_count: self.attempt_count,
            next_attempt_at: self.next_attempt_at,
            last_attempt_at: self.last_attempt_at,
            delivered_at: self.delivered_at,
            last_response_status: self.last_response_status,
            last_error: self.last_error.clone(),
            payload: self.payload.clone(),
            attempts,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

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
    client: Client,
    config: WorkflowEventDeliveryConfig,
}

impl WorkflowEventDeliveryManager {
    pub fn new(
        session_store: SessionStore,
        observability: Arc<WorkflowObservability>,
        config: WorkflowEventDeliveryConfig,
    ) -> Result<Self, SessionStoreError> {
        let client = Client::builder()
            .timeout(config.request_timeout)
            .build()
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to build workflow event delivery HTTP client: {error}"
                ))
            })?;
        Ok(Self {
            session_store,
            observability,
            client,
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
        let deliveries = self
            .session_store
            .claim_due_workflow_event_deliveries(self.config.batch_size, Utc::now())
            .await?;
        for delivery in deliveries {
            self.deliver(delivery).await?;
        }
        Ok(())
    }

    async fn deliver(&self, delivery: StoredWorkflowEventDelivery) -> Result<(), SessionStoreError> {
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

        let result = self
            .client
            .post(&delivery.target_url)
            .header("content-type", "application/json")
            .header("x-bpane-event-id", delivery.event_id.to_string())
            .header("x-bpane-event-type", delivery.event_type.as_str())
            .header("x-bpane-delivery-id", delivery.id.to_string())
            .header("x-bpane-subscription-id", delivery.subscription_id.to_string())
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

pub fn workflow_event_type_matches(subscription_event_types: &[String], event_type: &str) -> bool {
    subscription_event_types.iter().any(|candidate| {
        if let Some(prefix) = candidate.strip_suffix(".*") {
            event_type.starts_with(prefix) && event_type[prefix.len()..].starts_with('.')
        } else {
            candidate == event_type
        }
    })
}

pub fn build_workflow_event_delivery_payload(
    subscription_id: Uuid,
    delivery_id: Uuid,
    run: &StoredWorkflowRun,
    event: &StoredWorkflowRunEvent,
) -> Value {
    serde_json::json!({
        "subscription_id": subscription_id,
        "delivery_id": delivery_id,
        "event_id": event.id,
        "run_id": run.id,
        "session_id": run.session_id,
        "automation_task_id": run.automation_task_id,
        "workflow_definition_id": run.workflow_definition_id,
        "workflow_definition_version_id": run.workflow_definition_version_id,
        "workflow_version": run.workflow_version,
        "run_state": run.state.as_str(),
        "source_system": run.source_system,
        "source_reference": run.source_reference,
        "client_request_id": run.client_request_id,
        "event_type": event.event_type,
        "message": event.message,
        "data": event.data,
        "created_at": event.created_at,
    })
}

pub fn validate_workflow_event_subscription_request(
    request: &PersistWorkflowEventSubscriptionRequest,
) -> Result<(), SessionStoreError> {
    if request.name.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "workflow event subscription name must not be empty".to_string(),
        ));
    }
    if !(request.target_url.starts_with("http://") || request.target_url.starts_with("https://")) {
        return Err(SessionStoreError::InvalidRequest(
            "workflow event subscription target_url must be http or https".to_string(),
        ));
    }
    if request.signing_secret.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "workflow event subscription signing_secret must not be empty".to_string(),
        ));
    }
    if request.event_types.is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "workflow event subscription event_types must not be empty".to_string(),
        ));
    }
    for event_type in &request.event_types {
        let trimmed = event_type.trim();
        if trimmed.is_empty() || trimmed.contains(' ') {
            return Err(SessionStoreError::InvalidRequest(format!(
                "workflow event subscription event type {event_type:?} is invalid"
            )));
        }
    }
    Ok(())
}

pub fn sign_workflow_event_delivery(
    signing_secret: &str,
    timestamp: &str,
    body: &[u8],
) -> Result<String, SessionStoreError> {
    let mut mac = HmacSha256::new_from_slice(signing_secret.as_bytes()).map_err(|error| {
        SessionStoreError::Backend(format!(
            "failed to initialize workflow event delivery HMAC signer: {error}"
        ))
    })?;
    mac.update(timestamp.as_bytes());
    mac.update(b".");
    mac.update(body);
    let signature = mac.finalize().into_bytes();
    Ok(format!("{SIGNATURE_SCHEME}={}", hex::encode(signature)))
}

pub fn backoff_for_attempt(base_backoff: Duration, attempt_number: u32) -> Duration {
    let exponent = attempt_number.saturating_sub(1).min(8);
    let multiplier = 1u32 << exponent;
    let backoff = base_backoff.saturating_mul(multiplier);
    std::cmp::min(backoff, Duration::from_secs(300))
}

fn should_retry_http_status(status: u16) -> bool {
    status == 429 || (500..=599).contains(&status)
}

pub fn group_attempts_by_delivery(
    attempts: Vec<StoredWorkflowEventDeliveryAttempt>,
) -> HashMap<Uuid, Vec<WorkflowEventDeliveryAttemptResource>> {
    let mut grouped = HashMap::<Uuid, Vec<WorkflowEventDeliveryAttemptResource>>::new();
    for attempt in attempts {
        grouped
            .entry(attempt.delivery_id)
            .or_default()
            .push(attempt.to_resource());
    }
    for attempts in grouped.values_mut() {
        attempts.sort_by(|left, right| {
            left.attempt_number
                .cmp(&right.attempt_number)
                .then_with(|| left.created_at.cmp(&right.created_at))
        });
    }
    grouped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_matching_supports_exact_and_family_patterns() {
        assert!(workflow_event_type_matches(
            &["workflow_run.created".to_string()],
            "workflow_run.created"
        ));
        assert!(!workflow_event_type_matches(
            &["workflow_run.created".to_string()],
            "workflow_run.succeeded"
        ));
        assert!(workflow_event_type_matches(
            &["workflow_run.*".to_string()],
            "workflow_run.awaiting_input"
        ));
        assert!(!workflow_event_type_matches(
            &["workflow_run.*".to_string()],
            "automation_task.running"
        ));
    }

    #[test]
    fn signature_is_stable_for_timestamp_and_body() {
        let signature = sign_workflow_event_delivery("secret", "1714235400", br#"{"ok":true}"#)
            .expect("signature should be created");
        assert_eq!(
            signature,
            "v1=0604501d383ecf7376c043b341360645fc547f7977af54e29fcef2cc4e249289"
        );
    }

    #[test]
    fn backoff_grows_and_caps() {
        assert_eq!(backoff_for_attempt(Duration::from_secs(2), 1), Duration::from_secs(2));
        assert_eq!(backoff_for_attempt(Duration::from_secs(2), 2), Duration::from_secs(4));
        assert_eq!(backoff_for_attempt(Duration::from_secs(2), 4), Duration::from_secs(16));
        assert_eq!(
            backoff_for_attempt(Duration::from_secs(120), 8),
            Duration::from_secs(300)
        );
    }
}
