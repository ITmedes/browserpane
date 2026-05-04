use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::response::IntoResponse;
use axum::Json;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::auth::AuthValidator;
use crate::automation_tasks::{AutomationTaskLogStream, AutomationTaskState};
use crate::credentials::{
    CredentialBindingProvider, CredentialInjectionMode, CredentialProvider, CredentialTotpMetadata,
};
use crate::extensions::ExtensionVersionResource;
use crate::recording::{
    RecordingArtifactStore, RecordingObservability, SessionRecordingPlaybackResource,
};
use crate::recording_lifecycle::RecordingLifecycleManager;
use crate::session_access::{SessionAutomationAccessTokenManager, SessionConnectTicketManager};
use crate::session_control::{
    CreateSessionRequest, SessionConnectInfo, SessionLifecycleState, SessionOwnerMode,
    SessionRecordingFormat, SessionRecordingMode, SessionResource, SessionStatusSummary,
    SessionStore,
};
use crate::session_files::SessionFileBindingMode;
use crate::session_hub::{SessionConnectionTelemetryRole, SessionTelemetrySnapshot};
use crate::session_manager::SessionManager;
use crate::session_registry::SessionRegistry;
use crate::workflow::{
    WorkflowObservability, WorkflowRunProducedFileResource, WorkflowRunState, WorkflowSource,
    WorkflowSourceResolver,
};
use crate::workflow_lifecycle::WorkflowLifecycleManager;
use crate::workspaces::WorkspaceFileStore;

use super::errors::ErrorResponse;

pub(crate) struct ApiServerConfig {
    pub bind_addr: SocketAddr,
    pub registry: Arc<SessionRegistry>,
    pub auth_validator: Arc<AuthValidator>,
    pub connect_ticket_manager: Arc<SessionConnectTicketManager>,
    pub automation_access_token_manager: Arc<SessionAutomationAccessTokenManager>,
    pub session_store: SessionStore,
    pub session_manager: Arc<SessionManager>,
    pub credential_provider: Option<Arc<CredentialProvider>>,
    pub recording_artifact_store: Arc<RecordingArtifactStore>,
    pub workspace_file_store: Arc<WorkspaceFileStore>,
    pub workflow_source_resolver: Arc<WorkflowSourceResolver>,
    pub recording_observability: Arc<RecordingObservability>,
    pub recording_lifecycle: Arc<RecordingLifecycleManager>,
    pub workflow_lifecycle: Arc<WorkflowLifecycleManager>,
    pub workflow_observability: Arc<WorkflowObservability>,
    pub workflow_log_retention: Option<ChronoDuration>,
    pub workflow_output_retention: Option<ChronoDuration>,
    pub idle_stop_timeout: std::time::Duration,
    pub public_gateway_url: String,
    pub default_owner_mode: SessionOwnerMode,
}

/// Shared state for the HTTP API.
pub(super) struct ApiState {
    pub(super) registry: Arc<SessionRegistry>,
    pub(super) auth_validator: Arc<AuthValidator>,
    pub(super) connect_ticket_manager: Arc<SessionConnectTicketManager>,
    pub(super) automation_access_token_manager: Arc<SessionAutomationAccessTokenManager>,
    pub(super) session_store: SessionStore,
    pub(super) session_manager: Arc<SessionManager>,
    pub(super) credential_provider: Option<Arc<CredentialProvider>>,
    pub(super) recording_artifact_store: Arc<RecordingArtifactStore>,
    pub(super) workspace_file_store: Arc<WorkspaceFileStore>,
    pub(super) workflow_source_resolver: Arc<WorkflowSourceResolver>,
    pub(super) recording_observability: Arc<RecordingObservability>,
    pub(super) recording_lifecycle: Arc<RecordingLifecycleManager>,
    pub(super) workflow_lifecycle: Arc<WorkflowLifecycleManager>,
    pub(super) workflow_observability: Arc<WorkflowObservability>,
    pub(super) workflow_log_retention: Option<ChronoDuration>,
    pub(super) workflow_output_retention: Option<ChronoDuration>,
    pub(super) idle_stop_timeout: std::time::Duration,
    pub(super) public_gateway_url: String,
    pub(super) default_owner_mode: SessionOwnerMode,
}

