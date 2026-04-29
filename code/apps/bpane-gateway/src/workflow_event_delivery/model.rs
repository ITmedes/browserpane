use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

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
