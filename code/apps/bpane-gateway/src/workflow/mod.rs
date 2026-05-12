pub mod observability;
pub mod resources;
pub mod retention;
pub mod runtime;
pub mod source;
pub mod state;

pub use observability::{WorkflowObservability, WorkflowObservabilitySnapshot};
pub use resources::{
    WorkflowDefinitionListResponse, WorkflowDefinitionResource, WorkflowDefinitionVersionResource,
    WorkflowRunEventListResponse, WorkflowRunEventResource, WorkflowRunListResponse,
    WorkflowRunLogListResponse, WorkflowRunLogResource, WorkflowRunProducedFileResource,
    WorkflowRunRecordingResource, WorkflowRunResource, WorkflowRunRetentionResource,
};
pub use retention::WorkflowRetentionManager;
pub use runtime::{
    derive_workflow_run_admission_resource, derive_workflow_run_intervention_resource,
    derive_workflow_run_runtime_resource, parse_workflow_run_runtime_hold_request,
    WorkflowRunAdmissionResource, WorkflowRunInterventionResource, WorkflowRunRuntimeHoldRequest,
    WorkflowRunRuntimeResource,
};
pub use source::{
    validate_workflow_source_entrypoint, WorkflowSource, WorkflowSourceArchive,
    WorkflowSourceError, WorkflowSourceResolver,
};
pub use state::{
    automation_task_default_message_for_run_state, automation_task_event_type_for_run_state,
    workflow_run_default_message, workflow_run_event_type, WorkflowRunEventSource,
    WorkflowRunLogSource, WorkflowRunState,
};

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::automation_tasks::AutomationTaskLogStream;
use crate::credentials::WorkflowRunCredentialBinding;
use crate::extensions::AppliedExtension;

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

#[derive(Debug, Clone)]
pub struct CreateWorkflowRunResult {
    pub run: StoredWorkflowRun,
    pub created: bool,
}

#[cfg(test)]
mod tests;
