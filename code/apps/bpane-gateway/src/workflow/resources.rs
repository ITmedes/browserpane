use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use crate::automation_tasks::{
    AutomationTaskLogStream, StoredAutomationTaskEvent, StoredAutomationTaskLog,
};
use crate::credentials::WorkflowRunCredentialBindingResource;
use crate::extensions::{AppliedExtension, AppliedExtensionResource};

use super::{
    StoredWorkflowDefinition, StoredWorkflowDefinitionVersion, StoredWorkflowRun,
    StoredWorkflowRunEvent, StoredWorkflowRunLog, WorkflowRunAdmissionResource,
    WorkflowRunEventSource, WorkflowRunInterventionResource, WorkflowRunLogSource,
    WorkflowRunProducedFile, WorkflowRunRuntimeResource, WorkflowRunSourceSnapshot,
    WorkflowRunState, WorkflowRunWorkspaceInput, WorkflowSource,
};

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
