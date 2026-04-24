use std::collections::HashMap;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::automation_task::{
    AutomationTaskLogStream, AutomationTaskState, StoredAutomationTaskEvent,
    StoredAutomationTaskLog,
};
use crate::credential_binding::{
    WorkflowRunCredentialBinding, WorkflowRunCredentialBindingResource,
};
use crate::extension::{AppliedExtension, AppliedExtensionResource};
use crate::workflow_source::WorkflowSource;

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
    pub source: Option<WorkflowSource>,
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
    pub source_snapshot: Option<WorkflowRunSourceSnapshot>,
    pub extensions: Vec<AppliedExtension>,
    pub credential_bindings: Vec<WorkflowRunCredentialBinding>,
    pub workspace_inputs: Vec<WorkflowRunWorkspaceInput>,
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
pub struct PersistWorkflowRunLogRequest {
    pub stream: AutomationTaskLogStream,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct WorkflowRunTransitionRequest {
    pub state: WorkflowRunState,
    pub output: Option<Value>,
    pub error: Option<String>,
    pub artifact_refs: Vec<String>,
    pub message: Option<String>,
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
    pub source: Option<WorkflowSource>,
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
    pub source_snapshot: Option<WorkflowRunSourceSnapshot>,
    pub extensions: Vec<AppliedExtension>,
    pub credential_bindings: Vec<WorkflowRunCredentialBinding>,
    pub workspace_inputs: Vec<WorkflowRunWorkspaceInput>,
    pub state: WorkflowRunState,
    pub input: Option<Value>,
    pub output: Option<Value>,
    pub error: Option<String>,
    pub artifact_refs: Vec<String>,
    pub labels: HashMap<String, String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
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

#[derive(Debug, Clone)]
pub struct StoredWorkflowRunLog {
    pub id: Uuid,
    pub run_id: Uuid,
    pub stream: AutomationTaskLogStream,
    pub message: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowRunSourceSnapshot {
    pub source: WorkflowSource,
    pub entrypoint: String,
    pub workspace_id: Uuid,
    pub file_id: Uuid,
    pub file_name: String,
    pub media_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowRunWorkspaceInput {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub file_id: Uuid,
    pub file_name: String,
    pub media_type: Option<String>,
    pub byte_count: u64,
    pub sha256_hex: String,
    pub provenance: Option<Value>,
    pub mount_path: String,
    pub artifact_ref: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkflowRunSourceSnapshotResource {
    pub source: WorkflowSource,
    pub entrypoint: String,
    pub workspace_id: Uuid,
    pub file_id: Uuid,
    pub file_name: String,
    pub media_type: Option<String>,
    pub content_path: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkflowRunWorkspaceInputResource {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub file_id: Uuid,
    pub file_name: String,
    pub media_type: Option<String>,
    pub byte_count: u64,
    pub sha256_hex: String,
    pub provenance: Option<Value>,
    pub mount_path: String,
    pub content_path: String,
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

impl From<WorkflowRunState> for AutomationTaskState {
    fn from(value: WorkflowRunState) -> Self {
        match value {
            WorkflowRunState::Pending => Self::Pending,
            WorkflowRunState::Starting => Self::Starting,
            WorkflowRunState::Running => Self::Running,
            WorkflowRunState::AwaitingInput => Self::AwaitingInput,
            WorkflowRunState::Succeeded => Self::Succeeded,
            WorkflowRunState::Failed => Self::Failed,
            WorkflowRunState::Cancelled => Self::Cancelled,
            WorkflowRunState::TimedOut => Self::TimedOut,
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

impl WorkflowRunState {
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
    Run,
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
    pub source: Option<WorkflowSource>,
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
    pub source_snapshot: Option<WorkflowRunSourceSnapshotResource>,
    pub extensions: Vec<AppliedExtensionResource>,
    pub credential_bindings: Vec<WorkflowRunCredentialBindingResource>,
    pub workspace_inputs: Vec<WorkflowRunWorkspaceInputResource>,
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
            source: self.source.clone(),
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
    pub fn to_resource(&self) -> WorkflowRunResource {
        WorkflowRunResource {
            id: self.id,
            workflow_definition_id: self.workflow_definition_id,
            workflow_definition_version_id: self.workflow_definition_version_id,
            workflow_version: self.workflow_version.clone(),
            state: self.state,
            session_id: self.session_id,
            automation_task_id: self.automation_task_id,
            input: self.input.clone(),
            output: self.output.clone(),
            error: self.error.clone(),
            artifact_refs: self.artifact_refs.clone(),
            source_snapshot: self
                .source_snapshot
                .as_ref()
                .map(|snapshot| snapshot.to_resource(self.id)),
            extensions: self
                .extensions
                .iter()
                .map(AppliedExtension::to_resource)
                .collect(),
            credential_bindings: self
                .credential_bindings
                .iter()
                .map(|binding| binding.to_resource(self.id))
                .collect(),
            workspace_inputs: self
                .workspace_inputs
                .iter()
                .map(|input| input.to_resource(self.id))
                .collect(),
            labels: self.labels.clone(),
            started_at: self.started_at,
            completed_at: self.completed_at,
            events_path: format!("/api/v1/workflow-runs/{}/events", self.id),
            logs_path: format!("/api/v1/workflow-runs/{}/logs", self.id),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

impl WorkflowRunSourceSnapshot {
    pub fn to_resource(&self, run_id: Uuid) -> WorkflowRunSourceSnapshotResource {
        WorkflowRunSourceSnapshotResource {
            source: self.source.clone(),
            entrypoint: self.entrypoint.clone(),
            workspace_id: self.workspace_id,
            file_id: self.file_id,
            file_name: self.file_name.clone(),
            media_type: self.media_type.clone(),
            content_path: format!("/api/v1/workflow-runs/{run_id}/source-snapshot/content"),
        }
    }
}

impl WorkflowRunWorkspaceInput {
    pub fn to_resource(&self, run_id: Uuid) -> WorkflowRunWorkspaceInputResource {
        WorkflowRunWorkspaceInputResource {
            id: self.id,
            workspace_id: self.workspace_id,
            file_id: self.file_id,
            file_name: self.file_name.clone(),
            media_type: self.media_type.clone(),
            byte_count: self.byte_count,
            sha256_hex: self.sha256_hex.clone(),
            provenance: self.provenance.clone(),
            mount_path: self.mount_path.clone(),
            content_path: format!(
                "/api/v1/workflow-runs/{run_id}/workspace-inputs/{}/content",
                self.id
            ),
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
    pub fn from_run(run_id: Uuid, log: &StoredWorkflowRunLog) -> Self {
        Self {
            id: log.id,
            run_id,
            source: WorkflowRunLogSource::Run,
            automation_task_id: None,
            stream: log.stream,
            message: log.message.clone(),
            created_at: log.created_at,
        }
    }

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

pub fn workflow_run_event_type(state: WorkflowRunState) -> &'static str {
    match state {
        WorkflowRunState::Pending => "workflow_run.pending",
        WorkflowRunState::Starting => "workflow_run.starting",
        WorkflowRunState::Running => "workflow_run.running",
        WorkflowRunState::AwaitingInput => "workflow_run.awaiting_input",
        WorkflowRunState::Succeeded => "workflow_run.succeeded",
        WorkflowRunState::Failed => "workflow_run.failed",
        WorkflowRunState::Cancelled => "workflow_run.cancelled",
        WorkflowRunState::TimedOut => "workflow_run.timed_out",
    }
}

pub fn workflow_run_default_message(state: WorkflowRunState) -> &'static str {
    match state {
        WorkflowRunState::Pending => "workflow run returned to pending state",
        WorkflowRunState::Starting => "workflow run started",
        WorkflowRunState::Running => "workflow run entered running state",
        WorkflowRunState::AwaitingInput => "workflow run is awaiting input",
        WorkflowRunState::Succeeded => "workflow run completed successfully",
        WorkflowRunState::Failed => "workflow run failed",
        WorkflowRunState::Cancelled => "workflow run cancelled",
        WorkflowRunState::TimedOut => "workflow run timed out",
    }
}

pub fn automation_task_event_type_for_run_state(state: WorkflowRunState) -> &'static str {
    match state {
        WorkflowRunState::Pending => "automation_task.pending",
        WorkflowRunState::Starting => "automation_task.starting",
        WorkflowRunState::Running => "automation_task.running",
        WorkflowRunState::AwaitingInput => "automation_task.awaiting_input",
        WorkflowRunState::Succeeded => "automation_task.succeeded",
        WorkflowRunState::Failed => "automation_task.failed",
        WorkflowRunState::Cancelled => "automation_task.cancelled",
        WorkflowRunState::TimedOut => "automation_task.timed_out",
    }
}

pub fn automation_task_default_message_for_run_state(state: WorkflowRunState) -> &'static str {
    match state {
        WorkflowRunState::Pending => "automation task returned to pending state",
        WorkflowRunState::Starting => "automation task started",
        WorkflowRunState::Running => "automation task entered running state",
        WorkflowRunState::AwaitingInput => "automation task is awaiting input",
        WorkflowRunState::Succeeded => "automation task completed successfully",
        WorkflowRunState::Failed => "automation task failed",
        WorkflowRunState::Cancelled => "automation task cancelled",
        WorkflowRunState::TimedOut => "automation task timed out",
    }
}
