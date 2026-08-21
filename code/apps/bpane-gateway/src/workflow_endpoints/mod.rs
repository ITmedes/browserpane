mod outcome;
mod resources;
mod schema;

use std::collections::HashMap;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

pub use outcome::{derive_terminal_evidence, WorkflowEndpointRunEvidence};
pub use resources::{
    WorkflowEndpointArtifactResource, WorkflowEndpointGrantListResponse,
    WorkflowEndpointGrantResource, WorkflowEndpointInvocationLinks,
    WorkflowEndpointInvocationResource, WorkflowEndpointListResponse, WorkflowEndpointResource,
};
pub use schema::{
    canonical_request_fingerprint, validate_endpoint_schema, validate_schema_instance,
    WorkflowSchemaViolation,
};

pub const MIN_EXECUTION_TIMEOUT_SECONDS: u32 = 1;
pub const MAX_EXECUTION_TIMEOUT_SECONDS: u32 = 86_400;
pub const MIN_INLINE_RESULT_BYTES: u32 = 1;
pub const MAX_INLINE_RESULT_BYTES: u32 = 1_048_576;
pub const MAX_ARTIFACT_RETENTION_SECONDS: u32 = 2_592_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowEndpointState {
    Draft,
    Active,
    Disabled,
}

impl WorkflowEndpointState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Active => "active",
            Self::Disabled => "disabled",
        }
    }
}

impl FromStr for WorkflowEndpointState {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "draft" => Ok(Self::Draft),
            "active" => Ok(Self::Active),
            "disabled" => Ok(Self::Disabled),
            _ => Err("unknown workflow endpoint state"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorkflowEndpointGrantOperation {
    #[serde(rename = "invoke")]
    Invoke,
    #[serde(rename = "read")]
    Read,
    #[serde(rename = "cancel")]
    Cancel,
    #[serde(rename = "artifact.read")]
    ArtifactRead,
}

impl WorkflowEndpointGrantOperation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Invoke => "invoke",
            Self::Read => "read",
            Self::Cancel => "cancel",
            Self::ArtifactRead => "artifact.read",
        }
    }

    pub fn required_scope(self) -> &'static str {
        match self {
            Self::Invoke => "workflow-endpoints:invoke",
            Self::Read => "workflow-endpoints:read",
            Self::Cancel => "workflow-endpoints:cancel",
            Self::ArtifactRead => "workflow-endpoints:artifact.read",
        }
    }
}

impl FromStr for WorkflowEndpointGrantOperation {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "invoke" => Ok(Self::Invoke),
            "read" => Ok(Self::Read),
            "cancel" => Ok(Self::Cancel),
            "artifact.read" => Ok(Self::ArtifactRead),
            _ => Err("unknown workflow endpoint grant operation"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowSideEffectState {
    None,
    Confirmed,
    Uncertain,
}

impl WorkflowSideEffectState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Confirmed => "confirmed",
            Self::Uncertain => "uncertain",
        }
    }
}

impl FromStr for WorkflowSideEffectState {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "none" => Ok(Self::None),
            "confirmed" => Ok(Self::Confirmed),
            "uncertain" => Ok(Self::Uncertain),
            _ => Err("unknown workflow side-effect state"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowOutcomeCategory {
    Success,
    ValidationFailure,
    PolicyDenial,
    BusinessFailure,
    RetryableTechnicalFailure,
    PermanentTechnicalFailure,
    Timeout,
    Cancellation,
    ExternalInterventionRequired,
}

impl WorkflowOutcomeCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::ValidationFailure => "validation_failure",
            Self::PolicyDenial => "policy_denial",
            Self::BusinessFailure => "business_failure",
            Self::RetryableTechnicalFailure => "retryable_technical_failure",
            Self::PermanentTechnicalFailure => "permanent_technical_failure",
            Self::Timeout => "timeout",
            Self::Cancellation => "cancellation",
            Self::ExternalInterventionRequired => "external_intervention_required",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowRunOutcome {
    pub category: WorkflowOutcomeCategory,
    pub code: String,
    pub message: String,
    pub details: Option<Value>,
    pub retryable: bool,
    pub retry_after_seconds: Option<u32>,
    pub caused_by: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowEndpointArtifactBehavior {
    pub mode: String,
    pub retention_seconds: u32,
}

impl Default for WorkflowEndpointArtifactBehavior {
    fn default() -> Self {
        Self {
            mode: "authorized_references".to_string(),
            retention_seconds: 86_400,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PersistWorkflowEndpointRequest {
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
    pub labels: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct StoredWorkflowEndpoint {
    pub id: Uuid,
    pub owner_subject: String,
    pub owner_issuer: String,
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
    pub labels: HashMap<String, String>,
    pub state: WorkflowEndpointState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct PersistWorkflowEndpointGrantRequest {
    pub service_principal_id: Uuid,
    pub operations: Vec<WorkflowEndpointGrantOperation>,
}

#[derive(Debug, Clone)]
pub struct StoredWorkflowEndpointGrant {
    pub id: Uuid,
    pub endpoint_id: Uuid,
    pub project_id: Uuid,
    pub service_principal_id: Uuid,
    pub operations: Vec<WorkflowEndpointGrantOperation>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ReserveWorkflowEndpointInvocationRequest {
    pub endpoint_id: Uuid,
    pub caller_service_principal_id: Uuid,
    pub idempotency_key: String,
    pub request_fingerprint: String,
}

#[derive(Debug, Clone)]
pub struct StoredWorkflowEndpointInvocation {
    pub id: Uuid,
    pub endpoint_id: Uuid,
    pub caller_service_principal_id: Uuid,
    pub idempotency_key: String,
    pub request_fingerprint: String,
    pub run_id: Option<Uuid>,
    pub outcome: Option<WorkflowRunOutcome>,
    pub side_effect_state: WorkflowSideEffectState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ReserveWorkflowEndpointInvocationResult {
    pub invocation: StoredWorkflowEndpointInvocation,
    pub created: bool,
}

#[derive(Debug, Clone)]
pub struct WorkflowEndpointRunContext {
    pub endpoint_id: Uuid,
    pub invocation_id: Uuid,
    pub endpoint_key: String,
    pub caller_service_principal_id: Uuid,
    pub idempotency_key: String,
    pub request_fingerprint: String,
    pub execution_deadline_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests;
