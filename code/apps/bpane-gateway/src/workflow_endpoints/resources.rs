use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use super::{
    StoredWorkflowEndpoint, StoredWorkflowEndpointGrant, WorkflowEndpointArtifactBehavior,
    WorkflowEndpointGrantOperation, WorkflowEndpointState, WorkflowRunOutcome,
    WorkflowSideEffectState,
};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkflowEndpointResource {
    pub id: Uuid,
    pub project_id: Uuid,
    pub endpoint_key: String,
    pub purpose: String,
    pub workflow_definition_id: Uuid,
    pub workflow_definition_version_id: Uuid,
    pub workflow_version: String,
    pub input_schema: Value,
    pub output_schema: Value,
    pub execution_timeout_seconds: u32,
    pub inline_result_max_bytes: u32,
    pub artifact_behavior: WorkflowEndpointArtifactBehavior,
    pub supported_controls: Vec<String>,
    pub labels: HashMap<String, String>,
    pub state: WorkflowEndpointState,
    pub grants_path: String,
    pub invocations_path: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct WorkflowEndpointListResponse {
    pub workflow_endpoints: Vec<WorkflowEndpointResource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkflowEndpointGrantResource {
    pub id: Uuid,
    pub endpoint_id: Uuid,
    pub project_id: Uuid,
    pub service_principal_id: Uuid,
    pub operations: Vec<WorkflowEndpointGrantOperation>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct WorkflowEndpointGrantListResponse {
    pub grants: Vec<WorkflowEndpointGrantResource>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkflowEndpointArtifactResource {
    pub file_id: Uuid,
    pub file_name: String,
    pub media_type: String,
    pub byte_count: u64,
    pub sha256_hex: String,
    pub provenance: Option<Value>,
    pub retention_seconds: u32,
    pub expires_at: Option<DateTime<Utc>>,
    pub content_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkflowEndpointInvocationLinks {
    pub self_path: String,
    pub cancel_path: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkflowEndpointInvocationResource {
    pub id: Uuid,
    pub project_id: Uuid,
    pub endpoint_key: String,
    pub run_id: Option<Uuid>,
    pub state: String,
    pub source_system: Option<String>,
    pub source_reference: Option<String>,
    pub execution_deadline_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub outcome: Option<WorkflowRunOutcome>,
    pub side_effect_state: WorkflowSideEffectState,
    pub result: Option<Value>,
    pub artifacts: Vec<WorkflowEndpointArtifactResource>,
    pub links: WorkflowEndpointInvocationLinks,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl StoredWorkflowEndpoint {
    pub fn to_resource(&self) -> WorkflowEndpointResource {
        let base_path = format!(
            "/api/v1/projects/{}/workflow-endpoints/{}",
            self.project_id, self.endpoint_key
        );
        WorkflowEndpointResource {
            id: self.id,
            project_id: self.project_id,
            endpoint_key: self.endpoint_key.clone(),
            purpose: self.purpose.clone(),
            workflow_definition_id: self.workflow_definition_id,
            workflow_definition_version_id: self.workflow_definition_version_id,
            workflow_version: self.workflow_version.clone(),
            input_schema: self.input_schema.clone(),
            output_schema: self.output_schema.clone(),
            execution_timeout_seconds: self.execution_timeout_seconds,
            inline_result_max_bytes: self.inline_result_max_bytes,
            artifact_behavior: self.artifact_behavior.clone(),
            supported_controls: vec!["poll".to_string(), "cancel".to_string()],
            labels: self.labels.clone(),
            state: self.state,
            grants_path: format!("{base_path}/grants"),
            invocations_path: format!("{base_path}/invocations"),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

impl StoredWorkflowEndpointGrant {
    pub fn to_resource(&self) -> WorkflowEndpointGrantResource {
        WorkflowEndpointGrantResource {
            id: self.id,
            endpoint_id: self.endpoint_id,
            project_id: self.project_id,
            service_principal_id: self.service_principal_id,
            operations: self.operations.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}