pub(super) const AUTOMATION_ACCESS_TOKEN_HEADER: &str = "x-bpane-automation-access-token";
pub(super) const FILE_WORKSPACE_FILE_NAME_HEADER: &str = "x-bpane-file-name";
pub(super) const FILE_WORKSPACE_FILE_PROVENANCE_HEADER: &str = "x-bpane-file-provenance";
pub(super) const WORKFLOW_RUN_WORKSPACE_ID_HEADER: &str = "x-bpane-workflow-workspace-id";

#[derive(Serialize)]
pub(super) struct SessionStatus {
    pub(super) state: SessionLifecycleState,
    #[serde(flatten)]
    pub(super) summary: SessionStatusSummary,
    pub(super) connections: Vec<SessionConnectionInfo>,
    pub(super) browser_clients: u32,
    pub(super) viewer_clients: u32,
    pub(super) recorder_clients: u32,
    pub(super) max_viewers: u32,
    pub(super) viewer_slots_remaining: u32,
    pub(super) exclusive_browser_owner: bool,
    pub(super) mcp_owner: bool,
    pub(super) resolution: (u16, u16),
    pub(super) recording: SessionRecordingStatus,
    pub(super) playback: SessionRecordingPlaybackResource,
    pub(super) telemetry: SessionTelemetry,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum SessionConnectionRole {
    Owner,
    Viewer,
    Recorder,
}

impl From<SessionConnectionTelemetryRole> for SessionConnectionRole {
    fn from(value: SessionConnectionTelemetryRole) -> Self {
        match value {
            SessionConnectionTelemetryRole::Owner => Self::Owner,
            SessionConnectionTelemetryRole::Viewer => Self::Viewer,
            SessionConnectionTelemetryRole::Recorder => Self::Recorder,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct SessionConnectionInfo {
    pub(super) connection_id: u64,
    pub(super) role: SessionConnectionRole,
}

#[derive(Serialize)]
pub(super) struct SessionStopConflictResponse {
    pub(super) error: String,
    pub(super) session: SessionResource,
}

impl IntoResponse for SessionStopConflictResponse {
    fn into_response(self) -> axum::response::Response {
        Json(self).into_response()
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum SessionRecordingStatusState {
    Disabled,
    Idle,
    Recording,
    Finalizing,
    Ready,
    Failed,
}

#[derive(Serialize)]
pub(super) struct SessionRecordingStatus {
    pub(super) configured_mode: SessionRecordingMode,
    pub(super) format: SessionRecordingFormat,
    pub(super) retention_sec: Option<u32>,
    pub(super) state: SessionRecordingStatusState,
    pub(super) active_recording_id: Option<String>,
    pub(super) recorder_attached: bool,
    pub(super) started_at: Option<DateTime<Utc>>,
    pub(super) bytes_written: Option<u64>,
    pub(super) duration_ms: Option<u64>,
}

#[derive(Serialize)]
pub(super) struct SessionTelemetry {
    pub(super) joins_accepted: u64,
    pub(super) joins_rejected_viewer_cap: u64,
    pub(super) last_join_latency_ms: u64,
    pub(super) average_join_latency_ms: f64,
    pub(super) max_join_latency_ms: u64,
    pub(super) full_refresh_requests: u64,
    pub(super) full_refresh_tiles_requested: u64,
    pub(super) last_full_refresh_tiles: u64,
    pub(super) max_full_refresh_tiles: u64,
    pub(super) egress_send_stream_lock_acquires_total: u64,
    pub(super) egress_send_stream_lock_wait_us_total: u64,
    pub(super) egress_send_stream_lock_wait_us_average: f64,
    pub(super) egress_send_stream_lock_wait_us_max: u64,
    pub(super) egress_lagged_receives_total: u64,
    pub(super) egress_lagged_frames_total: u64,
}

#[derive(Deserialize)]
pub(super) struct McpOwnerRequest {
    pub(super) width: u16,
    pub(super) height: u16,
}

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct AutomationTaskSessionRequest {
    #[serde(default)]
    pub(super) existing_session_id: Option<Uuid>,
    #[serde(default)]
    pub(super) create_session: Option<CreateSessionRequest>,
}

#[derive(Deserialize)]
pub(super) struct CreateAutomationTaskRequest {
    #[serde(default)]
    pub(super) display_name: Option<String>,
    pub(super) executor: String,
    pub(super) session: AutomationTaskSessionRequest,
    #[serde(default)]
    pub(super) input: Option<Value>,
    #[serde(default)]
    pub(super) labels: HashMap<String, String>,
}

#[derive(Deserialize)]
pub(super) struct CreateWorkflowDefinitionRequest {
    pub(super) name: String,
    #[serde(default)]
    pub(super) description: Option<String>,
    #[serde(default)]
    pub(super) labels: HashMap<String, String>,
}

#[derive(Deserialize)]
pub(super) struct CreateWorkflowDefinitionVersionRequest {
    pub(super) version: String,
    pub(super) executor: String,
    pub(super) entrypoint: String,
    #[serde(default)]
    pub(super) source: Option<WorkflowSource>,
    #[serde(default)]
    pub(super) input_schema: Option<Value>,
    #[serde(default)]
    pub(super) output_schema: Option<Value>,
    #[serde(default)]
    pub(super) default_session: Option<Value>,
    #[serde(default)]
    pub(super) allowed_credential_binding_ids: Vec<String>,
    #[serde(default)]
    pub(super) allowed_extension_ids: Vec<String>,
    #[serde(default)]
    pub(super) allowed_file_workspace_ids: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct CreateWorkflowRunRequest {
    pub(super) workflow_id: Uuid,
    pub(super) version: String,
    #[serde(default)]
    pub(super) session: Option<AutomationTaskSessionRequest>,
    #[serde(default)]
    pub(super) input: Option<Value>,
    #[serde(default)]
    pub(super) source_system: Option<String>,
    #[serde(default)]
    pub(super) source_reference: Option<String>,
    #[serde(default)]
    pub(super) client_request_id: Option<String>,
    #[serde(default)]
    pub(super) credential_binding_ids: Vec<Uuid>,
    #[serde(default)]
    pub(super) workspace_inputs: Vec<CreateWorkflowRunWorkspaceInputRequest>,
    #[serde(default)]
    pub(super) labels: HashMap<String, String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct CreateWorkflowRunWorkspaceInputRequest {
    pub(super) workspace_id: Uuid,
    pub(super) file_id: Uuid,
    #[serde(default)]
    pub(super) mount_path: Option<String>,
}

#[derive(Serialize)]
pub(super) struct WorkflowRunProducedFileListResponse {
    pub(super) files: Vec<WorkflowRunProducedFileResource>,
}

#[derive(Deserialize)]
pub(super) struct CreateFileWorkspaceRequest {
    pub(super) name: String,
    #[serde(default)]
    pub(super) description: Option<String>,
    #[serde(default)]
    pub(super) labels: HashMap<String, String>,
}

#[derive(Deserialize)]
pub(super) struct CreateSessionFileBindingRequest {
    pub(super) workspace_id: Uuid,
    pub(super) file_id: Uuid,
    pub(super) mount_path: String,
    #[serde(default = "default_session_file_binding_mode")]
    pub(super) mode: SessionFileBindingMode,
    #[serde(default)]
    pub(super) labels: HashMap<String, String>,
}

fn default_session_file_binding_mode() -> SessionFileBindingMode {
    SessionFileBindingMode::ReadOnly
}

#[derive(Deserialize)]
pub(super) struct CreateCredentialBindingRequest {
    pub(super) name: String,
    pub(super) provider: CredentialBindingProvider,
    #[serde(default)]
    pub(super) external_ref: Option<String>,
    #[serde(default)]
    pub(super) namespace: Option<String>,
    #[serde(default)]
    pub(super) allowed_origins: Vec<String>,
    pub(super) injection_mode: CredentialInjectionMode,
    #[serde(default)]
    pub(super) totp: Option<CredentialTotpMetadata>,
    #[serde(default)]
    pub(super) secret_payload: Option<Value>,
    #[serde(default)]
    pub(super) labels: HashMap<String, String>,
}

#[derive(Deserialize)]
pub(super) struct CreateExtensionDefinitionRequest {
    pub(super) name: String,
    #[serde(default)]
    pub(super) description: Option<String>,
    #[serde(default)]
    pub(super) labels: HashMap<String, String>,
}

#[derive(Deserialize)]
pub(super) struct CreateExtensionVersionRequest {
    pub(super) version: String,
    pub(super) install_path: String,
}

#[derive(Deserialize)]
pub(super) struct TransitionAutomationTaskRequest {
    pub(super) state: AutomationTaskState,
    #[serde(default)]
    pub(super) output: Option<Value>,
    #[serde(default)]
    pub(super) error: Option<String>,
    #[serde(default)]
    pub(super) artifact_refs: Vec<String>,
    #[serde(default)]
    pub(super) message: Option<String>,
    #[serde(default)]
    pub(super) data: Option<Value>,
}

#[derive(Deserialize)]
pub(super) struct AppendAutomationTaskLogRequest {
    pub(super) stream: AutomationTaskLogStream,
    pub(super) message: String,
}

#[derive(Deserialize)]
pub(super) struct TransitionWorkflowRunRequest {
    pub(super) state: WorkflowRunState,
    #[serde(default)]
    pub(super) output: Option<Value>,
    #[serde(default)]
    pub(super) error: Option<String>,
    #[serde(default)]
    pub(super) artifact_refs: Vec<String>,
    #[serde(default)]
    pub(super) message: Option<String>,
    #[serde(default)]
    pub(super) data: Option<Value>,
}

#[derive(Deserialize)]
pub(super) struct SubmitWorkflowRunInputRequest {
    pub(super) input: Value,
    #[serde(default)]
    pub(super) comment: Option<String>,
    #[serde(default)]
    pub(super) details: Option<Value>,
}

#[derive(Deserialize)]
pub(super) struct ResumeWorkflowRunRequest {
    #[serde(default)]
    pub(super) comment: Option<String>,
    #[serde(default)]
    pub(super) details: Option<Value>,
}

#[derive(Deserialize)]
pub(super) struct RejectWorkflowRunRequest {
    pub(super) reason: String,
    #[serde(default)]
    pub(super) details: Option<Value>,
}

#[derive(Deserialize)]
pub(super) struct AppendWorkflowRunLogRequest {
    pub(super) stream: AutomationTaskLogStream,
    pub(super) message: String,
}

#[derive(Deserialize)]
pub(super) struct CreateWorkflowEventSubscriptionRequest {
    pub(super) name: String,
    pub(super) target_url: String,
    pub(super) event_types: Vec<String>,
    pub(super) signing_secret: String,
}

#[derive(Serialize)]
pub(super) struct OkResponse {
    pub(super) ok: bool,
}

#[derive(Serialize)]
pub(super) struct SessionAccessTokenResponse {
    pub(super) session_id: Uuid,
    pub(super) token_type: String,
    pub(super) token: String,
    pub(super) expires_at: DateTime<Utc>,
    pub(super) connect: SessionConnectInfo,
}

#[derive(Serialize)]
pub(super) struct SessionAutomationAccessInfo {
    pub(super) endpoint_url: String,
    pub(super) protocol: String,
    pub(super) auth_type: String,
    pub(super) auth_header: String,
    pub(super) status_path: String,
    pub(super) mcp_owner_path: String,
    pub(super) compatibility_mode: String,
}

#[derive(Serialize)]
pub(super) struct SessionAutomationAccessResponse {
    pub(super) session_id: Uuid,
    pub(super) token_type: String,
    pub(super) token: String,
    pub(super) expires_at: DateTime<Utc>,
    pub(super) automation: SessionAutomationAccessInfo,
}

#[allow(dead_code)]
fn _keep_types_module_linked(
    _: Json<ErrorResponse>,
    _: SessionTelemetrySnapshot,
    _: ExtensionVersionResource,
) {
}
