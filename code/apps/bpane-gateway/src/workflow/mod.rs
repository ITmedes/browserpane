pub mod observability;
pub mod retention;
pub mod source;

pub use observability::{WorkflowObservability, WorkflowObservabilitySnapshot};
pub use retention::WorkflowRetentionManager;
pub use source::{
    validate_workflow_source_entrypoint, WorkflowSource, WorkflowSourceArchive,
    WorkflowSourceError, WorkflowSourceResolver,
};

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
use crate::credentials::{WorkflowRunCredentialBinding, WorkflowRunCredentialBindingResource};
use crate::extensions::{AppliedExtension, AppliedExtensionResource};
use crate::session_control::SessionLifecycleState;

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
    pub source_system: Option<String>,
    pub source_reference: Option<String>,
    pub client_request_id: Option<String>,
    pub create_request_fingerprint: Option<String>,
    pub source_snapshot: Option<WorkflowRunSourceSnapshot>,
    pub extensions: Vec<AppliedExtension>,
    pub credential_bindings: Vec<WorkflowRunCredentialBinding>,
    pub workspace_inputs: Vec<WorkflowRunWorkspaceInput>,
    pub input: Option<Value>,
    pub labels: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct PersistWorkflowRunProducedFileRequest {
    pub workspace_id: Uuid,
    pub file_id: Uuid,
    pub file_name: String,
    pub media_type: Option<String>,
    pub byte_count: u64,
    pub sha256_hex: String,
    pub provenance: Option<Value>,
    pub artifact_ref: String,
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
    pub owner_subject: String,
    pub owner_issuer: String,
    pub workflow_definition_id: Uuid,
    pub workflow_definition_version_id: Uuid,
    pub workflow_version: String,
    pub session_id: Uuid,
    pub automation_task_id: Uuid,
    pub source_system: Option<String>,
    pub source_reference: Option<String>,
    pub client_request_id: Option<String>,
    pub create_request_fingerprint: Option<String>,
    pub source_snapshot: Option<WorkflowRunSourceSnapshot>,
    pub extensions: Vec<AppliedExtension>,
    pub credential_bindings: Vec<WorkflowRunCredentialBinding>,
    pub workspace_inputs: Vec<WorkflowRunWorkspaceInput>,
    pub produced_files: Vec<WorkflowRunProducedFile>,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowRunProducedFile {
    pub workspace_id: Uuid,
    pub file_id: Uuid,
    pub file_name: String,
    pub media_type: Option<String>,
    pub byte_count: u64,
    pub sha256_hex: String,
    pub provenance: Option<Value>,
    pub artifact_ref: String,
    pub created_at: DateTime<Utc>,
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

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkflowRunProducedFileResource {
    pub workspace_id: Uuid,
    pub file_id: Uuid,
    pub file_name: String,
    pub media_type: Option<String>,
    pub byte_count: u64,
    pub sha256_hex: String,
    pub provenance: Option<Value>,
    pub content_path: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkflowRunRecordingResource {
    pub id: Uuid,
    pub session_id: Uuid,
    pub state: String,
    pub format: String,
    pub mime_type: Option<String>,
    pub bytes: Option<u64>,
    pub duration_ms: Option<u64>,
    pub error: Option<String>,
    pub termination_reason: Option<String>,
    pub previous_recording_id: Option<Uuid>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub content_path: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkflowRunRetentionResource {
    pub logs_expire_at: Option<DateTime<Utc>>,
    pub output_expire_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowRunInterventionRequest {
    pub request_id: Uuid,
    pub kind: String,
    pub prompt: Option<String>,
    pub details: Option<Value>,
    pub requested_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRunInterventionAction {
    SubmitInput,
    Resume,
    Reject,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowRunInterventionResolution {
    pub request_id: Option<Uuid>,
    pub action: WorkflowRunInterventionAction,
    pub input: Option<Value>,
    pub reason: Option<String>,
    pub actor_subject: String,
    pub actor_issuer: String,
    pub actor_display_name: Option<String>,
    pub details: Option<Value>,
    pub resolved_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkflowRunInterventionResource {
    pub pending_request: Option<WorkflowRunInterventionRequest>,
    pub last_resolution: Option<WorkflowRunInterventionResolution>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRunAdmissionState {
    Queued,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowRunAdmissionResource {
    pub state: WorkflowRunAdmissionState,
    pub reason: String,
    pub details: Option<Value>,
    pub queued_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRunResumeMode {
    LiveRuntime,
    ProfileRestart,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkflowRunRuntimeResource {
    pub resume_mode: WorkflowRunResumeMode,
    pub exact_runtime_available: bool,
    pub hold_until: Option<DateTime<Utc>>,
    pub released_at: Option<DateTime<Utc>>,
    pub release_reason: Option<String>,
    pub session_state: Option<SessionLifecycleState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRunState {
    Pending,
    Queued,
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
            AutomationTaskState::Queued => Self::Queued,
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
            WorkflowRunState::Queued => Self::Queued,
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
            "queued" => Ok(Self::Queued),
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
            Self::Queued => "queued",
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
    pub source_system: Option<String>,
    pub source_reference: Option<String>,
    pub client_request_id: Option<String>,
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
    pub produced_files: Vec<WorkflowRunProducedFileResource>,
    pub recordings: Vec<WorkflowRunRecordingResource>,
    pub retention: WorkflowRunRetentionResource,
    pub admission: Option<WorkflowRunAdmissionResource>,
    pub intervention: WorkflowRunInterventionResource,
    pub runtime: Option<WorkflowRunRuntimeResource>,
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

#[derive(Debug, Clone)]
pub struct CreateWorkflowRunResult {
    pub run: StoredWorkflowRun,
    pub created: bool,
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
    pub fn to_resource(
        &self,
        recordings: Vec<WorkflowRunRecordingResource>,
        retention: WorkflowRunRetentionResource,
        admission: Option<WorkflowRunAdmissionResource>,
        intervention: WorkflowRunInterventionResource,
        runtime: Option<WorkflowRunRuntimeResource>,
    ) -> WorkflowRunResource {
        WorkflowRunResource {
            id: self.id,
            workflow_definition_id: self.workflow_definition_id,
            workflow_definition_version_id: self.workflow_definition_version_id,
            workflow_version: self.workflow_version.clone(),
            source_system: self.source_system.clone(),
            source_reference: self.source_reference.clone(),
            client_request_id: self.client_request_id.clone(),
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
            produced_files: self
                .produced_files
                .iter()
                .map(|file| file.to_resource(self.id))
                .collect(),
            recordings,
            retention,
            admission,
            intervention,
            runtime,
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

fn parse_intervention_request_value(
    value: &Value,
    fallback_request_id: Uuid,
    fallback_requested_at: DateTime<Utc>,
) -> Option<WorkflowRunInterventionRequest> {
    let object = value.as_object()?;
    let nested = object
        .get("intervention_request")
        .and_then(Value::as_object)
        .unwrap_or(object);
    let kind = nested
        .get("kind")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("generic_input")
        .to_string();
    let prompt = nested
        .get("prompt")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let request_id = nested
        .get("request_id")
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
        .unwrap_or(fallback_request_id);
    let details = nested
        .get("details")
        .cloned()
        .or_else(|| Some(value.clone()));
    Some(WorkflowRunInterventionRequest {
        request_id,
        kind,
        prompt,
        details,
        requested_at: fallback_requested_at,
    })
}

fn parse_intervention_resolution_value(
    value: &Value,
    fallback_resolved_at: DateTime<Utc>,
) -> Option<WorkflowRunInterventionResolution> {
    let object = value.as_object()?;
    let nested = object
        .get("intervention_resolution")
        .and_then(Value::as_object)
        .unwrap_or(object);
    let action = nested
        .get("action")
        .and_then(Value::as_str)
        .and_then(|value| match value {
            "submit_input" => Some(WorkflowRunInterventionAction::SubmitInput),
            "resume" => Some(WorkflowRunInterventionAction::Resume),
            "reject" => Some(WorkflowRunInterventionAction::Reject),
            _ => None,
        })?;
    let actor_subject = nested
        .get("actor_subject")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_string();
    let actor_issuer = nested
        .get("actor_issuer")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_string();
    let request_id = nested
        .get("request_id")
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok());
    let reason = nested
        .get("reason")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let actor_display_name = nested
        .get("actor_display_name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    Some(WorkflowRunInterventionResolution {
        request_id,
        action,
        input: nested.get("input").cloned(),
        reason,
        actor_subject,
        actor_issuer,
        actor_display_name,
        details: nested.get("details").cloned(),
        resolved_at: fallback_resolved_at,
    })
}

pub fn derive_workflow_run_intervention_resource(
    run_state: WorkflowRunState,
    events: &[WorkflowRunEventResource],
) -> WorkflowRunInterventionResource {
    let pending_request = if run_state == WorkflowRunState::AwaitingInput {
        events
            .iter()
            .rev()
            .find(|event| {
                event.event_type == "workflow_run.awaiting_input"
                    || event.event_type == "automation_task.awaiting_input"
            })
            .and_then(|event| {
                event
                    .data
                    .as_ref()
                    .and_then(|value| {
                        parse_intervention_request_value(value, event.id, event.created_at)
                    })
                    .or_else(|| {
                        Some(WorkflowRunInterventionRequest {
                            request_id: event.id,
                            kind: "generic_input".to_string(),
                            prompt: Some(event.message.clone()),
                            details: event.data.clone(),
                            requested_at: event.created_at,
                        })
                    })
            })
    } else {
        None
    };

    let last_resolution = events.iter().rev().find_map(|event| {
        if !matches!(
            event.event_type.as_str(),
            "workflow_run.input_submitted" | "workflow_run.resumed" | "workflow_run.rejected"
        ) {
            return None;
        }
        event
            .data
            .as_ref()
            .and_then(|value| parse_intervention_resolution_value(value, event.created_at))
    });

    WorkflowRunInterventionResource {
        pending_request,
        last_resolution,
    }
}

pub fn derive_workflow_run_admission_resource(
    run_state: WorkflowRunState,
    events: &[WorkflowRunEventResource],
) -> Option<WorkflowRunAdmissionResource> {
    if run_state != WorkflowRunState::Queued {
        return None;
    }

    let event = events.iter().rev().find(|event| {
        event.event_type == "workflow_run.queued" || event.event_type == "automation_task.queued"
    })?;
    let admission = event
        .data
        .as_ref()
        .and_then(Value::as_object)
        .and_then(|value| value.get("admission"))
        .or(event.data.as_ref());
    let reason = admission
        .and_then(Value::as_object)
        .and_then(|value| value.get("reason"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("workflow_worker_capacity")
        .to_string();
    let details = admission
        .and_then(Value::as_object)
        .and_then(|value| value.get("details"))
        .cloned()
        .or_else(|| event.data.clone());

    Some(WorkflowRunAdmissionResource {
        state: WorkflowRunAdmissionState::Queued,
        reason,
        details,
        queued_at: event.created_at,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowRunRuntimeHoldMode {
    Live,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowRunRuntimeHoldRequest {
    pub mode: WorkflowRunRuntimeHoldMode,
    pub timeout_sec: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowRunRuntimeRelease {
    pub reason: String,
    pub released_at: DateTime<Utc>,
}

pub fn parse_workflow_run_runtime_hold_request(
    value: &Value,
) -> Result<Option<WorkflowRunRuntimeHoldRequest>, &'static str> {
    let Some(object) = value.as_object() else {
        return Ok(None);
    };
    let Some(runtime_hold) = object.get("runtime_hold") else {
        return Ok(None);
    };
    let hold_object = runtime_hold
        .as_object()
        .ok_or("workflow runtime_hold must be a JSON object")?;
    let mode = hold_object
        .get("mode")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("live");
    let mode = match mode {
        "live" => WorkflowRunRuntimeHoldMode::Live,
        _ => return Err("workflow runtime_hold.mode must currently be \"live\""),
    };
    let timeout_sec = hold_object
        .get("timeout_sec")
        .and_then(Value::as_u64)
        .ok_or("workflow runtime_hold.timeout_sec must be a positive integer")?;
    if timeout_sec == 0 {
        return Err("workflow runtime_hold.timeout_sec must be greater than zero");
    }
    Ok(Some(WorkflowRunRuntimeHoldRequest { mode, timeout_sec }))
}

fn parse_workflow_run_runtime_release(
    value: &Value,
    fallback_released_at: DateTime<Utc>,
) -> Option<WorkflowRunRuntimeRelease> {
    let object = value.as_object()?;
    let nested = object
        .get("runtime_release")
        .and_then(Value::as_object)
        .unwrap_or(object);
    let reason = nested
        .get("reason")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("runtime_released")
        .to_string();
    Some(WorkflowRunRuntimeRelease {
        reason,
        released_at: fallback_released_at,
    })
}

fn latest_awaiting_input_event(
    events: &[WorkflowRunEventResource],
) -> Option<&WorkflowRunEventResource> {
    events.iter().rev().find(|event| {
        event.event_type == "workflow_run.awaiting_input"
            || event.event_type == "automation_task.awaiting_input"
    })
}

pub fn derive_workflow_run_runtime_resource(
    run_state: WorkflowRunState,
    session_state: Option<SessionLifecycleState>,
    events: &[WorkflowRunEventResource],
) -> Option<WorkflowRunRuntimeResource> {
    let awaiting_input = latest_awaiting_input_event(events)?;
    if run_state != WorkflowRunState::AwaitingInput
        && !events.iter().rev().any(|event| {
            event.created_at >= awaiting_input.created_at
                && event.event_type == "workflow_run.runtime_released"
        })
    {
        return None;
    }

    let hold_request = awaiting_input
        .data
        .as_ref()
        .and_then(|value| parse_workflow_run_runtime_hold_request(value).ok())
        .flatten();
    let hold_until = hold_request.as_ref().and_then(|request| {
        chrono::Duration::from_std(std::time::Duration::from_secs(request.timeout_sec))
            .ok()
            .map(|duration| awaiting_input.created_at + duration)
    });
    let released = events.iter().rev().find_map(|event| {
        if event.created_at < awaiting_input.created_at
            || event.event_type != "workflow_run.runtime_released"
        {
            return None;
        }
        event
            .data
            .as_ref()
            .and_then(|value| parse_workflow_run_runtime_release(value, event.created_at))
            .or_else(|| {
                Some(WorkflowRunRuntimeRelease {
                    reason: "runtime_released".to_string(),
                    released_at: event.created_at,
                })
            })
    });
    let exact_runtime_available = released.is_none()
        && session_state
            .map(SessionLifecycleState::is_runtime_candidate)
            .unwrap_or(false);
    Some(WorkflowRunRuntimeResource {
        resume_mode: if exact_runtime_available {
            WorkflowRunResumeMode::LiveRuntime
        } else {
            WorkflowRunResumeMode::ProfileRestart
        },
        exact_runtime_available,
        hold_until,
        released_at: released.as_ref().map(|value| value.released_at),
        release_reason: released.map(|value| value.reason),
        session_state,
    })
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

impl WorkflowRunProducedFile {
    pub fn to_resource(&self, run_id: Uuid) -> WorkflowRunProducedFileResource {
        WorkflowRunProducedFileResource {
            workspace_id: self.workspace_id,
            file_id: self.file_id,
            file_name: self.file_name.clone(),
            media_type: self.media_type.clone(),
            byte_count: self.byte_count,
            sha256_hex: self.sha256_hex.clone(),
            provenance: self.provenance.clone(),
            content_path: format!(
                "/api/v1/workflow-runs/{run_id}/produced-files/{}/content",
                self.file_id
            ),
            created_at: self.created_at,
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
        WorkflowRunState::Queued => "workflow_run.queued",
        WorkflowRunState::Starting => "workflow_run.starting",
        WorkflowRunState::Running => "workflow_run.running",
        WorkflowRunState::AwaitingInput => "workflow_run.awaiting_input",
        WorkflowRunState::Succeeded => "workflow_run.succeeded",
        WorkflowRunState::Failed => "workflow_run.failed",
        WorkflowRunState::Cancelled => "workflow_run.cancelled",
        WorkflowRunState::TimedOut => "workflow_run.timed_out",
    }
}

#[cfg(test)]
mod tests;

pub fn workflow_run_default_message(state: WorkflowRunState) -> &'static str {
    match state {
        WorkflowRunState::Pending => "workflow run returned to pending state",
        WorkflowRunState::Queued => "workflow run queued until worker capacity is available",
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
        WorkflowRunState::Queued => "automation_task.queued",
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
        WorkflowRunState::Queued => "automation task queued until worker capacity is available",
        WorkflowRunState::Starting => "automation task started",
        WorkflowRunState::Running => "automation task entered running state",
        WorkflowRunState::AwaitingInput => "automation task is awaiting input",
        WorkflowRunState::Succeeded => "automation task completed successfully",
        WorkflowRunState::Failed => "automation task failed",
        WorkflowRunState::Cancelled => "automation task cancelled",
        WorkflowRunState::TimedOut => "automation task timed out",
    }
}
