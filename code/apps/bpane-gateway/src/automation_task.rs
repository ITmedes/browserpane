use std::collections::HashMap;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomationTaskState {
    Pending,
    Starting,
    Running,
    AwaitingInput,
    Succeeded,
    Failed,
    Cancelled,
    TimedOut,
}

impl AutomationTaskState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::AwaitingInput => "awaiting_input",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::TimedOut => "timed_out",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Cancelled | Self::TimedOut
        )
    }

    pub fn can_transition_to(self, next: Self) -> bool {
        match (self, next) {
            (current, target) if current == target => true,
            (Self::Pending, Self::Starting | Self::Running | Self::Cancelled | Self::Failed)
            | (
                Self::Starting,
                Self::Running
                | Self::AwaitingInput
                | Self::Cancelled
                | Self::Failed
                | Self::TimedOut,
            )
            | (
                Self::Running,
                Self::AwaitingInput
                | Self::Succeeded
                | Self::Cancelled
                | Self::Failed
                | Self::TimedOut,
            )
            | (
                Self::AwaitingInput,
                Self::Running | Self::Cancelled | Self::Failed | Self::TimedOut,
            ) => true,
            _ => false,
        }
    }
}

impl FromStr for AutomationTaskState {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "pending" => Ok(Self::Pending),
            "starting" => Ok(Self::Starting),
            "running" => Ok(Self::Running),
            "awaiting_input" => Ok(Self::AwaitingInput),
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            "timed_out" => Ok(Self::TimedOut),
            _ => Err("unknown automation task state"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomationTaskSessionSource {
    ExistingSession,
    CreatedSession,
}

impl AutomationTaskSessionSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ExistingSession => "existing_session",
            Self::CreatedSession => "created_session",
        }
    }
}

impl FromStr for AutomationTaskSessionSource {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "existing_session" => Ok(Self::ExistingSession),
            "created_session" => Ok(Self::CreatedSession),
            _ => Err("unknown automation task session source"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomationTaskLogStream {
    Stdout,
    Stderr,
    System,
}

impl AutomationTaskLogStream {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
            Self::System => "system",
        }
    }
}

impl FromStr for AutomationTaskLogStream {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "stdout" => Ok(Self::Stdout),
            "stderr" => Ok(Self::Stderr),
            "system" => Ok(Self::System),
            _ => Err("unknown automation task log stream"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PersistAutomationTaskRequest {
    pub display_name: Option<String>,
    pub executor: String,
    pub session_id: Uuid,
    pub session_source: AutomationTaskSessionSource,
    pub input: Option<Value>,
    pub labels: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct AutomationTaskTransitionRequest {
    pub state: AutomationTaskState,
    pub output: Option<Value>,
    pub error: Option<String>,
    pub artifact_refs: Vec<String>,
    pub event_type: String,
    pub event_message: String,
    pub event_data: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct StoredAutomationTask {
    pub id: Uuid,
    pub display_name: Option<String>,
    pub executor: String,
    pub state: AutomationTaskState,
    pub session_id: Uuid,
    pub session_source: AutomationTaskSessionSource,
    pub input: Option<Value>,
    pub output: Option<Value>,
    pub error: Option<String>,
    pub artifact_refs: Vec<String>,
    pub labels: HashMap<String, String>,
    pub cancel_requested_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct StoredAutomationTaskEvent {
    pub id: Uuid,
    pub task_id: Uuid,
    pub event_type: String,
    pub message: String,
    pub data: Option<Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct StoredAutomationTaskLog {
    pub id: Uuid,
    pub task_id: Uuid,
    pub stream: AutomationTaskLogStream,
    pub message: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AutomationTaskSessionBinding {
    pub source: AutomationTaskSessionSource,
    pub session_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AutomationTaskResource {
    pub id: Uuid,
    pub display_name: Option<String>,
    pub executor: String,
    pub state: AutomationTaskState,
    pub session: AutomationTaskSessionBinding,
    pub input: Option<Value>,
    pub output: Option<Value>,
    pub error: Option<String>,
    pub artifact_refs: Vec<String>,
    pub labels: HashMap<String, String>,
    pub cancel_requested_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub logs_path: String,
    pub events_path: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct AutomationTaskListResponse {
    pub tasks: Vec<AutomationTaskResource>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AutomationTaskEventResource {
    pub id: Uuid,
    pub task_id: Uuid,
    pub event_type: String,
    pub message: String,
    pub data: Option<Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct AutomationTaskEventListResponse {
    pub events: Vec<AutomationTaskEventResource>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AutomationTaskLogLineResource {
    pub id: Uuid,
    pub task_id: Uuid,
    pub stream: AutomationTaskLogStream,
    pub message: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct AutomationTaskLogListResponse {
    pub logs: Vec<AutomationTaskLogLineResource>,
}

impl StoredAutomationTask {
    pub fn to_resource(&self) -> AutomationTaskResource {
        AutomationTaskResource {
            id: self.id,
            display_name: self.display_name.clone(),
            executor: self.executor.clone(),
            state: self.state,
            session: AutomationTaskSessionBinding {
                source: self.session_source,
                session_id: self.session_id,
            },
            input: self.input.clone(),
            output: self.output.clone(),
            error: self.error.clone(),
            artifact_refs: self.artifact_refs.clone(),
            labels: self.labels.clone(),
            cancel_requested_at: self.cancel_requested_at,
            started_at: self.started_at,
            completed_at: self.completed_at,
            logs_path: format!("/api/v1/automation-tasks/{}/logs", self.id),
            events_path: format!("/api/v1/automation-tasks/{}/events", self.id),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

impl StoredAutomationTaskEvent {
    pub fn to_resource(&self) -> AutomationTaskEventResource {
        AutomationTaskEventResource {
            id: self.id,
            task_id: self.task_id,
            event_type: self.event_type.clone(),
            message: self.message.clone(),
            data: self.data.clone(),
            created_at: self.created_at,
        }
    }
}

impl StoredAutomationTaskLog {
    pub fn to_resource(&self) -> AutomationTaskLogLineResource {
        AutomationTaskLogLineResource {
            id: self.id,
            task_id: self.task_id,
            stream: self.stream,
            message: self.message.clone(),
            created_at: self.created_at,
        }
    }
}
