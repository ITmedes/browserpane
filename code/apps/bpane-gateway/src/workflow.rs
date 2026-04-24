use std::collections::HashMap;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::automation_task::{
    AutomationTaskLogStream, AutomationTaskState, StoredAutomationTask, StoredAutomationTaskEvent,
    StoredAutomationTaskLog,
};

#[derive(Debug, Clone)]
pub struct PersistWorkflowDefinitionRequest {
    pub name: String,
    pub description: Option<String>,
    pub labels: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct PersistWorkflowDefinitionVersionRequest {
    pub workflow_definition_id: Uuid,
    pub version: String,
    pub executor: String,
    pub entrypoint: String,
    pub input_schema: Option<Value>,
    pub output_schema: Option<Value>,
    pub default_session: Option<Value>,
    pub allowed_credential_binding_ids: Vec<String>,
    pub allowed_extension_ids: Vec<String>,
    pub allowed_file_workspace_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PersistWorkflowRunRequest {
    pub workflow_definition_id: Uuid,
    pub workflow_definition_version_id: Uuid,
    pub workflow_version: String,
    pub session_id: Uuid,
    pub automation_task_id: Uuid,
    pub input: Option<Value>,
    pub labels: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct PersistWorkflowRunEventRequest {
    pub event_type: String,
    pub message: String,
    pub data: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct StoredWorkflowDefinition {
    pub id: Uuid,
    pub owner_subject: String,
    pub owner_issuer: String,
    pub name: String,
    pub description: Option<String>,
    pub labels: HashMap<String, String>,
    pub latest_version: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct StoredWorkflowDefinitionVersion {
    pub id: Uuid,
    pub workflow_definition_id: Uuid,
    pub version: String,
    pub executor: String,
    pub entrypoint: String,
    pub input_schema: Option<Value>,
    pub output_schema: Option<Value>,
    pub default_session: Option<Value>,
    pub allowed_credential_binding_ids: Vec<String>,
    pub allowed_extension_ids: Vec<String>,
    pub allowed_file_workspace_ids: Vec<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct StoredWorkflowRun {
    pub id: Uuid,
    pub workflow_definition_id: Uuid,
    pub workflow_definition_version_id: Uuid,
    pub workflow_version: String,
    pub session_id: Uuid,
    pub automation_task_id: Uuid,
    pub input: Option<Value>,
    pub labels: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct StoredWorkflowRunEvent {
    pub id: Uuid,
    pub run_id: Uuid,
    pub event_type: String,
    pub message: String,
    pub data: Option<Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRunState {
    Pending,
    Starting,
    Running,
    AwaitingInput,
    Succeeded,
    Failed,
    Cancelled,
    TimedOut,
}

impl From<AutomationTaskState> for WorkflowRunState {
    fn from(value: AutomationTaskState) -> Self {
        match value {
            AutomationTaskState::Pending => Self::Pending,
            AutomationTaskState::Starting => Self::Starting,
            AutomationTaskState::Running => Self::Running,
            AutomationTaskState::AwaitingInput => Self::AwaitingInput,
            AutomationTaskState::Succeeded => Self::Succeeded,
            AutomationTaskState::Failed => Self::Failed,
            AutomationTaskState::Cancelled => Self::Cancelled,
            AutomationTaskState::TimedOut => Self::TimedOut,
        }
    }
}

impl FromStr for WorkflowRunState {
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
            _ => Err("unknown workflow run state"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRunEventSource {
    Run,
    AutomationTask,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRunLogSource {
    AutomationTask,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkflowDefinitionResource {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub labels: HashMap<String, String>,
    pub latest_version: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct WorkflowDefinitionListResponse {
    pub workflows: Vec<WorkflowDefinitionResource>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkflowDefinitionVersionResource {
    pub id: Uuid,
    pub workflow_definition_id: Uuid,
    pub version: String,
    pub executor: String,
    pub entrypoint: String,
    pub input_schema: Option<Value>,
    pub output_schema: Option<Value>,
    pub default_session: Option<Value>,
    pub allowed_credential_binding_ids: Vec<String>,
    pub allowed_extension_ids: Vec<String>,
    pub allowed_file_workspace_ids: Vec<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkflowRunResource {
    pub id: Uuid,
    pub workflow_definition_id: Uuid,
    pub workflow_definition_version_id: Uuid,
    pub workflow_version: String,
    pub state: WorkflowRunState,
    pub session_id: Uuid,
    pub automation_task_id: Uuid,
    pub input: Option<Value>,
    pub output: Option<Value>,
    pub error: Option<String>,
    pub artifact_refs: Vec<String>,
    pub labels: HashMap<String, String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub events_path: String,
    pub logs_path: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkflowRunEventResource {
    pub id: Uuid,
    pub run_id: Uuid,
    pub source: WorkflowRunEventSource,
    pub automation_task_id: Option<Uuid>,
    pub event_type: String,
    pub message: String,
    pub data: Option<Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct WorkflowRunEventListResponse {
    pub events: Vec<WorkflowRunEventResource>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkflowRunLogResource {
    pub id: Uuid,
    pub run_id: Uuid,
    pub source: WorkflowRunLogSource,
    pub automation_task_id: Option<Uuid>,
    pub stream: AutomationTaskLogStream,
    pub message: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct WorkflowRunLogListResponse {
    pub logs: Vec<WorkflowRunLogResource>,
}

impl StoredWorkflowDefinition {
    pub fn to_resource(&self) -> WorkflowDefinitionResource {
        WorkflowDefinitionResource {
            id: self.id,
            name: self.name.clone(),
            description: self.description.clone(),
            labels: self.labels.clone(),
            latest_version: self.latest_version.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

impl StoredWorkflowDefinitionVersion {
    pub fn to_resource(&self) -> WorkflowDefinitionVersionResource {
        WorkflowDefinitionVersionResource {
            id: self.id,
            workflow_definition_id: self.workflow_definition_id,
            version: self.version.clone(),
            executor: self.executor.clone(),
            entrypoint: self.entrypoint.clone(),
            input_schema: self.input_schema.clone(),
            output_schema: self.output_schema.clone(),
            default_session: self.default_session.clone(),
            allowed_credential_binding_ids: self.allowed_credential_binding_ids.clone(),
            allowed_extension_ids: self.allowed_extension_ids.clone(),
            allowed_file_workspace_ids: self.allowed_file_workspace_ids.clone(),
            created_at: self.created_at,
        }
    }
}

impl StoredWorkflowRun {
    pub fn to_resource(&self, task: &StoredAutomationTask) -> WorkflowRunResource {
        WorkflowRunResource {
            id: self.id,
            workflow_definition_id: self.workflow_definition_id,
            workflow_definition_version_id: self.workflow_definition_version_id,
            workflow_version: self.workflow_version.clone(),
            state: WorkflowRunState::from(task.state),
            session_id: self.session_id,
            automation_task_id: self.automation_task_id,
            input: self.input.clone(),
            output: task.output.clone(),
            error: task.error.clone(),
            artifact_refs: task.artifact_refs.clone(),
            labels: self.labels.clone(),
            started_at: task.started_at,
            completed_at: task.completed_at,
            events_path: format!("/api/v1/workflow-runs/{}/events", self.id),
            logs_path: format!("/api/v1/workflow-runs/{}/logs", self.id),
            created_at: self.created_at,
            updated_at: std::cmp::max(self.updated_at, task.updated_at),
        }
    }
}

impl StoredWorkflowRunEvent {
    pub fn to_resource(&self) -> WorkflowRunEventResource {
        WorkflowRunEventResource {
            id: self.id,
            run_id: self.run_id,
            source: WorkflowRunEventSource::Run,
            automation_task_id: None,
            event_type: self.event_type.clone(),
            message: self.message.clone(),
            data: self.data.clone(),
            created_at: self.created_at,
        }
    }
}

impl WorkflowRunEventResource {
    pub fn from_automation_task(
        run_id: Uuid,
        task_id: Uuid,
        event: &StoredAutomationTaskEvent,
    ) -> Self {
        Self {
            id: event.id,
            run_id,
            source: WorkflowRunEventSource::AutomationTask,
            automation_task_id: Some(task_id),
            event_type: event.event_type.clone(),
            message: event.message.clone(),
            data: event.data.clone(),
            created_at: event.created_at,
        }
    }
}

impl WorkflowRunLogResource {
    pub fn from_automation_task(
        run_id: Uuid,
        task_id: Uuid,
        log: &StoredAutomationTaskLog,
    ) -> Self {
        Self {
            id: log.id,
            run_id,
            source: WorkflowRunLogSource::AutomationTask,
            automation_task_id: Some(task_id),
            stream: log.stream,
            message: log.message.clone(),
            created_at: log.created_at,
        }
    }
}
