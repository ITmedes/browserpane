use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::Context;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value};
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use tokio_postgres::{Client, Connection, NoTls, Row, Socket, Transaction};
use uuid::Uuid;

use crate::auth::AuthenticatedPrincipal;
use crate::automation_task::{
    AutomationTaskLogStream, AutomationTaskSessionSource, AutomationTaskState,
    AutomationTaskTransitionRequest, PersistAutomationTaskRequest, StoredAutomationTask,
    StoredAutomationTaskEvent, StoredAutomationTaskLog,
};
use crate::credential_binding::{
    CredentialBindingProvider, CredentialInjectionMode, CredentialTotpMetadata,
    PersistCredentialBindingRequest, StoredCredentialBinding, WorkflowRunCredentialBinding,
};
use crate::extension::{
    AppliedExtension, PersistExtensionDefinitionRequest, PersistExtensionVersionRequest,
    StoredExtensionDefinition, StoredExtensionVersion,
};
use crate::file_workspace::{
    PersistFileWorkspaceFileRequest, PersistFileWorkspaceRequest, StoredFileWorkspace,
    StoredFileWorkspaceFile,
};
use crate::session_manager::{
    PersistedSessionRuntimeAssignment, SessionManagerProfile, SessionRuntimeAccess,
    SessionRuntimeAssignmentStatus,
};
use crate::workflow::{
    automation_task_default_message_for_run_state, automation_task_event_type_for_run_state,
    workflow_run_default_message, workflow_run_event_type, CreateWorkflowRunResult,
    PersistWorkflowDefinitionRequest, PersistWorkflowDefinitionVersionRequest,
    PersistWorkflowRunEventRequest, PersistWorkflowRunLogRequest,
    PersistWorkflowRunProducedFileRequest, PersistWorkflowRunRequest, StoredWorkflowDefinition,
    StoredWorkflowDefinitionVersion, StoredWorkflowRun, StoredWorkflowRunEvent,
    StoredWorkflowRunLog, WorkflowRunProducedFile, WorkflowRunSourceSnapshot, WorkflowRunState,
    WorkflowRunTransitionRequest, WorkflowRunWorkspaceInput,
};
use crate::workflow_event_delivery::{
    build_workflow_event_delivery_payload, validate_workflow_event_subscription_request,
    workflow_event_type_matches, PersistWorkflowEventSubscriptionRequest,
    RecordWorkflowEventDeliveryAttemptRequest, StoredWorkflowEventDelivery,
    StoredWorkflowEventDeliveryAttempt, StoredWorkflowEventSubscription,
    WorkflowEventDeliveryState,
};
use crate::workflow_source::WorkflowSource;

mod rows;
mod validation;

use rows::*;
use validation::*;

const DEFAULT_VIEWPORT_WIDTH: u16 = 1600;
const DEFAULT_VIEWPORT_HEIGHT: u16 = 900;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionLifecycleState {
    Pending,
    Starting,
    Ready,
    Active,
    Idle,
    Stopping,
    Stopped,
    Failed,
    Expired,
}

impl SessionLifecycleState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Starting => "starting",
            Self::Ready => "ready",
            Self::Active => "active",
            Self::Idle => "idle",
            Self::Stopping => "stopping",
            Self::Stopped => "stopped",
            Self::Failed => "failed",
            Self::Expired => "expired",
        }
    }

    pub fn is_runtime_candidate(self) -> bool {
        matches!(
            self,
            Self::Pending | Self::Starting | Self::Ready | Self::Active | Self::Idle
        )
    }
}

impl FromStr for SessionLifecycleState {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "pending" => Ok(Self::Pending),
            "starting" => Ok(Self::Starting),
            "ready" => Ok(Self::Ready),
            "active" => Ok(Self::Active),
            "idle" => Ok(Self::Idle),
            "stopping" => Ok(Self::Stopping),
            "stopped" => Ok(Self::Stopped),
            "failed" => Ok(Self::Failed),
            "expired" => Ok(Self::Expired),
            _ => Err("unknown session lifecycle state"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionOwnerMode {
    Collaborative,
    ExclusiveBrowserOwner,
}

impl SessionOwnerMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Collaborative => "collaborative",
            Self::ExclusiveBrowserOwner => "exclusive_browser_owner",
        }
    }
}

impl FromStr for SessionOwnerMode {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "collaborative" => Ok(Self::Collaborative),
            "exclusive_browser_owner" => Ok(Self::ExclusiveBrowserOwner),
            _ => Err("unknown session owner mode"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionViewport {
    pub width: u16,
    pub height: u16,
}

impl Default for SessionViewport {
    fn default() -> Self {
        Self {
            width: DEFAULT_VIEWPORT_WIDTH,
            height: DEFAULT_VIEWPORT_HEIGHT,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionRecordingMode {
    #[default]
    Disabled,
    Manual,
    Always,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionRecordingFormat {
    #[default]
    Webm,
}

impl SessionRecordingFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Webm => "webm",
        }
    }
}

impl FromStr for SessionRecordingFormat {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "webm" => Ok(Self::Webm),
            _ => Err("unknown session recording format"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRecordingPolicy {
    #[serde(default)]
    pub mode: SessionRecordingMode,
    #[serde(default)]
    pub format: SessionRecordingFormat,
    #[serde(default)]
    pub retention_sec: Option<u32>,
}

impl Default for SessionRecordingPolicy {
    fn default() -> Self {
        Self {
            mode: SessionRecordingMode::Disabled,
            format: SessionRecordingFormat::Webm,
            retention_sec: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionRecordingState {
    Starting,
    Recording,
    Finalizing,
    Ready,
    Failed,
}

impl SessionRecordingState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Recording => "recording",
            Self::Finalizing => "finalizing",
            Self::Ready => "ready",
            Self::Failed => "failed",
        }
    }

    pub fn is_active(self) -> bool {
        matches!(self, Self::Starting | Self::Recording | Self::Finalizing)
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Ready | Self::Failed)
    }
}

impl FromStr for SessionRecordingState {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "starting" => Ok(Self::Starting),
            "recording" => Ok(Self::Recording),
            "finalizing" => Ok(Self::Finalizing),
            "ready" => Ok(Self::Ready),
            "failed" => Ok(Self::Failed),
            _ => Err("unknown session recording state"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionRecordingTerminationReason {
    ManualStop,
    SessionStop,
    IdleStop,
    GatewayRestart,
    WorkerExit,
}

impl SessionRecordingTerminationReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ManualStop => "manual_stop",
            Self::SessionStop => "session_stop",
            Self::IdleStop => "idle_stop",
            Self::GatewayRestart => "gateway_restart",
            Self::WorkerExit => "worker_exit",
        }
    }
}

impl FromStr for SessionRecordingTerminationReason {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "manual_stop" => Ok(Self::ManualStop),
            "session_stop" => Ok(Self::SessionStop),
            "idle_stop" => Ok(Self::IdleStop),
            "gateway_restart" => Ok(Self::GatewayRestart),
            "worker_exit" => Ok(Self::WorkerExit),
            _ => Err("unknown session recording termination reason"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionOwner {
    pub subject: String,
    pub issuer: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionAutomationDelegate {
    pub client_id: String,
    pub issuer: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionCapabilities {
    pub browser_input: bool,
    pub clipboard: bool,
    pub audio: bool,
    pub microphone: bool,
    pub camera: bool,
    pub file_transfer: bool,
    pub resize: bool,
}

impl Default for SessionCapabilities {
    fn default() -> Self {
        Self {
            browser_input: true,
            clipboard: true,
            audio: true,
            microphone: true,
            camera: true,
            file_transfer: true,
            resize: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionConnectInfo {
    pub gateway_url: String,
    pub transport_path: String,
    pub auth_type: String,
    pub ticket_path: Option<String>,
    pub compatibility_mode: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRuntimeInfo {
    pub binding: String,
    pub compatibility_mode: String,
    pub cdp_endpoint: Option<String>,
}

impl From<SessionRuntimeAccess> for SessionRuntimeInfo {
    fn from(value: SessionRuntimeAccess) -> Self {
        Self {
            binding: value.binding,
            compatibility_mode: value.compatibility_mode,
            cdp_endpoint: value.cdp_endpoint,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SessionResource {
    pub id: Uuid,
    pub state: SessionLifecycleState,
    pub template_id: Option<String>,
    pub owner_mode: SessionOwnerMode,
    pub viewport: SessionViewport,
    pub capabilities: SessionCapabilities,
    pub owner: SessionOwner,
    pub automation_delegate: Option<SessionAutomationDelegate>,
    pub idle_timeout_sec: Option<u32>,
    pub labels: HashMap<String, String>,
    pub integration_context: Option<Value>,
    pub extensions: Vec<crate::extension::AppliedExtensionResource>,
    pub recording: SessionRecordingPolicy,
    pub connect: SessionConnectInfo,
    pub runtime: SessionRuntimeInfo,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub stopped_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SessionRecordingResource {
    pub id: Uuid,
    pub session_id: Uuid,
    pub previous_recording_id: Option<Uuid>,
    pub state: SessionRecordingState,
    pub format: SessionRecordingFormat,
    pub mime_type: Option<String>,
    pub bytes: Option<u64>,
    pub duration_ms: Option<u64>,
    pub error: Option<String>,
    pub termination_reason: Option<SessionRecordingTerminationReason>,
    pub artifact_available: bool,
    pub content_path: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct SessionRecordingListResponse {
    pub recordings: Vec<SessionRecordingResource>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateSessionRequest {
    #[serde(default)]
    pub template_id: Option<String>,
    #[serde(default)]
    pub owner_mode: Option<SessionOwnerMode>,
    #[serde(default)]
    pub viewport: Option<SessionViewport>,
    #[serde(default)]
    pub idle_timeout_sec: Option<u32>,
    #[serde(default)]
    pub labels: HashMap<String, String>,
    #[serde(default)]
    pub integration_context: Option<Value>,
    #[serde(default)]
    pub extension_ids: Vec<Uuid>,
    #[serde(default)]
    pub recording: SessionRecordingPolicy,
    #[serde(skip)]
    pub extensions: Vec<AppliedExtension>,
}

#[derive(Debug, Deserialize)]
pub struct SetAutomationDelegateRequest {
    pub client_id: String,
    #[serde(default)]
    pub issuer: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CompleteSessionRecordingRequest {
    #[serde(alias = "artifact_path")]
    pub source_path: String,
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub bytes: Option<u64>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct PersistCompletedSessionRecordingRequest {
    pub artifact_ref: String,
    pub mime_type: Option<String>,
    pub bytes: Option<u64>,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FailSessionRecordingRequest {
    pub error: String,
    #[serde(default)]
    pub termination_reason: Option<SessionRecordingTerminationReason>,
}

#[derive(Debug, Serialize)]
pub struct SessionListResponse {
    pub sessions: Vec<SessionResource>,
}

#[derive(Debug, Clone)]
pub struct StoredSession {
    pub id: Uuid,
    pub state: SessionLifecycleState,
    pub template_id: Option<String>,
    pub owner_mode: SessionOwnerMode,
    pub viewport: SessionViewport,
    pub owner: SessionOwner,
    pub automation_delegate: Option<SessionAutomationDelegate>,
    pub idle_timeout_sec: Option<u32>,
    pub labels: HashMap<String, String>,
    pub integration_context: Option<Value>,
    pub extensions: Vec<AppliedExtension>,
    pub recording: SessionRecordingPolicy,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub stopped_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct StoredSessionRecording {
    pub id: Uuid,
    pub session_id: Uuid,
    pub previous_recording_id: Option<Uuid>,
    pub state: SessionRecordingState,
    pub format: SessionRecordingFormat,
    pub mime_type: Option<String>,
    pub bytes: Option<u64>,
    pub duration_ms: Option<u64>,
    pub error: Option<String>,
    pub termination_reason: Option<SessionRecordingTerminationReason>,
    pub artifact_ref: Option<String>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordingArtifactRetentionCandidate {
    pub session_id: Uuid,
    pub recording_id: Uuid,
    pub artifact_ref: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionRecordingWorkerAssignmentStatus {
    Starting,
    Running,
    Stopping,
}

impl SessionRecordingWorkerAssignmentStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Stopping => "stopping",
        }
    }
}

impl FromStr for SessionRecordingWorkerAssignmentStatus {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "starting" => Ok(Self::Starting),
            "running" => Ok(Self::Running),
            "stopping" => Ok(Self::Stopping),
            _ => Err("unknown session recording worker assignment status"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedSessionRecordingWorkerAssignment {
    pub session_id: Uuid,
    pub recording_id: Uuid,
    pub status: SessionRecordingWorkerAssignmentStatus,
    pub process_id: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowRunWorkerAssignmentStatus {
    Starting,
    Running,
    Stopping,
}

impl WorkflowRunWorkerAssignmentStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Stopping => "stopping",
        }
    }
}

impl FromStr for WorkflowRunWorkerAssignmentStatus {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "starting" => Ok(Self::Starting),
            "running" => Ok(Self::Running),
            "stopping" => Ok(Self::Stopping),
            _ => Err("unknown workflow run worker assignment status"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedWorkflowRunWorkerAssignment {
    pub run_id: Uuid,
    pub session_id: Uuid,
    pub automation_task_id: Uuid,
    pub status: WorkflowRunWorkerAssignmentStatus,
    pub process_id: Option<u32>,
    pub container_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowRunLogRetentionCandidate {
    pub run_id: Uuid,
    pub automation_task_id: Uuid,
    pub session_id: Uuid,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowRunOutputRetentionCandidate {
    pub run_id: Uuid,
    pub session_id: Uuid,
    pub expires_at: DateTime<Utc>,
}

impl StoredSession {
    pub fn to_resource(
        &self,
        public_gateway_url: &str,
        runtime: SessionRuntimeInfo,
        state_override: Option<SessionLifecycleState>,
    ) -> SessionResource {
        SessionResource {
            id: self.id,
            state: state_override.unwrap_or(self.state),
            template_id: self.template_id.clone(),
            owner_mode: self.owner_mode,
            viewport: self.viewport.clone(),
            capabilities: SessionCapabilities::default(),
            owner: self.owner.clone(),
            automation_delegate: self.automation_delegate.clone(),
            idle_timeout_sec: self.idle_timeout_sec,
            labels: self.labels.clone(),
            integration_context: self.integration_context.clone(),
            extensions: self
                .extensions
                .iter()
                .map(AppliedExtension::to_resource)
                .collect(),
            recording: self.recording.clone(),
            connect: SessionConnectInfo {
                gateway_url: public_gateway_url.to_string(),
                transport_path: "/session".to_string(),
                auth_type: "session_connect_ticket".to_string(),
                ticket_path: Some(format!("/api/v1/sessions/{}/access-tokens", self.id)),
                compatibility_mode: runtime.compatibility_mode.clone(),
            },
            runtime,
            created_at: self.created_at,
            updated_at: self.updated_at,
            stopped_at: self.stopped_at,
        }
    }
}

impl StoredSessionRecording {
    pub fn to_resource(&self) -> SessionRecordingResource {
        SessionRecordingResource {
            id: self.id,
            session_id: self.session_id,
            previous_recording_id: self.previous_recording_id,
            state: self.state,
            format: self.format,
            mime_type: self.mime_type.clone(),
            bytes: self.bytes,
            duration_ms: self.duration_ms,
            error: self.error.clone(),
            termination_reason: self.termination_reason,
            artifact_available: self.artifact_ref.is_some(),
            content_path: format!(
                "/api/v1/sessions/{}/recordings/{}/content",
                self.session_id, self.id
            ),
            started_at: self.started_at,
            completed_at: self.completed_at,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

#[derive(Debug, Clone)]
pub enum SessionStoreError {
    ActiveSessionConflict { max_runtime_sessions: usize },
    Conflict(String),
    NotFound(String),
    InvalidRequest(String),
    Backend(String),
}

impl std::fmt::Display for SessionStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ActiveSessionConflict {
                max_runtime_sessions,
            } => {
                write!(
                    f,
                    "the current gateway runtime only supports {} active runtime-backed session{}",
                    max_runtime_sessions,
                    if *max_runtime_sessions == 1 { "" } else { "s" }
                )
            }
            Self::Conflict(message) => write!(f, "{message}"),
            Self::NotFound(message) => write!(f, "{message}"),
            Self::InvalidRequest(message) => write!(f, "{message}"),
            Self::Backend(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for SessionStoreError {}

#[derive(Clone)]
pub struct SessionStore {
    backend: SessionStoreBackend,
}

#[derive(Debug, Clone)]
struct SessionStoreConfig {
    runtime_binding: String,
    max_runtime_candidates: usize,
}

#[derive(Clone)]
enum SessionStoreBackend {
    InMemory(Arc<InMemorySessionStore>),
    Postgres(Arc<PostgresSessionStore>),
}

impl From<SessionManagerProfile> for SessionStoreConfig {
    fn from(runtime_profile: SessionManagerProfile) -> Self {
        Self {
            runtime_binding: runtime_profile.runtime_binding,
            max_runtime_candidates: runtime_profile.max_runtime_sessions,
        }
    }
}

#[cfg(test)]
fn legacy_runtime_profile() -> SessionManagerProfile {
    SessionManagerProfile {
        runtime_binding: "legacy_single_session".to_string(),
        compatibility_mode: "legacy_single_runtime".to_string(),
        max_runtime_sessions: 1,
        supports_legacy_global_routes: true,
        supports_session_extensions: false,
    }
}

impl SessionStore {
    #[cfg(test)]
    pub fn in_memory() -> Self {
        Self::in_memory_with_config(legacy_runtime_profile())
    }

    pub fn in_memory_with_config(runtime_profile: SessionManagerProfile) -> Self {
        Self {
            backend: SessionStoreBackend::InMemory(Arc::new(InMemorySessionStore {
                sessions: Mutex::new(Vec::new()),
                automation_tasks: Mutex::new(Vec::new()),
                automation_task_events: Mutex::new(Vec::new()),
                automation_task_logs: Mutex::new(Vec::new()),
                workflow_definitions: Mutex::new(Vec::new()),
                workflow_definition_versions: Mutex::new(Vec::new()),
                workflow_runs: Mutex::new(Vec::new()),
                workflow_run_events: Mutex::new(Vec::new()),
                workflow_run_logs: Mutex::new(Vec::new()),
                workflow_event_subscriptions: Mutex::new(Vec::new()),
                workflow_event_deliveries: Mutex::new(Vec::new()),
                workflow_event_delivery_attempts: Mutex::new(Vec::new()),
                credential_bindings: Mutex::new(Vec::new()),
                extension_definitions: Mutex::new(Vec::new()),
                extension_versions: Mutex::new(Vec::new()),
                file_workspaces: Mutex::new(Vec::new()),
                file_workspace_files: Mutex::new(Vec::new()),
                recordings: Mutex::new(Vec::new()),
                runtime_assignments: Mutex::new(HashMap::new()),
                recording_worker_assignments: Mutex::new(HashMap::new()),
                workflow_run_worker_assignments: Mutex::new(HashMap::new()),
                config: SessionStoreConfig::from(runtime_profile),
            })),
        }
    }

    pub async fn from_database_url_with_config(
        database_url: &str,
        runtime_profile: SessionManagerProfile,
    ) -> Result<Self, SessionStoreError> {
        let (client, connection) = connect_to_postgres_with_retry(database_url).await?;
        tokio::spawn(async move {
            if let Err(error) = connection.await {
                tracing::error!("postgres connection error: {error}");
            }
        });
        let store = PostgresSessionStore {
            client: Mutex::new(client),
            config: SessionStoreConfig::from(runtime_profile),
        };
        store.migrate().await?;
        Ok(Self {
            backend: SessionStoreBackend::Postgres(Arc::new(store)),
        })
    }

    pub async fn create_session(
        &self,
        principal: &AuthenticatedPrincipal,
        request: CreateSessionRequest,
        owner_mode: SessionOwnerMode,
    ) -> Result<StoredSession, SessionStoreError> {
        validate_create_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.create_session(principal, request, owner_mode).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.create_session(principal, request, owner_mode).await
            }
        }
    }

    pub async fn list_sessions_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredSession>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => store.list_sessions_for_owner(principal).await,
            SessionStoreBackend::Postgres(store) => store.list_sessions_for_owner(principal).await,
        }
    }

    pub async fn get_session_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.get_session_for_owner(principal, id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.get_session_for_owner(principal, id).await
            }
        }
    }

    pub async fn stop_session_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.stop_session_for_owner(principal, id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.stop_session_for_owner(principal, id).await
            }
        }
    }

    pub async fn get_session_for_principal(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.get_session_for_principal(principal, id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.get_session_for_principal(principal, id).await
            }
        }
    }

    pub async fn get_session_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => store.get_session_by_id(id).await,
            SessionStoreBackend::Postgres(store) => store.get_session_by_id(id).await,
        }
    }

    pub async fn prepare_session_for_connect(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => store.prepare_session_for_connect(id).await,
            SessionStoreBackend::Postgres(store) => store.prepare_session_for_connect(id).await,
        }
    }

    pub async fn get_runtime_candidate_session(
        &self,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => store.get_runtime_candidate_session().await,
            SessionStoreBackend::Postgres(store) => store.get_runtime_candidate_session().await,
        }
    }

    pub async fn set_automation_delegate_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        request: SetAutomationDelegateRequest,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        validate_automation_delegate_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .set_automation_delegate_for_owner(principal, id, request)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .set_automation_delegate_for_owner(principal, id, request)
                    .await
            }
        }
    }

    pub async fn clear_automation_delegate_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .clear_automation_delegate_for_owner(principal, id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .clear_automation_delegate_for_owner(principal, id)
                    .await
            }
        }
    }

    pub async fn create_automation_task(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistAutomationTaskRequest,
    ) -> Result<StoredAutomationTask, SessionStoreError> {
        validate_persist_automation_task_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.create_automation_task(principal, request).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.create_automation_task(principal, request).await
            }
        }
    }

    pub async fn list_automation_tasks_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredAutomationTask>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.list_automation_tasks_for_owner(principal).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.list_automation_tasks_for_owner(principal).await
            }
        }
    }

    pub async fn get_automation_task_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredAutomationTask>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.get_automation_task_for_owner(principal, id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.get_automation_task_for_owner(principal, id).await
            }
        }
    }

    pub async fn get_automation_task_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredAutomationTask>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => store.get_automation_task_by_id(id).await,
            SessionStoreBackend::Postgres(store) => store.get_automation_task_by_id(id).await,
        }
    }

    pub async fn cancel_automation_task_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredAutomationTask>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.cancel_automation_task_for_owner(principal, id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.cancel_automation_task_for_owner(principal, id).await
            }
        }
    }

    pub async fn list_automation_task_events_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Vec<StoredAutomationTaskEvent>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .list_automation_task_events_for_owner(principal, id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .list_automation_task_events_for_owner(principal, id)
                    .await
            }
        }
    }

    pub async fn list_automation_task_logs_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Vec<StoredAutomationTaskLog>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .list_automation_task_logs_for_owner(principal, id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .list_automation_task_logs_for_owner(principal, id)
                    .await
            }
        }
    }

    pub async fn transition_automation_task(
        &self,
        id: Uuid,
        request: AutomationTaskTransitionRequest,
    ) -> Result<Option<StoredAutomationTask>, SessionStoreError> {
        validate_automation_task_transition_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.transition_automation_task(id, request).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.transition_automation_task(id, request).await
            }
        }
    }

    pub async fn append_automation_task_log(
        &self,
        id: Uuid,
        stream: AutomationTaskLogStream,
        message: String,
    ) -> Result<Option<StoredAutomationTaskLog>, SessionStoreError> {
        validate_automation_task_log_message(&message)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.append_automation_task_log(id, stream, message).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.append_automation_task_log(id, stream, message).await
            }
        }
    }

    pub async fn create_workflow_definition(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistWorkflowDefinitionRequest,
    ) -> Result<StoredWorkflowDefinition, SessionStoreError> {
        validate_workflow_definition_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.create_workflow_definition(principal, request).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.create_workflow_definition(principal, request).await
            }
        }
    }

    pub async fn list_workflow_definitions_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredWorkflowDefinition>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.list_workflow_definitions_for_owner(principal).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.list_workflow_definitions_for_owner(principal).await
            }
        }
    }

    pub async fn get_workflow_definition_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowDefinition>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.get_workflow_definition_for_owner(principal, id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.get_workflow_definition_for_owner(principal, id).await
            }
        }
    }

    pub async fn create_workflow_definition_version(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistWorkflowDefinitionVersionRequest,
    ) -> Result<StoredWorkflowDefinitionVersion, SessionStoreError> {
        validate_workflow_definition_version_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .create_workflow_definition_version(principal, request)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .create_workflow_definition_version(principal, request)
                    .await
            }
        }
    }

    pub async fn get_workflow_definition_version_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        workflow_definition_id: Uuid,
        version: &str,
    ) -> Result<Option<StoredWorkflowDefinitionVersion>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .get_workflow_definition_version_for_owner(
                        principal,
                        workflow_definition_id,
                        version,
                    )
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .get_workflow_definition_version_for_owner(
                        principal,
                        workflow_definition_id,
                        version,
                    )
                    .await
            }
        }
    }

    pub async fn get_workflow_definition_version_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowDefinitionVersion>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.get_workflow_definition_version_by_id(id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.get_workflow_definition_version_by_id(id).await
            }
        }
    }

    pub async fn create_workflow_run(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistWorkflowRunRequest,
    ) -> Result<CreateWorkflowRunResult, SessionStoreError> {
        validate_workflow_run_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.create_workflow_run(principal, request).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.create_workflow_run(principal, request).await
            }
        }
    }

    pub async fn get_workflow_run_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.get_workflow_run_for_owner(principal, id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.get_workflow_run_for_owner(principal, id).await
            }
        }
    }

    pub async fn get_workflow_run_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => store.get_workflow_run_by_id(id).await,
            SessionStoreBackend::Postgres(store) => store.get_workflow_run_by_id(id).await,
        }
    }

    pub async fn list_dispatchable_workflow_runs(
        &self,
    ) -> Result<Vec<StoredWorkflowRun>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => store.list_dispatchable_workflow_runs().await,
            SessionStoreBackend::Postgres(store) => store.list_dispatchable_workflow_runs().await,
        }
    }

    pub async fn find_workflow_run_by_client_request_id_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        client_request_id: &str,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .find_workflow_run_by_client_request_id_for_owner(principal, client_request_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .find_workflow_run_by_client_request_id_for_owner(principal, client_request_id)
                    .await
            }
        }
    }

    pub async fn list_workflow_run_events_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Vec<StoredWorkflowRunEvent>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .list_workflow_run_events_for_owner(principal, id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .list_workflow_run_events_for_owner(principal, id)
                    .await
            }
        }
    }

    pub async fn list_workflow_run_events(
        &self,
        id: Uuid,
    ) -> Result<Vec<StoredWorkflowRunEvent>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => store.list_workflow_run_events(id).await,
            SessionStoreBackend::Postgres(store) => store.list_workflow_run_events(id).await,
        }
    }

    pub async fn list_workflow_run_logs_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Vec<StoredWorkflowRunLog>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.list_workflow_run_logs_for_owner(principal, id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.list_workflow_run_logs_for_owner(principal, id).await
            }
        }
    }

    pub async fn create_workflow_event_subscription(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistWorkflowEventSubscriptionRequest,
    ) -> Result<StoredWorkflowEventSubscription, SessionStoreError> {
        validate_workflow_event_subscription_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .create_workflow_event_subscription(principal, request)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .create_workflow_event_subscription(principal, request)
                    .await
            }
        }
    }

    pub async fn list_workflow_event_subscriptions_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredWorkflowEventSubscription>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .list_workflow_event_subscriptions_for_owner(principal)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .list_workflow_event_subscriptions_for_owner(principal)
                    .await
            }
        }
    }

    pub async fn get_workflow_event_subscription_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowEventSubscription>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .get_workflow_event_subscription_for_owner(principal, id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .get_workflow_event_subscription_for_owner(principal, id)
                    .await
            }
        }
    }

    pub async fn delete_workflow_event_subscription_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowEventSubscription>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .delete_workflow_event_subscription_for_owner(principal, id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .delete_workflow_event_subscription_for_owner(principal, id)
                    .await
            }
        }
    }

    pub async fn list_workflow_event_deliveries_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        subscription_id: Uuid,
    ) -> Result<Vec<StoredWorkflowEventDelivery>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .list_workflow_event_deliveries_for_owner(principal, subscription_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .list_workflow_event_deliveries_for_owner(principal, subscription_id)
                    .await
            }
        }
    }

    pub async fn list_workflow_event_delivery_attempts_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        subscription_id: Uuid,
    ) -> Result<Vec<StoredWorkflowEventDeliveryAttempt>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .list_workflow_event_delivery_attempts_for_owner(principal, subscription_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .list_workflow_event_delivery_attempts_for_owner(principal, subscription_id)
                    .await
            }
        }
    }

    pub async fn requeue_inflight_workflow_event_deliveries(
        &self,
    ) -> Result<(), SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.requeue_inflight_workflow_event_deliveries().await
            }
            SessionStoreBackend::Postgres(store) => {
                store.requeue_inflight_workflow_event_deliveries().await
            }
        }
    }

    pub async fn claim_due_workflow_event_deliveries(
        &self,
        limit: usize,
        now: DateTime<Utc>,
    ) -> Result<Vec<StoredWorkflowEventDelivery>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.claim_due_workflow_event_deliveries(limit, now).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.claim_due_workflow_event_deliveries(limit, now).await
            }
        }
    }

    pub async fn record_workflow_event_delivery_attempt(
        &self,
        delivery_id: Uuid,
        request: RecordWorkflowEventDeliveryAttemptRequest,
    ) -> Result<Option<StoredWorkflowEventDelivery>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .record_workflow_event_delivery_attempt(delivery_id, request)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .record_workflow_event_delivery_attempt(delivery_id, request)
                    .await
            }
        }
    }

    pub async fn append_workflow_run_event_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        request: PersistWorkflowRunEventRequest,
    ) -> Result<Option<StoredWorkflowRunEvent>, SessionStoreError> {
        validate_workflow_run_event_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .append_workflow_run_event_for_owner(principal, id, request)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .append_workflow_run_event_for_owner(principal, id, request)
                    .await
            }
        }
    }

    pub async fn append_workflow_run_event(
        &self,
        id: Uuid,
        request: PersistWorkflowRunEventRequest,
    ) -> Result<Option<StoredWorkflowRunEvent>, SessionStoreError> {
        validate_workflow_run_event_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.append_workflow_run_event(id, request).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.append_workflow_run_event(id, request).await
            }
        }
    }

    pub async fn transition_workflow_run(
        &self,
        id: Uuid,
        request: WorkflowRunTransitionRequest,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        validate_workflow_run_transition_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.transition_workflow_run(id, request).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.transition_workflow_run(id, request).await
            }
        }
    }

    pub async fn reconcile_workflow_run_from_task(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.reconcile_workflow_run_from_task(id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.reconcile_workflow_run_from_task(id).await
            }
        }
    }

    pub async fn list_awaiting_input_workflow_runs(
        &self,
    ) -> Result<Vec<StoredWorkflowRun>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => store.list_awaiting_input_workflow_runs().await,
            SessionStoreBackend::Postgres(store) => store.list_awaiting_input_workflow_runs().await,
        }
    }

    pub async fn append_workflow_run_log(
        &self,
        id: Uuid,
        request: PersistWorkflowRunLogRequest,
    ) -> Result<Option<StoredWorkflowRunLog>, SessionStoreError> {
        validate_workflow_run_log_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.append_workflow_run_log(id, request).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.append_workflow_run_log(id, request).await
            }
        }
    }

    pub async fn append_workflow_run_produced_file(
        &self,
        id: Uuid,
        request: PersistWorkflowRunProducedFileRequest,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        validate_workflow_run_produced_file_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.append_workflow_run_produced_file(id, request).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.append_workflow_run_produced_file(id, request).await
            }
        }
    }

    pub async fn list_workflow_run_log_retention_candidates(
        &self,
        now: DateTime<Utc>,
        retention: ChronoDuration,
    ) -> Result<Vec<WorkflowRunLogRetentionCandidate>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .list_workflow_run_log_retention_candidates(now, retention)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .list_workflow_run_log_retention_candidates(now, retention)
                    .await
            }
        }
    }

    pub async fn delete_workflow_run_logs(
        &self,
        run_id: Uuid,
        automation_task_id: Uuid,
    ) -> Result<usize, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .delete_workflow_run_logs(run_id, automation_task_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .delete_workflow_run_logs(run_id, automation_task_id)
                    .await
            }
        }
    }

    pub async fn list_workflow_run_output_retention_candidates(
        &self,
        now: DateTime<Utc>,
        retention: ChronoDuration,
    ) -> Result<Vec<WorkflowRunOutputRetentionCandidate>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .list_workflow_run_output_retention_candidates(now, retention)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .list_workflow_run_output_retention_candidates(now, retention)
                    .await
            }
        }
    }

    pub async fn clear_workflow_run_output(
        &self,
        run_id: Uuid,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => store.clear_workflow_run_output(run_id).await,
            SessionStoreBackend::Postgres(store) => store.clear_workflow_run_output(run_id).await,
        }
    }

    pub async fn create_file_workspace(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistFileWorkspaceRequest,
    ) -> Result<StoredFileWorkspace, SessionStoreError> {
        validate_file_workspace_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.create_file_workspace(principal, request).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.create_file_workspace(principal, request).await
            }
        }
    }

    pub async fn create_credential_binding(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistCredentialBindingRequest,
    ) -> Result<StoredCredentialBinding, SessionStoreError> {
        validate_credential_binding_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.create_credential_binding(principal, request).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.create_credential_binding(principal, request).await
            }
        }
    }

    pub async fn list_credential_bindings_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredCredentialBinding>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.list_credential_bindings_for_owner(principal).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.list_credential_bindings_for_owner(principal).await
            }
        }
    }

    pub async fn get_credential_binding_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredCredentialBinding>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.get_credential_binding_for_owner(principal, id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.get_credential_binding_for_owner(principal, id).await
            }
        }
    }

    pub async fn create_extension_definition(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistExtensionDefinitionRequest,
    ) -> Result<StoredExtensionDefinition, SessionStoreError> {
        validate_extension_definition_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.create_extension_definition(principal, request).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.create_extension_definition(principal, request).await
            }
        }
    }

    pub async fn list_extension_definitions_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredExtensionDefinition>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.list_extension_definitions_for_owner(principal).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.list_extension_definitions_for_owner(principal).await
            }
        }
    }

    pub async fn get_extension_definition_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredExtensionDefinition>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .get_extension_definition_for_owner(principal, id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .get_extension_definition_for_owner(principal, id)
                    .await
            }
        }
    }

    pub async fn set_extension_definition_enabled_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        enabled: bool,
    ) -> Result<Option<StoredExtensionDefinition>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .set_extension_definition_enabled_for_owner(principal, id, enabled)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .set_extension_definition_enabled_for_owner(principal, id, enabled)
                    .await
            }
        }
    }

    pub async fn create_extension_version_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistExtensionVersionRequest,
    ) -> Result<StoredExtensionVersion, SessionStoreError> {
        validate_extension_version_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .create_extension_version_for_owner(principal, request)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .create_extension_version_for_owner(principal, request)
                    .await
            }
        }
    }

    pub async fn get_latest_extension_version_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        extension_definition_id: Uuid,
    ) -> Result<Option<StoredExtensionVersion>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .get_latest_extension_version_for_owner(principal, extension_definition_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .get_latest_extension_version_for_owner(principal, extension_definition_id)
                    .await
            }
        }
    }

    pub async fn list_file_workspaces_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredFileWorkspace>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.list_file_workspaces_for_owner(principal).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.list_file_workspaces_for_owner(principal).await
            }
        }
    }

    pub async fn get_file_workspace_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredFileWorkspace>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.get_file_workspace_for_owner(principal, id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.get_file_workspace_for_owner(principal, id).await
            }
        }
    }

    pub async fn create_file_workspace_file_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistFileWorkspaceFileRequest,
    ) -> Result<StoredFileWorkspaceFile, SessionStoreError> {
        validate_file_workspace_file_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .create_file_workspace_file_for_owner(principal, request)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .create_file_workspace_file_for_owner(principal, request)
                    .await
            }
        }
    }

    pub async fn list_file_workspace_files_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        workspace_id: Uuid,
    ) -> Result<Vec<StoredFileWorkspaceFile>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .list_file_workspace_files_for_owner(principal, workspace_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .list_file_workspace_files_for_owner(principal, workspace_id)
                    .await
            }
        }
    }

    pub async fn get_file_workspace_file_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        workspace_id: Uuid,
        file_id: Uuid,
    ) -> Result<Option<StoredFileWorkspaceFile>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .get_file_workspace_file_for_owner(principal, workspace_id, file_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .get_file_workspace_file_for_owner(principal, workspace_id, file_id)
                    .await
            }
        }
    }

    pub async fn delete_file_workspace_file_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        workspace_id: Uuid,
        file_id: Uuid,
    ) -> Result<Option<StoredFileWorkspaceFile>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .delete_file_workspace_file_for_owner(principal, workspace_id, file_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .delete_file_workspace_file_for_owner(principal, workspace_id, file_id)
                    .await
            }
        }
    }

    pub async fn mark_session_active(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .mark_session_state(id, SessionLifecycleState::Active)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .mark_session_state(id, SessionLifecycleState::Active)
                    .await
            }
        }
    }

    pub async fn mark_session_idle(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .mark_session_state(id, SessionLifecycleState::Idle)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .mark_session_state(id, SessionLifecycleState::Idle)
                    .await
            }
        }
    }

    pub async fn stop_session_if_idle(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => store.stop_session_if_idle(id).await,
            SessionStoreBackend::Postgres(store) => store.stop_session_if_idle(id).await,
        }
    }

    pub async fn upsert_runtime_assignment(
        &self,
        assignment: PersistedSessionRuntimeAssignment,
    ) -> Result<(), SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.upsert_runtime_assignment(assignment).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.upsert_runtime_assignment(assignment).await
            }
        }
    }

    pub async fn create_recording_for_session(
        &self,
        session_id: Uuid,
        format: SessionRecordingFormat,
        previous_recording_id: Option<Uuid>,
    ) -> Result<StoredSessionRecording, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .create_recording_for_session(session_id, format, previous_recording_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .create_recording_for_session(session_id, format, previous_recording_id)
                    .await
            }
        }
    }

    pub async fn list_recordings_for_session(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<StoredSessionRecording>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.list_recordings_for_session(session_id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.list_recordings_for_session(session_id).await
            }
        }
    }

    pub async fn get_recording_for_session(
        &self,
        session_id: Uuid,
        recording_id: Uuid,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .get_recording_for_session(session_id, recording_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .get_recording_for_session(session_id, recording_id)
                    .await
            }
        }
    }

    pub async fn get_latest_recording_for_session(
        &self,
        session_id: Uuid,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.get_latest_recording_for_session(session_id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.get_latest_recording_for_session(session_id).await
            }
        }
    }

    pub async fn list_recording_artifact_retention_candidates(
        &self,
        now: DateTime<Utc>,
    ) -> Result<Vec<RecordingArtifactRetentionCandidate>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .list_recording_artifact_retention_candidates(now)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .list_recording_artifact_retention_candidates(now)
                    .await
            }
        }
    }

    pub async fn stop_recording_for_session(
        &self,
        session_id: Uuid,
        recording_id: Uuid,
        termination_reason: SessionRecordingTerminationReason,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .stop_recording_for_session(session_id, recording_id, termination_reason)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .stop_recording_for_session(session_id, recording_id, termination_reason)
                    .await
            }
        }
    }

    pub async fn clear_recording_artifact_path(
        &self,
        session_id: Uuid,
        recording_id: Uuid,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .clear_recording_artifact_path(session_id, recording_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .clear_recording_artifact_path(session_id, recording_id)
                    .await
            }
        }
    }

    pub async fn complete_recording_for_session(
        &self,
        session_id: Uuid,
        recording_id: Uuid,
        request: PersistCompletedSessionRecordingRequest,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        validate_persist_completed_recording_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .complete_recording_for_session(session_id, recording_id, request)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .complete_recording_for_session(session_id, recording_id, request)
                    .await
            }
        }
    }

    pub async fn fail_recording_for_session(
        &self,
        session_id: Uuid,
        recording_id: Uuid,
        request: FailSessionRecordingRequest,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        validate_fail_recording_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .fail_recording_for_session(session_id, recording_id, request)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .fail_recording_for_session(session_id, recording_id, request)
                    .await
            }
        }
    }

    pub async fn clear_runtime_assignment(&self, id: Uuid) -> Result<(), SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => store.clear_runtime_assignment(id).await,
            SessionStoreBackend::Postgres(store) => store.clear_runtime_assignment(id).await,
        }
    }

    pub async fn upsert_recording_worker_assignment(
        &self,
        assignment: PersistedSessionRecordingWorkerAssignment,
    ) -> Result<(), SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.upsert_recording_worker_assignment(assignment).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.upsert_recording_worker_assignment(assignment).await
            }
        }
    }

    pub async fn clear_recording_worker_assignment(
        &self,
        session_id: Uuid,
    ) -> Result<(), SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.clear_recording_worker_assignment(session_id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.clear_recording_worker_assignment(session_id).await
            }
        }
    }

    pub async fn get_recording_worker_assignment(
        &self,
        session_id: Uuid,
    ) -> Result<Option<PersistedSessionRecordingWorkerAssignment>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.get_recording_worker_assignment(session_id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.get_recording_worker_assignment(session_id).await
            }
        }
    }

    pub async fn list_recording_worker_assignments(
        &self,
    ) -> Result<Vec<PersistedSessionRecordingWorkerAssignment>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => store.list_recording_worker_assignments().await,
            SessionStoreBackend::Postgres(store) => store.list_recording_worker_assignments().await,
        }
    }

    pub async fn upsert_workflow_run_worker_assignment(
        &self,
        assignment: PersistedWorkflowRunWorkerAssignment,
    ) -> Result<(), SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .upsert_workflow_run_worker_assignment(assignment)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .upsert_workflow_run_worker_assignment(assignment)
                    .await
            }
        }
    }

    pub async fn clear_workflow_run_worker_assignment(
        &self,
        run_id: Uuid,
    ) -> Result<(), SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.clear_workflow_run_worker_assignment(run_id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.clear_workflow_run_worker_assignment(run_id).await
            }
        }
    }

    pub async fn get_workflow_run_worker_assignment(
        &self,
        run_id: Uuid,
    ) -> Result<Option<PersistedWorkflowRunWorkerAssignment>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.get_workflow_run_worker_assignment(run_id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.get_workflow_run_worker_assignment(run_id).await
            }
        }
    }

    pub async fn list_workflow_run_worker_assignments(
        &self,
    ) -> Result<Vec<PersistedWorkflowRunWorkerAssignment>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.list_workflow_run_worker_assignments().await
            }
            SessionStoreBackend::Postgres(store) => {
                store.list_workflow_run_worker_assignments().await
            }
        }
    }

    pub async fn list_runtime_assignments(
        &self,
        runtime_binding: &str,
    ) -> Result<Vec<PersistedSessionRuntimeAssignment>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.list_runtime_assignments(runtime_binding).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.list_runtime_assignments(runtime_binding).await
            }
        }
    }

    pub async fn mark_session_ready_after_runtime_loss(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.mark_session_ready_after_runtime_loss(id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.mark_session_ready_after_runtime_loss(id).await
            }
        }
    }
}

async fn connect_to_postgres_with_retry(
    database_url: &str,
) -> Result<(Client, Connection<Socket, tokio_postgres::tls::NoTlsStream>), SessionStoreError> {
    let max_attempts = 30;
    let mut last_error = String::new();
    for attempt in 0..max_attempts {
        match tokio_postgres::connect(database_url, NoTls).await {
            Ok(connection) => return Ok(connection),
            Err(error) => {
                last_error = error.to_string();
                if attempt + 1 < max_attempts {
                    sleep(Duration::from_secs(2)).await;
                }
            }
        }
    }

    Err(SessionStoreError::Backend(format!(
        "failed to connect to postgres after retries: {last_error}"
    )))
}

struct InMemorySessionStore {
    sessions: Mutex<Vec<StoredSession>>,
    automation_tasks: Mutex<Vec<StoredAutomationTask>>,
    automation_task_events: Mutex<Vec<StoredAutomationTaskEvent>>,
    automation_task_logs: Mutex<Vec<StoredAutomationTaskLog>>,
    workflow_definitions: Mutex<Vec<StoredWorkflowDefinition>>,
    workflow_definition_versions: Mutex<Vec<StoredWorkflowDefinitionVersion>>,
    workflow_runs: Mutex<Vec<StoredWorkflowRun>>,
    workflow_run_events: Mutex<Vec<StoredWorkflowRunEvent>>,
    workflow_run_logs: Mutex<Vec<StoredWorkflowRunLog>>,
    workflow_event_subscriptions: Mutex<Vec<StoredWorkflowEventSubscription>>,
    workflow_event_deliveries: Mutex<Vec<StoredWorkflowEventDelivery>>,
    workflow_event_delivery_attempts: Mutex<Vec<StoredWorkflowEventDeliveryAttempt>>,
    credential_bindings: Mutex<Vec<StoredCredentialBinding>>,
    extension_definitions: Mutex<Vec<StoredExtensionDefinition>>,
    extension_versions: Mutex<Vec<StoredExtensionVersion>>,
    file_workspaces: Mutex<Vec<StoredFileWorkspace>>,
    file_workspace_files: Mutex<Vec<StoredFileWorkspaceFile>>,
    recordings: Mutex<Vec<StoredSessionRecording>>,
    runtime_assignments: Mutex<HashMap<Uuid, PersistedSessionRuntimeAssignment>>,
    recording_worker_assignments: Mutex<HashMap<Uuid, PersistedSessionRecordingWorkerAssignment>>,
    workflow_run_worker_assignments: Mutex<HashMap<Uuid, PersistedWorkflowRunWorkerAssignment>>,
    config: SessionStoreConfig,
}

impl InMemorySessionStore {
    async fn get_session_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let session = self
            .sessions
            .lock()
            .await
            .iter()
            .find(|session| session.id == id)
            .cloned();
        Ok(session)
    }

    async fn queue_workflow_event_deliveries_for_run_event(
        &self,
        run: &StoredWorkflowRun,
        event: &StoredWorkflowRunEvent,
    ) {
        let subscriptions = self.workflow_event_subscriptions.lock().await.clone();
        let mut deliveries = self.workflow_event_deliveries.lock().await;
        for subscription in subscriptions.into_iter().filter(|subscription| {
            subscription.owner_subject == run.owner_subject
                && subscription.owner_issuer == run.owner_issuer
                && workflow_event_type_matches(&subscription.event_types, &event.event_type)
        }) {
            let delivery_id = Uuid::now_v7();
            deliveries.push(StoredWorkflowEventDelivery {
                id: delivery_id,
                subscription_id: subscription.id,
                run_id: run.id,
                event_id: event.id,
                event_type: event.event_type.clone(),
                target_url: subscription.target_url.clone(),
                signing_secret: subscription.signing_secret.clone(),
                payload: build_workflow_event_delivery_payload(
                    subscription.id,
                    delivery_id,
                    run,
                    event,
                ),
                state: WorkflowEventDeliveryState::Pending,
                attempt_count: 0,
                next_attempt_at: Some(event.created_at),
                last_attempt_at: None,
                delivered_at: None,
                last_response_status: None,
                last_error: None,
                created_at: event.created_at,
                updated_at: event.created_at,
            });
        }
    }

    async fn create_session(
        &self,
        principal: &AuthenticatedPrincipal,
        request: CreateSessionRequest,
        owner_mode: SessionOwnerMode,
    ) -> Result<StoredSession, SessionStoreError> {
        let mut sessions = self.sessions.lock().await;
        let active_runtime_candidates = sessions
            .iter()
            .filter(|session| session.state.is_runtime_candidate())
            .count();
        if active_runtime_candidates >= self.config.max_runtime_candidates {
            return Err(SessionStoreError::ActiveSessionConflict {
                max_runtime_sessions: self.config.max_runtime_candidates,
            });
        }

        let now = Utc::now();
        let session = StoredSession {
            id: Uuid::now_v7(),
            state: SessionLifecycleState::Ready,
            template_id: request.template_id,
            owner_mode,
            viewport: request.viewport.unwrap_or_default(),
            owner: SessionOwner {
                subject: principal.subject.clone(),
                issuer: principal.issuer.clone(),
                display_name: principal.display_name.clone(),
            },
            automation_delegate: None,
            idle_timeout_sec: request.idle_timeout_sec,
            labels: request.labels,
            integration_context: request.integration_context,
            extensions: request.extensions,
            recording: request.recording,
            created_at: now,
            updated_at: now,
            stopped_at: None,
        };
        sessions.push(session.clone());
        Ok(session)
    }

    async fn list_sessions_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredSession>, SessionStoreError> {
        let mut sessions = self
            .sessions
            .lock()
            .await
            .iter()
            .filter(|session| {
                session.owner.subject == principal.subject
                    && session.owner.issuer == principal.issuer
            })
            .cloned()
            .collect::<Vec<_>>();
        sessions.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(sessions)
    }

    async fn get_session_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let session = self
            .sessions
            .lock()
            .await
            .iter()
            .find(|session| {
                session.id == id
                    && session.owner.subject == principal.subject
                    && session.owner.issuer == principal.issuer
            })
            .cloned();
        Ok(session)
    }

    async fn get_session_for_principal(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let session = self
            .sessions
            .lock()
            .await
            .iter()
            .find(|session| session.id == id && session_visible_to_principal(session, principal))
            .cloned();
        Ok(session)
    }

    async fn get_runtime_candidate_session(
        &self,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let session = self
            .sessions
            .lock()
            .await
            .iter()
            .filter(|session| session.state.is_runtime_candidate())
            .max_by(|left, right| left.updated_at.cmp(&right.updated_at))
            .cloned();
        Ok(session)
    }

    async fn stop_session_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let mut sessions = self.sessions.lock().await;
        let Some(session) = sessions.iter_mut().find(|session| {
            session.id == id
                && session.owner.subject == principal.subject
                && session.owner.issuer == principal.issuer
        }) else {
            return Ok(None);
        };

        if session.state != SessionLifecycleState::Stopped {
            session.state = SessionLifecycleState::Stopped;
            session.updated_at = Utc::now();
            session.stopped_at = Some(session.updated_at);
        }

        Ok(Some(session.clone()))
    }

    async fn mark_session_state(
        &self,
        id: Uuid,
        state: SessionLifecycleState,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let mut sessions = self.sessions.lock().await;
        let Some(session) = sessions.iter_mut().find(|session| session.id == id) else {
            return Ok(None);
        };

        if !session.state.is_runtime_candidate() {
            return Ok(Some(session.clone()));
        }

        session.state = state;
        session.updated_at = Utc::now();
        Ok(Some(session.clone()))
    }

    async fn stop_session_if_idle(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let mut sessions = self.sessions.lock().await;
        let Some(session) = sessions.iter_mut().find(|session| session.id == id) else {
            return Ok(None);
        };

        if !matches!(
            session.state,
            SessionLifecycleState::Ready | SessionLifecycleState::Idle
        ) {
            return Ok(Some(session.clone()));
        }

        session.state = SessionLifecycleState::Stopped;
        session.updated_at = Utc::now();
        session.stopped_at = Some(session.updated_at);
        Ok(Some(session.clone()))
    }

    async fn prepare_session_for_connect(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let mut sessions = self.sessions.lock().await;
        let Some(index) = sessions.iter().position(|session| session.id == id) else {
            return Ok(None);
        };

        let state = sessions[index].state;
        if state != SessionLifecycleState::Stopped {
            return Ok(Some(sessions[index].clone()));
        }

        let active_runtime_candidates = sessions
            .iter()
            .filter(|session| session.state.is_runtime_candidate())
            .count();
        if active_runtime_candidates >= self.config.max_runtime_candidates {
            return Err(SessionStoreError::ActiveSessionConflict {
                max_runtime_sessions: self.config.max_runtime_candidates,
            });
        }

        let session = &mut sessions[index];
        session.state = SessionLifecycleState::Ready;
        session.updated_at = Utc::now();
        session.stopped_at = None;
        Ok(Some(session.clone()))
    }

    async fn set_automation_delegate_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        request: SetAutomationDelegateRequest,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let mut sessions = self.sessions.lock().await;
        let Some(session) = sessions.iter_mut().find(|session| {
            session.id == id
                && session.owner.subject == principal.subject
                && session.owner.issuer == principal.issuer
        }) else {
            return Ok(None);
        };

        session.automation_delegate = Some(SessionAutomationDelegate {
            client_id: request.client_id,
            issuer: request.issuer.unwrap_or_else(|| principal.issuer.clone()),
            display_name: request.display_name,
        });
        session.updated_at = Utc::now();

        Ok(Some(session.clone()))
    }

    async fn clear_automation_delegate_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let mut sessions = self.sessions.lock().await;
        let Some(session) = sessions.iter_mut().find(|session| {
            session.id == id
                && session.owner.subject == principal.subject
                && session.owner.issuer == principal.issuer
        }) else {
            return Ok(None);
        };

        session.automation_delegate = None;
        session.updated_at = Utc::now();

        Ok(Some(session.clone()))
    }

    async fn create_automation_task(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistAutomationTaskRequest,
    ) -> Result<StoredAutomationTask, SessionStoreError> {
        let now = Utc::now();
        let task = StoredAutomationTask {
            id: Uuid::now_v7(),
            display_name: request.display_name,
            executor: request.executor,
            state: AutomationTaskState::Pending,
            session_id: request.session_id,
            session_source: request.session_source,
            input: request.input,
            output: None,
            error: None,
            artifact_refs: Vec::new(),
            labels: request.labels,
            cancel_requested_at: None,
            started_at: None,
            completed_at: None,
            created_at: now,
            updated_at: now,
        };
        let event = StoredAutomationTaskEvent {
            id: Uuid::now_v7(),
            task_id: task.id,
            event_type: "automation_task.created".to_string(),
            message: "automation task created".to_string(),
            data: Some(serde_json::json!({
                "session_id": task.session_id,
                "session_source": task.session_source.as_str(),
                "executor": task.executor,
                "owner_subject": principal.subject,
                "owner_issuer": principal.issuer,
            })),
            created_at: now,
        };
        self.automation_tasks.lock().await.push(task.clone());
        self.automation_task_events.lock().await.push(event);
        Ok(task)
    }

    async fn list_automation_tasks_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredAutomationTask>, SessionStoreError> {
        let sessions = self.sessions.lock().await;
        let visible_session_ids = sessions
            .iter()
            .filter(|session| task_visible_to_principal(session, principal))
            .map(|session| session.id)
            .collect::<Vec<_>>();
        drop(sessions);

        let mut tasks = self
            .automation_tasks
            .lock()
            .await
            .iter()
            .filter(|task| visible_session_ids.contains(&task.session_id))
            .cloned()
            .collect::<Vec<_>>();
        tasks.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(tasks)
    }

    async fn get_automation_task_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredAutomationTask>, SessionStoreError> {
        let Some(task) = self
            .automation_tasks
            .lock()
            .await
            .iter()
            .find(|task| task.id == id)
            .cloned()
        else {
            return Ok(None);
        };
        let Some(session) = self.get_session_by_id(task.session_id).await? else {
            return Ok(None);
        };
        if !task_visible_to_principal(&session, principal) {
            return Ok(None);
        }
        Ok(Some(task))
    }

    async fn get_automation_task_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredAutomationTask>, SessionStoreError> {
        Ok(self
            .automation_tasks
            .lock()
            .await
            .iter()
            .find(|task| task.id == id)
            .cloned())
    }

    async fn cancel_automation_task_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredAutomationTask>, SessionStoreError> {
        let visible = self.get_automation_task_for_owner(principal, id).await?;
        let Some(visible) = visible else {
            return Ok(None);
        };
        if visible.state.is_terminal() {
            return Err(SessionStoreError::Conflict(format!(
                "automation task {id} is already terminal"
            )));
        }

        let mut tasks = self.automation_tasks.lock().await;
        let Some(task) = tasks.iter_mut().find(|task| task.id == id) else {
            return Ok(None);
        };
        let now = Utc::now();
        task.state = AutomationTaskState::Cancelled;
        task.cancel_requested_at = Some(now);
        task.completed_at = Some(now);
        task.updated_at = now;
        let task = task.clone();
        drop(tasks);

        let workflow_run_id = if let Some(run) = self
            .workflow_runs
            .lock()
            .await
            .iter_mut()
            .find(|run| run.automation_task_id == id)
        {
            sync_workflow_run_with_task(run, &task);
            Some(run.id)
        } else {
            None
        };

        self.automation_task_events
            .lock()
            .await
            .push(StoredAutomationTaskEvent {
                id: Uuid::now_v7(),
                task_id: id,
                event_type: "automation_task.cancelled".to_string(),
                message: "automation task cancelled".to_string(),
                data: None,
                created_at: now,
            });
        self.automation_task_logs
            .lock()
            .await
            .push(StoredAutomationTaskLog {
                id: Uuid::now_v7(),
                task_id: id,
                stream: AutomationTaskLogStream::System,
                message: "automation task cancelled".to_string(),
                created_at: now,
            });
        if let Some(run_id) = workflow_run_id {
            let event = StoredWorkflowRunEvent {
                id: Uuid::now_v7(),
                run_id,
                event_type: "workflow_run.cancelled".to_string(),
                message: "workflow run cancelled".to_string(),
                data: None,
                created_at: now,
            };
            self.workflow_run_events.lock().await.push(event.clone());
            if let Some(run) = self
                .workflow_runs
                .lock()
                .await
                .iter()
                .find(|run| run.id == run_id)
                .cloned()
            {
                self.queue_workflow_event_deliveries_for_run_event(&run, &event)
                    .await;
            }
            self.workflow_run_logs
                .lock()
                .await
                .push(StoredWorkflowRunLog {
                    id: Uuid::now_v7(),
                    run_id,
                    stream: AutomationTaskLogStream::System,
                    message: "workflow run cancelled".to_string(),
                    created_at: now,
                });
        }
        Ok(Some(task))
    }

    async fn list_automation_task_events_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Vec<StoredAutomationTaskEvent>, SessionStoreError> {
        if self
            .get_automation_task_for_owner(principal, id)
            .await?
            .is_none()
        {
            return Ok(Vec::new());
        }

        let mut events = self
            .automation_task_events
            .lock()
            .await
            .iter()
            .filter(|event| event.task_id == id)
            .cloned()
            .collect::<Vec<_>>();
        events.sort_by(|left, right| left.created_at.cmp(&right.created_at));
        Ok(events)
    }

    async fn list_automation_task_logs_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Vec<StoredAutomationTaskLog>, SessionStoreError> {
        if self
            .get_automation_task_for_owner(principal, id)
            .await?
            .is_none()
        {
            return Ok(Vec::new());
        }

        let mut logs = self
            .automation_task_logs
            .lock()
            .await
            .iter()
            .filter(|log| log.task_id == id)
            .cloned()
            .collect::<Vec<_>>();
        logs.sort_by(|left, right| left.created_at.cmp(&right.created_at));
        Ok(logs)
    }

    async fn transition_automation_task(
        &self,
        id: Uuid,
        request: AutomationTaskTransitionRequest,
    ) -> Result<Option<StoredAutomationTask>, SessionStoreError> {
        let mut tasks = self.automation_tasks.lock().await;
        let Some(task) = tasks.iter_mut().find(|task| task.id == id) else {
            return Ok(None);
        };
        if task.state.is_terminal() {
            return Err(SessionStoreError::Conflict(format!(
                "automation task {id} is already terminal"
            )));
        }
        if !task.state.can_transition_to(request.state) {
            return Err(SessionStoreError::Conflict(format!(
                "automation task {id} cannot transition from {} to {}",
                task.state.as_str(),
                request.state.as_str()
            )));
        }
        let now = Utc::now();
        if matches!(
            request.state,
            AutomationTaskState::Starting
                | AutomationTaskState::Running
                | AutomationTaskState::AwaitingInput
        ) && task.started_at.is_none()
        {
            task.started_at = Some(now);
        }
        if request.state.is_terminal() {
            task.completed_at = Some(now);
        }
        task.state = request.state;
        task.output = request.output;
        task.error = request.error;
        task.artifact_refs = request.artifact_refs;
        task.updated_at = now;
        let task = task.clone();
        drop(tasks);

        if let Some(run) = self
            .workflow_runs
            .lock()
            .await
            .iter_mut()
            .find(|run| run.automation_task_id == id)
        {
            sync_workflow_run_with_task(run, &task);
        }

        self.automation_task_events
            .lock()
            .await
            .push(StoredAutomationTaskEvent {
                id: Uuid::now_v7(),
                task_id: id,
                event_type: request.event_type,
                message: request.event_message,
                data: request.event_data,
                created_at: now,
            });
        Ok(Some(task))
    }

    async fn append_automation_task_log(
        &self,
        id: Uuid,
        stream: AutomationTaskLogStream,
        message: String,
    ) -> Result<Option<StoredAutomationTaskLog>, SessionStoreError> {
        let tasks = self.automation_tasks.lock().await;
        if !tasks.iter().any(|task| task.id == id) {
            return Ok(None);
        }
        drop(tasks);

        let log = StoredAutomationTaskLog {
            id: Uuid::now_v7(),
            task_id: id,
            stream,
            message,
            created_at: Utc::now(),
        };
        self.automation_task_logs.lock().await.push(log.clone());
        Ok(Some(log))
    }

    async fn create_workflow_definition(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistWorkflowDefinitionRequest,
    ) -> Result<StoredWorkflowDefinition, SessionStoreError> {
        let now = Utc::now();
        let workflow = StoredWorkflowDefinition {
            id: Uuid::now_v7(),
            owner_subject: principal.subject.clone(),
            owner_issuer: principal.issuer.clone(),
            name: request.name,
            description: request.description,
            labels: request.labels,
            latest_version: None,
            created_at: now,
            updated_at: now,
        };
        self.workflow_definitions
            .lock()
            .await
            .push(workflow.clone());
        Ok(workflow)
    }

    async fn list_workflow_definitions_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredWorkflowDefinition>, SessionStoreError> {
        let mut workflows = self
            .workflow_definitions
            .lock()
            .await
            .iter()
            .filter(|workflow| {
                workflow.owner_subject == principal.subject
                    && workflow.owner_issuer == principal.issuer
            })
            .cloned()
            .collect::<Vec<_>>();
        workflows.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(workflows)
    }

    async fn get_workflow_definition_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowDefinition>, SessionStoreError> {
        Ok(self
            .workflow_definitions
            .lock()
            .await
            .iter()
            .find(|workflow| {
                workflow.id == id
                    && workflow.owner_subject == principal.subject
                    && workflow.owner_issuer == principal.issuer
            })
            .cloned())
    }

    async fn create_workflow_definition_version(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistWorkflowDefinitionVersionRequest,
    ) -> Result<StoredWorkflowDefinitionVersion, SessionStoreError> {
        let Some(_) = self
            .get_workflow_definition_for_owner(principal, request.workflow_definition_id)
            .await?
        else {
            return Err(SessionStoreError::NotFound(format!(
                "workflow definition {} not found",
                request.workflow_definition_id
            )));
        };

        let mut versions = self.workflow_definition_versions.lock().await;
        if versions.iter().any(|version| {
            version.workflow_definition_id == request.workflow_definition_id
                && version.version == request.version
        }) {
            return Err(SessionStoreError::Conflict(format!(
                "workflow version {} already exists",
                request.version
            )));
        }

        let now = Utc::now();
        let version = StoredWorkflowDefinitionVersion {
            id: Uuid::now_v7(),
            workflow_definition_id: request.workflow_definition_id,
            version: request.version.clone(),
            executor: request.executor,
            entrypoint: request.entrypoint,
            source: request.source,
            input_schema: request.input_schema,
            output_schema: request.output_schema,
            default_session: request.default_session,
            allowed_credential_binding_ids: request.allowed_credential_binding_ids,
            allowed_extension_ids: request.allowed_extension_ids,
            allowed_file_workspace_ids: request.allowed_file_workspace_ids,
            created_at: now,
        };
        versions.push(version.clone());
        drop(versions);

        if let Some(workflow) = self
            .workflow_definitions
            .lock()
            .await
            .iter_mut()
            .find(|workflow| workflow.id == request.workflow_definition_id)
        {
            workflow.latest_version = Some(version.version.clone());
            workflow.updated_at = now;
        }

        Ok(version)
    }

    async fn get_workflow_definition_version_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        workflow_definition_id: Uuid,
        version: &str,
    ) -> Result<Option<StoredWorkflowDefinitionVersion>, SessionStoreError> {
        if self
            .get_workflow_definition_for_owner(principal, workflow_definition_id)
            .await?
            .is_none()
        {
            return Ok(None);
        }
        Ok(self
            .workflow_definition_versions
            .lock()
            .await
            .iter()
            .find(|stored| {
                stored.workflow_definition_id == workflow_definition_id && stored.version == version
            })
            .cloned())
    }

    async fn get_workflow_definition_version_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowDefinitionVersion>, SessionStoreError> {
        Ok(self
            .workflow_definition_versions
            .lock()
            .await
            .iter()
            .find(|version| version.id == id)
            .cloned())
    }

    async fn create_workflow_run(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistWorkflowRunRequest,
    ) -> Result<CreateWorkflowRunResult, SessionStoreError> {
        let Some(task) = self
            .get_automation_task_for_owner(principal, request.automation_task_id)
            .await?
        else {
            return Err(SessionStoreError::NotFound(format!(
                "automation task {} not found",
                request.automation_task_id
            )));
        };
        if task.session_id != request.session_id {
            return Err(SessionStoreError::InvalidRequest(
                "workflow run session_id must match the bound automation task session".to_string(),
            ));
        }
        if self
            .get_workflow_definition_for_owner(principal, request.workflow_definition_id)
            .await?
            .is_none()
        {
            return Err(SessionStoreError::NotFound(format!(
                "workflow definition {} not found",
                request.workflow_definition_id
            )));
        }
        let Some(version) = self
            .workflow_definition_versions
            .lock()
            .await
            .iter()
            .find(|version| version.id == request.workflow_definition_version_id)
            .cloned()
        else {
            return Err(SessionStoreError::NotFound(format!(
                "workflow definition version {} not found",
                request.workflow_definition_version_id
            )));
        };
        if version.workflow_definition_id != request.workflow_definition_id {
            return Err(SessionStoreError::InvalidRequest(
                "workflow run version must belong to the requested workflow definition".to_string(),
            ));
        }

        if let Some(client_request_id) = request.client_request_id.as_deref() {
            let existing_run = {
                let runs = self.workflow_runs.lock().await;
                runs.iter()
                    .find(|run| {
                        run.owner_subject == principal.subject
                            && run.owner_issuer == principal.issuer
                            && run.client_request_id.as_deref() == Some(client_request_id)
                    })
                    .cloned()
            };
            if let Some(existing_run) = existing_run {
                if existing_run.create_request_fingerprint == request.create_request_fingerprint {
                    return Ok(CreateWorkflowRunResult {
                        run: existing_run,
                        created: false,
                    });
                }
                return Err(SessionStoreError::Conflict(format!(
                    "workflow run client_request_id {} is already bound to a different request",
                    client_request_id
                )));
            }
        }

        let now = Utc::now();
        let run = StoredWorkflowRun {
            id: Uuid::now_v7(),
            owner_subject: principal.subject.clone(),
            owner_issuer: principal.issuer.clone(),
            workflow_definition_id: request.workflow_definition_id,
            workflow_definition_version_id: request.workflow_definition_version_id,
            workflow_version: request.workflow_version.clone(),
            session_id: request.session_id,
            automation_task_id: request.automation_task_id,
            source_system: request.source_system.clone(),
            source_reference: request.source_reference.clone(),
            client_request_id: request.client_request_id.clone(),
            create_request_fingerprint: request.create_request_fingerprint.clone(),
            source_snapshot: request.source_snapshot,
            extensions: request.extensions,
            credential_bindings: request.credential_bindings,
            workspace_inputs: request.workspace_inputs,
            produced_files: Vec::new(),
            state: WorkflowRunState::Pending,
            input: request.input,
            output: None,
            error: None,
            artifact_refs: Vec::new(),
            labels: request.labels,
            started_at: None,
            completed_at: None,
            created_at: now,
            updated_at: now,
        };
        self.workflow_runs.lock().await.push(run.clone());
        let event = StoredWorkflowRunEvent {
            id: Uuid::now_v7(),
            run_id: run.id,
            event_type: "workflow_run.created".to_string(),
            message: "workflow run created".to_string(),
            data: Some(serde_json::json!({
                "workflow_definition_id": run.workflow_definition_id,
                "workflow_definition_version_id": run.workflow_definition_version_id,
                "workflow_version": run.workflow_version,
                "automation_task_id": run.automation_task_id,
                "session_id": run.session_id,
                "source_system": run.source_system.clone(),
                "source_reference": run.source_reference.clone(),
                "client_request_id": run.client_request_id.clone(),
            })),
            created_at: now,
        };
        self.workflow_run_events.lock().await.push(event.clone());
        self.queue_workflow_event_deliveries_for_run_event(&run, &event)
            .await;
        Ok(CreateWorkflowRunResult { run, created: true })
    }

    async fn get_workflow_run_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        let Some(run) = self
            .workflow_runs
            .lock()
            .await
            .iter()
            .find(|run| run.id == id)
            .cloned()
        else {
            return Ok(None);
        };
        let Some(task) = self
            .get_automation_task_for_owner(principal, run.automation_task_id)
            .await?
        else {
            return Ok(None);
        };
        if task.session_id != run.session_id {
            return Ok(None);
        }
        Ok(Some(run))
    }

    async fn get_workflow_run_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        Ok(self
            .workflow_runs
            .lock()
            .await
            .iter()
            .find(|run| run.id == id)
            .cloned())
    }

    async fn list_dispatchable_workflow_runs(
        &self,
    ) -> Result<Vec<StoredWorkflowRun>, SessionStoreError> {
        let mut runs = self
            .workflow_runs
            .lock()
            .await
            .iter()
            .filter(|run| {
                matches!(
                    run.state,
                    WorkflowRunState::Pending | WorkflowRunState::Queued
                )
            })
            .cloned()
            .collect::<Vec<_>>();
        runs.sort_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(runs)
    }

    async fn find_workflow_run_by_client_request_id_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        client_request_id: &str,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        let runs = self.workflow_runs.lock().await.clone();
        for run in runs {
            if run.owner_subject == principal.subject
                && run.owner_issuer == principal.issuer
                && run.client_request_id.as_deref() == Some(client_request_id)
            {
                return Ok(Some(run));
            }
        }
        Ok(None)
    }

    async fn create_workflow_event_subscription(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistWorkflowEventSubscriptionRequest,
    ) -> Result<StoredWorkflowEventSubscription, SessionStoreError> {
        let now = Utc::now();
        let subscription = StoredWorkflowEventSubscription {
            id: Uuid::now_v7(),
            owner_subject: principal.subject.clone(),
            owner_issuer: principal.issuer.clone(),
            name: request.name,
            target_url: request.target_url,
            event_types: request.event_types,
            signing_secret: request.signing_secret,
            created_at: now,
            updated_at: now,
        };
        self.workflow_event_subscriptions
            .lock()
            .await
            .push(subscription.clone());
        Ok(subscription)
    }

    async fn list_workflow_event_subscriptions_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredWorkflowEventSubscription>, SessionStoreError> {
        let mut subscriptions = self
            .workflow_event_subscriptions
            .lock()
            .await
            .iter()
            .filter(|subscription| {
                subscription.owner_subject == principal.subject
                    && subscription.owner_issuer == principal.issuer
            })
            .cloned()
            .collect::<Vec<_>>();
        subscriptions.sort_by(|left, right| {
            right
                .created_at
                .cmp(&left.created_at)
                .then_with(|| right.id.cmp(&left.id))
        });
        Ok(subscriptions)
    }

    async fn get_workflow_event_subscription_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowEventSubscription>, SessionStoreError> {
        Ok(self
            .workflow_event_subscriptions
            .lock()
            .await
            .iter()
            .find(|subscription| {
                subscription.id == id
                    && subscription.owner_subject == principal.subject
                    && subscription.owner_issuer == principal.issuer
            })
            .cloned())
    }

    async fn delete_workflow_event_subscription_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowEventSubscription>, SessionStoreError> {
        let mut subscriptions = self.workflow_event_subscriptions.lock().await;
        let Some(index) = subscriptions.iter().position(|subscription| {
            subscription.id == id
                && subscription.owner_subject == principal.subject
                && subscription.owner_issuer == principal.issuer
        }) else {
            return Ok(None);
        };
        let removed = subscriptions.remove(index);
        drop(subscriptions);

        let delivery_ids = {
            let mut deliveries = self.workflow_event_deliveries.lock().await;
            let delivery_ids = deliveries
                .iter()
                .filter(|delivery| delivery.subscription_id == id)
                .map(|delivery| delivery.id)
                .collect::<Vec<_>>();
            deliveries.retain(|delivery| delivery.subscription_id != id);
            delivery_ids
        };
        self.workflow_event_delivery_attempts
            .lock()
            .await
            .retain(|attempt| !delivery_ids.contains(&attempt.delivery_id));
        Ok(Some(removed))
    }

    async fn list_workflow_event_deliveries_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        subscription_id: Uuid,
    ) -> Result<Vec<StoredWorkflowEventDelivery>, SessionStoreError> {
        if self
            .get_workflow_event_subscription_for_owner(principal, subscription_id)
            .await?
            .is_none()
        {
            return Ok(Vec::new());
        }
        let mut deliveries = self
            .workflow_event_deliveries
            .lock()
            .await
            .iter()
            .filter(|delivery| delivery.subscription_id == subscription_id)
            .cloned()
            .collect::<Vec<_>>();
        deliveries.sort_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| left.event_id.cmp(&right.event_id))
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(deliveries)
    }

    async fn list_workflow_event_delivery_attempts_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        subscription_id: Uuid,
    ) -> Result<Vec<StoredWorkflowEventDeliveryAttempt>, SessionStoreError> {
        let deliveries = self
            .list_workflow_event_deliveries_for_owner(principal, subscription_id)
            .await?;
        let delivery_ids = deliveries
            .into_iter()
            .map(|delivery| delivery.id)
            .collect::<Vec<_>>();
        let mut attempts = self
            .workflow_event_delivery_attempts
            .lock()
            .await
            .iter()
            .filter(|attempt| delivery_ids.contains(&attempt.delivery_id))
            .cloned()
            .collect::<Vec<_>>();
        attempts.sort_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(attempts)
    }

    async fn requeue_inflight_workflow_event_deliveries(&self) -> Result<(), SessionStoreError> {
        let now = Utc::now();
        for delivery in self.workflow_event_deliveries.lock().await.iter_mut() {
            if delivery.state == WorkflowEventDeliveryState::Delivering {
                delivery.state = WorkflowEventDeliveryState::Pending;
                delivery.next_attempt_at = Some(now);
                delivery.updated_at = now;
            }
        }
        Ok(())
    }

    async fn claim_due_workflow_event_deliveries(
        &self,
        limit: usize,
        now: DateTime<Utc>,
    ) -> Result<Vec<StoredWorkflowEventDelivery>, SessionStoreError> {
        let mut deliveries = self.workflow_event_deliveries.lock().await;
        let mut due_indexes = deliveries
            .iter()
            .enumerate()
            .filter(|(_, delivery)| {
                delivery.state == WorkflowEventDeliveryState::Pending
                    && delivery
                        .next_attempt_at
                        .map(|value| value <= now)
                        .unwrap_or(true)
            })
            .map(|(index, delivery)| (index, delivery.created_at, delivery.event_id, delivery.id))
            .collect::<Vec<_>>();
        due_indexes.sort_by(|left, right| {
            left.1
                .cmp(&right.1)
                .then_with(|| left.2.cmp(&right.2))
                .then_with(|| left.3.cmp(&right.3))
        });
        let mut due_indexes = due_indexes
            .into_iter()
            .map(|(index, _, _, _)| index)
            .take(limit)
            .collect::<Vec<_>>();
        let mut claimed = Vec::with_capacity(due_indexes.len());
        for index in due_indexes.drain(..) {
            if let Some(delivery) = deliveries.get_mut(index) {
                delivery.state = WorkflowEventDeliveryState::Delivering;
                delivery.updated_at = now;
                claimed.push(delivery.clone());
            }
        }
        Ok(claimed)
    }

    async fn record_workflow_event_delivery_attempt(
        &self,
        delivery_id: Uuid,
        request: RecordWorkflowEventDeliveryAttemptRequest,
    ) -> Result<Option<StoredWorkflowEventDelivery>, SessionStoreError> {
        let now = request.attempted_at;
        let mut deliveries = self.workflow_event_deliveries.lock().await;
        let Some(delivery) = deliveries
            .iter_mut()
            .find(|delivery| delivery.id == delivery_id)
        else {
            return Ok(None);
        };
        delivery.state = request.state;
        delivery.attempt_count = request.attempt_number;
        delivery.next_attempt_at = request.next_attempt_at;
        delivery.last_attempt_at = Some(now);
        delivery.delivered_at = request.delivered_at;
        delivery.last_response_status = request.response_status;
        delivery.last_error = request.error.clone();
        delivery.updated_at = now;
        let updated = delivery.clone();
        drop(deliveries);

        self.workflow_event_delivery_attempts.lock().await.push(
            StoredWorkflowEventDeliveryAttempt {
                id: Uuid::now_v7(),
                delivery_id,
                attempt_number: request.attempt_number,
                response_status: request.response_status,
                error: request.error,
                created_at: now,
            },
        );
        Ok(Some(updated))
    }

    async fn list_workflow_run_events_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Vec<StoredWorkflowRunEvent>, SessionStoreError> {
        if self
            .get_workflow_run_for_owner(principal, id)
            .await?
            .is_none()
        {
            return Ok(Vec::new());
        }
        let mut events = self
            .workflow_run_events
            .lock()
            .await
            .iter()
            .filter(|event| event.run_id == id)
            .cloned()
            .collect::<Vec<_>>();
        events.sort_by(|left, right| left.created_at.cmp(&right.created_at));
        Ok(events)
    }

    async fn list_workflow_run_events(
        &self,
        id: Uuid,
    ) -> Result<Vec<StoredWorkflowRunEvent>, SessionStoreError> {
        let mut events = self
            .workflow_run_events
            .lock()
            .await
            .iter()
            .filter(|event| event.run_id == id)
            .cloned()
            .collect::<Vec<_>>();
        events.sort_by(|left, right| left.created_at.cmp(&right.created_at));
        Ok(events)
    }

    async fn list_workflow_run_logs_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Vec<StoredWorkflowRunLog>, SessionStoreError> {
        if self
            .get_workflow_run_for_owner(principal, id)
            .await?
            .is_none()
        {
            return Ok(Vec::new());
        }

        let mut logs = self
            .workflow_run_logs
            .lock()
            .await
            .iter()
            .filter(|log| log.run_id == id)
            .cloned()
            .collect::<Vec<_>>();
        logs.sort_by(|left, right| left.created_at.cmp(&right.created_at));
        Ok(logs)
    }

    async fn append_workflow_run_event_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        request: PersistWorkflowRunEventRequest,
    ) -> Result<Option<StoredWorkflowRunEvent>, SessionStoreError> {
        if self
            .get_workflow_run_for_owner(principal, id)
            .await?
            .is_none()
        {
            return Ok(None);
        }
        let event = StoredWorkflowRunEvent {
            id: Uuid::now_v7(),
            run_id: id,
            event_type: request.event_type,
            message: request.message,
            data: request.data,
            created_at: Utc::now(),
        };
        self.workflow_run_events.lock().await.push(event.clone());
        let mut updated_run = None;
        {
            let mut runs = self.workflow_runs.lock().await;
            if let Some(run) = runs.iter_mut().find(|run| run.id == id) {
                run.updated_at = event.created_at;
                updated_run = Some(run.clone());
            }
        }
        if let Some(run) = updated_run.as_ref() {
            self.queue_workflow_event_deliveries_for_run_event(run, &event)
                .await;
        }
        Ok(Some(event))
    }

    async fn append_workflow_run_event(
        &self,
        id: Uuid,
        request: PersistWorkflowRunEventRequest,
    ) -> Result<Option<StoredWorkflowRunEvent>, SessionStoreError> {
        if self.get_workflow_run_by_id(id).await?.is_none() {
            return Ok(None);
        }
        let event = StoredWorkflowRunEvent {
            id: Uuid::now_v7(),
            run_id: id,
            event_type: request.event_type,
            message: request.message,
            data: request.data,
            created_at: Utc::now(),
        };
        self.workflow_run_events.lock().await.push(event.clone());
        let mut updated_run = None;
        {
            let mut runs = self.workflow_runs.lock().await;
            if let Some(run) = runs.iter_mut().find(|run| run.id == id) {
                run.updated_at = event.created_at;
                updated_run = Some(run.clone());
            }
        }
        if let Some(run) = updated_run.as_ref() {
            self.queue_workflow_event_deliveries_for_run_event(run, &event)
                .await;
        }
        Ok(Some(event))
    }

    async fn transition_workflow_run(
        &self,
        id: Uuid,
        request: WorkflowRunTransitionRequest,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        let run = self
            .workflow_runs
            .lock()
            .await
            .iter()
            .find(|run| run.id == id)
            .cloned();
        let Some(run) = run else {
            return Ok(None);
        };

        let task_state: AutomationTaskState = request.state.into();
        let task_event_type = automation_task_event_type_for_run_state(request.state).to_string();
        let task_event_message = request.message.clone().unwrap_or_else(|| {
            automation_task_default_message_for_run_state(request.state).to_string()
        });
        let now = Utc::now();

        let task = {
            let mut tasks = self.automation_tasks.lock().await;
            let Some(task) = tasks
                .iter_mut()
                .find(|task| task.id == run.automation_task_id)
            else {
                return Err(SessionStoreError::NotFound(format!(
                    "automation task {} for workflow run {} not found",
                    run.automation_task_id, id
                )));
            };
            if task.state.is_terminal() {
                return Err(SessionStoreError::Conflict(format!(
                    "automation task {} is already terminal",
                    task.id
                )));
            }
            if !task.state.can_transition_to(task_state) {
                return Err(SessionStoreError::Conflict(format!(
                    "automation task {} cannot transition from {} to {}",
                    task.id,
                    task.state.as_str(),
                    task_state.as_str()
                )));
            }
            if matches!(
                task_state,
                AutomationTaskState::Starting
                    | AutomationTaskState::Running
                    | AutomationTaskState::AwaitingInput
            ) && task.started_at.is_none()
            {
                task.started_at = Some(now);
            }
            if task_state.is_terminal() {
                task.completed_at = Some(now);
            }
            task.state = task_state;
            task.output = request.output.clone();
            task.error = request.error.clone();
            task.artifact_refs = request.artifact_refs.clone();
            task.updated_at = now;
            task.clone()
        };

        self.automation_task_events
            .lock()
            .await
            .push(StoredAutomationTaskEvent {
                id: Uuid::now_v7(),
                task_id: task.id,
                event_type: task_event_type,
                message: task_event_message,
                data: request.data.clone(),
                created_at: now,
            });

        let run_message = request
            .message
            .unwrap_or_else(|| workflow_run_default_message(request.state).to_string());
        let run = {
            let mut runs = self.workflow_runs.lock().await;
            let Some(run) = runs.iter_mut().find(|run| run.id == id) else {
                return Ok(None);
            };
            sync_workflow_run_with_task(run, &task);
            run.updated_at = now;
            run.clone()
        };

        let event = StoredWorkflowRunEvent {
            id: Uuid::now_v7(),
            run_id: id,
            event_type: workflow_run_event_type(request.state).to_string(),
            message: run_message,
            data: request.data,
            created_at: now,
        };
        self.workflow_run_events.lock().await.push(event.clone());
        self.queue_workflow_event_deliveries_for_run_event(&run, &event)
            .await;

        Ok(Some(run))
    }

    async fn list_awaiting_input_workflow_runs(
        &self,
    ) -> Result<Vec<StoredWorkflowRun>, SessionStoreError> {
        let mut runs = self
            .workflow_runs
            .lock()
            .await
            .iter()
            .filter(|run| run.state == WorkflowRunState::AwaitingInput)
            .cloned()
            .collect::<Vec<_>>();
        runs.sort_by(|left, right| {
            left.updated_at
                .cmp(&right.updated_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(runs)
    }

    async fn reconcile_workflow_run_from_task(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        let run = self
            .workflow_runs
            .lock()
            .await
            .iter()
            .find(|run| run.id == id)
            .cloned();
        let Some(current_run) = run else {
            return Ok(None);
        };

        let task = self
            .automation_tasks
            .lock()
            .await
            .iter()
            .find(|task| task.id == current_run.automation_task_id)
            .cloned()
            .ok_or_else(|| {
                SessionStoreError::NotFound(format!(
                    "automation task {} for workflow run {} not found",
                    current_run.automation_task_id, id
                ))
            })?;
        if !task.state.is_terminal() {
            return Ok(Some(current_run));
        }

        let target_state: WorkflowRunState = task.state.into();
        let artifact_refs = task.artifact_refs.clone();
        if current_run.state == target_state
            && current_run.output == task.output
            && current_run.error == task.error
            && current_run.artifact_refs == artifact_refs
            && current_run.started_at == task.started_at
            && current_run.completed_at == task.completed_at
        {
            return Ok(Some(current_run));
        }

        let now = Utc::now();
        let run = {
            let mut runs = self.workflow_runs.lock().await;
            let Some(run) = runs.iter_mut().find(|run| run.id == id) else {
                return Ok(None);
            };
            run.state = target_state;
            run.output = task.output.clone();
            run.error = task.error.clone();
            run.artifact_refs = artifact_refs;
            run.started_at = task.started_at;
            run.completed_at = task.completed_at;
            run.updated_at = now;
            run.clone()
        };

        let event = StoredWorkflowRunEvent {
            id: Uuid::now_v7(),
            run_id: id,
            event_type: workflow_run_event_type(target_state).to_string(),
            message: "workflow run reconciled from terminal automation task state".to_string(),
            data: Some(serde_json::json!({
                "reconciled_from": "automation_task"
            })),
            created_at: now,
        };
        self.workflow_run_events.lock().await.push(event.clone());
        self.queue_workflow_event_deliveries_for_run_event(&run, &event)
            .await;
        Ok(Some(run))
    }

    async fn append_workflow_run_log(
        &self,
        id: Uuid,
        request: PersistWorkflowRunLogRequest,
    ) -> Result<Option<StoredWorkflowRunLog>, SessionStoreError> {
        let mut runs = self.workflow_runs.lock().await;
        let Some(run) = runs.iter_mut().find(|run| run.id == id) else {
            return Ok(None);
        };

        let log = StoredWorkflowRunLog {
            id: Uuid::now_v7(),
            run_id: id,
            stream: request.stream,
            message: request.message,
            created_at: Utc::now(),
        };
        run.updated_at = log.created_at;
        drop(runs);

        self.workflow_run_logs.lock().await.push(log.clone());
        Ok(Some(log))
    }

    async fn append_workflow_run_produced_file(
        &self,
        id: Uuid,
        request: PersistWorkflowRunProducedFileRequest,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        let now = Utc::now();
        let produced_file = WorkflowRunProducedFile {
            workspace_id: request.workspace_id,
            file_id: request.file_id,
            file_name: request.file_name,
            media_type: request.media_type,
            byte_count: request.byte_count,
            sha256_hex: request.sha256_hex,
            provenance: request.provenance,
            artifact_ref: request.artifact_ref,
            created_at: now,
        };

        let mut runs = self.workflow_runs.lock().await;
        let Some(run) = runs.iter_mut().find(|run| run.id == id) else {
            return Ok(None);
        };
        if run
            .produced_files
            .iter()
            .any(|file| file.file_id == produced_file.file_id)
        {
            return Err(SessionStoreError::Conflict(format!(
                "workflow run {id} already contains produced file {}",
                produced_file.file_id
            )));
        }
        run.produced_files.push(produced_file.clone());
        run.updated_at = now;
        let updated = run.clone();
        drop(runs);

        let event = StoredWorkflowRunEvent {
            id: Uuid::now_v7(),
            run_id: id,
            event_type: "workflow_run.produced_file_added".to_string(),
            message: format!(
                "workflow run produced file {} stored in workspace {}",
                produced_file.file_id, produced_file.workspace_id
            ),
            data: Some(serde_json::json!({
                "workspace_id": produced_file.workspace_id,
                "file_id": produced_file.file_id,
                "file_name": produced_file.file_name,
            })),
            created_at: now,
        };
        self.workflow_run_events.lock().await.push(event.clone());
        self.queue_workflow_event_deliveries_for_run_event(&updated, &event)
            .await;

        Ok(Some(updated))
    }

    async fn list_workflow_run_log_retention_candidates(
        &self,
        now: DateTime<Utc>,
        retention: ChronoDuration,
    ) -> Result<Vec<WorkflowRunLogRetentionCandidate>, SessionStoreError> {
        let task_logs = self.automation_task_logs.lock().await;
        let run_logs = self.workflow_run_logs.lock().await;
        let mut candidates = self
            .workflow_runs
            .lock()
            .await
            .iter()
            .filter_map(|run| {
                let completed_at = run.completed_at?;
                if completed_at + retention > now {
                    return None;
                }
                let has_logs = run_logs.iter().any(|log| log.run_id == run.id)
                    || task_logs
                        .iter()
                        .any(|log| log.task_id == run.automation_task_id);
                if !has_logs {
                    return None;
                }
                Some(WorkflowRunLogRetentionCandidate {
                    run_id: run.id,
                    automation_task_id: run.automation_task_id,
                    session_id: run.session_id,
                    expires_at: completed_at + retention,
                })
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            left.expires_at
                .cmp(&right.expires_at)
                .then_with(|| left.run_id.cmp(&right.run_id))
        });
        Ok(candidates)
    }

    async fn delete_workflow_run_logs(
        &self,
        run_id: Uuid,
        automation_task_id: Uuid,
    ) -> Result<usize, SessionStoreError> {
        let mut deleted = 0usize;
        {
            let mut logs = self.workflow_run_logs.lock().await;
            let before = logs.len();
            logs.retain(|log| log.run_id != run_id);
            deleted += before - logs.len();
        }
        {
            let mut logs = self.automation_task_logs.lock().await;
            let before = logs.len();
            logs.retain(|log| log.task_id != automation_task_id);
            deleted += before - logs.len();
        }
        if let Some(run) = self
            .workflow_runs
            .lock()
            .await
            .iter_mut()
            .find(|run| run.id == run_id)
        {
            run.updated_at = Utc::now();
        }
        Ok(deleted)
    }

    async fn list_workflow_run_output_retention_candidates(
        &self,
        now: DateTime<Utc>,
        retention: ChronoDuration,
    ) -> Result<Vec<WorkflowRunOutputRetentionCandidate>, SessionStoreError> {
        let mut candidates = self
            .workflow_runs
            .lock()
            .await
            .iter()
            .filter_map(|run| {
                let completed_at = run.completed_at?;
                if run.output.is_none() || completed_at + retention > now {
                    return None;
                }
                Some(WorkflowRunOutputRetentionCandidate {
                    run_id: run.id,
                    session_id: run.session_id,
                    expires_at: completed_at + retention,
                })
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            left.expires_at
                .cmp(&right.expires_at)
                .then_with(|| left.run_id.cmp(&right.run_id))
        });
        Ok(candidates)
    }

    async fn clear_workflow_run_output(
        &self,
        run_id: Uuid,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        let mut runs = self.workflow_runs.lock().await;
        let Some(run) = runs.iter_mut().find(|run| run.id == run_id) else {
            return Ok(None);
        };
        run.output = None;
        run.updated_at = Utc::now();
        Ok(Some(run.clone()))
    }

    async fn create_credential_binding(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistCredentialBindingRequest,
    ) -> Result<StoredCredentialBinding, SessionStoreError> {
        let now = Utc::now();
        let binding = StoredCredentialBinding {
            id: request.id,
            owner_subject: principal.subject.clone(),
            owner_issuer: principal.issuer.clone(),
            name: request.name,
            provider: request.provider,
            external_ref: request.external_ref,
            namespace: request.namespace,
            allowed_origins: request.allowed_origins,
            injection_mode: request.injection_mode,
            totp: request.totp,
            labels: request.labels,
            created_at: now,
            updated_at: now,
        };
        self.credential_bindings.lock().await.push(binding.clone());
        Ok(binding)
    }

    async fn list_credential_bindings_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredCredentialBinding>, SessionStoreError> {
        let mut bindings = self
            .credential_bindings
            .lock()
            .await
            .iter()
            .filter(|binding| {
                binding.owner_subject == principal.subject
                    && binding.owner_issuer == principal.issuer
            })
            .cloned()
            .collect::<Vec<_>>();
        bindings.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(bindings)
    }

    async fn get_credential_binding_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredCredentialBinding>, SessionStoreError> {
        Ok(self
            .credential_bindings
            .lock()
            .await
            .iter()
            .find(|binding| {
                binding.id == id
                    && binding.owner_subject == principal.subject
                    && binding.owner_issuer == principal.issuer
            })
            .cloned())
    }

    async fn create_extension_definition(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistExtensionDefinitionRequest,
    ) -> Result<StoredExtensionDefinition, SessionStoreError> {
        let now = Utc::now();
        let definition = StoredExtensionDefinition {
            id: Uuid::now_v7(),
            owner_subject: principal.subject.clone(),
            owner_issuer: principal.issuer.clone(),
            name: request.name,
            description: request.description,
            enabled: true,
            latest_version: None,
            labels: request.labels,
            created_at: now,
            updated_at: now,
        };
        self.extension_definitions
            .lock()
            .await
            .push(definition.clone());
        Ok(definition)
    }

    async fn list_extension_definitions_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredExtensionDefinition>, SessionStoreError> {
        let mut definitions = self
            .extension_definitions
            .lock()
            .await
            .iter()
            .filter(|definition| {
                definition.owner_subject == principal.subject
                    && definition.owner_issuer == principal.issuer
            })
            .cloned()
            .collect::<Vec<_>>();
        definitions.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(definitions)
    }

    async fn get_extension_definition_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredExtensionDefinition>, SessionStoreError> {
        Ok(self
            .extension_definitions
            .lock()
            .await
            .iter()
            .find(|definition| {
                definition.id == id
                    && definition.owner_subject == principal.subject
                    && definition.owner_issuer == principal.issuer
            })
            .cloned())
    }

    async fn set_extension_definition_enabled_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        enabled: bool,
    ) -> Result<Option<StoredExtensionDefinition>, SessionStoreError> {
        let mut definitions = self.extension_definitions.lock().await;
        let Some(definition) = definitions.iter_mut().find(|definition| {
            definition.id == id
                && definition.owner_subject == principal.subject
                && definition.owner_issuer == principal.issuer
        }) else {
            return Ok(None);
        };
        definition.enabled = enabled;
        definition.updated_at = Utc::now();
        Ok(Some(definition.clone()))
    }

    async fn create_extension_version_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistExtensionVersionRequest,
    ) -> Result<StoredExtensionVersion, SessionStoreError> {
        let mut definitions = self.extension_definitions.lock().await;
        let Some(definition) = definitions.iter_mut().find(|definition| {
            definition.id == request.extension_definition_id
                && definition.owner_subject == principal.subject
                && definition.owner_issuer == principal.issuer
        }) else {
            return Err(SessionStoreError::InvalidRequest(format!(
                "extension {} not found",
                request.extension_definition_id
            )));
        };
        let versions = self.extension_versions.lock().await;
        if versions.iter().any(|version| {
            version.extension_definition_id == request.extension_definition_id
                && version.version == request.version
        }) {
            return Err(SessionStoreError::InvalidRequest(format!(
                "extension {} already has version {}",
                request.extension_definition_id, request.version
            )));
        }
        drop(versions);
        let now = Utc::now();
        let version = StoredExtensionVersion {
            id: Uuid::now_v7(),
            extension_definition_id: request.extension_definition_id,
            version: request.version,
            install_path: request.install_path,
            created_at: now,
        };
        self.extension_versions.lock().await.push(version.clone());
        definition.latest_version = Some(version.version.clone());
        definition.updated_at = now;
        Ok(version)
    }

    async fn get_latest_extension_version_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        extension_definition_id: Uuid,
    ) -> Result<Option<StoredExtensionVersion>, SessionStoreError> {
        let definitions = self.extension_definitions.lock().await;
        if !definitions.iter().any(|definition| {
            definition.id == extension_definition_id
                && definition.owner_subject == principal.subject
                && definition.owner_issuer == principal.issuer
        }) {
            return Ok(None);
        }
        drop(definitions);
        let latest = self
            .extension_versions
            .lock()
            .await
            .iter()
            .filter(|version| version.extension_definition_id == extension_definition_id)
            .cloned()
            .max_by(|left, right| {
                left.created_at
                    .cmp(&right.created_at)
                    .then_with(|| left.id.cmp(&right.id))
            });
        Ok(latest)
    }

    async fn create_file_workspace(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistFileWorkspaceRequest,
    ) -> Result<StoredFileWorkspace, SessionStoreError> {
        let now = Utc::now();
        let workspace = StoredFileWorkspace {
            id: Uuid::now_v7(),
            owner_subject: principal.subject.clone(),
            owner_issuer: principal.issuer.clone(),
            name: request.name,
            description: request.description,
            labels: request.labels,
            created_at: now,
            updated_at: now,
        };
        self.file_workspaces.lock().await.push(workspace.clone());
        Ok(workspace)
    }

    async fn list_file_workspaces_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredFileWorkspace>, SessionStoreError> {
        let mut workspaces = self
            .file_workspaces
            .lock()
            .await
            .iter()
            .filter(|workspace| {
                workspace.owner_subject == principal.subject
                    && workspace.owner_issuer == principal.issuer
            })
            .cloned()
            .collect::<Vec<_>>();
        workspaces.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(workspaces)
    }

    async fn get_file_workspace_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredFileWorkspace>, SessionStoreError> {
        Ok(self
            .file_workspaces
            .lock()
            .await
            .iter()
            .find(|workspace| {
                workspace.id == id
                    && workspace.owner_subject == principal.subject
                    && workspace.owner_issuer == principal.issuer
            })
            .cloned())
    }

    async fn create_file_workspace_file_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistFileWorkspaceFileRequest,
    ) -> Result<StoredFileWorkspaceFile, SessionStoreError> {
        let Some(workspace) = self
            .get_file_workspace_for_owner(principal, request.workspace_id)
            .await?
        else {
            return Err(SessionStoreError::NotFound(format!(
                "file workspace {} not found",
                request.workspace_id
            )));
        };

        let now = Utc::now();
        let file = StoredFileWorkspaceFile {
            id: request.id,
            workspace_id: workspace.id,
            name: request.name,
            media_type: request.media_type,
            byte_count: request.byte_count,
            sha256_hex: request.sha256_hex,
            provenance: request.provenance,
            artifact_ref: request.artifact_ref,
            created_at: now,
            updated_at: now,
        };
        self.file_workspace_files.lock().await.push(file.clone());
        Ok(file)
    }

    async fn list_file_workspace_files_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        workspace_id: Uuid,
    ) -> Result<Vec<StoredFileWorkspaceFile>, SessionStoreError> {
        if self
            .get_file_workspace_for_owner(principal, workspace_id)
            .await?
            .is_none()
        {
            return Ok(Vec::new());
        }

        let mut files = self
            .file_workspace_files
            .lock()
            .await
            .iter()
            .filter(|file| file.workspace_id == workspace_id)
            .cloned()
            .collect::<Vec<_>>();
        files.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(files)
    }

    async fn get_file_workspace_file_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        workspace_id: Uuid,
        file_id: Uuid,
    ) -> Result<Option<StoredFileWorkspaceFile>, SessionStoreError> {
        if self
            .get_file_workspace_for_owner(principal, workspace_id)
            .await?
            .is_none()
        {
            return Ok(None);
        }

        Ok(self
            .file_workspace_files
            .lock()
            .await
            .iter()
            .find(|file| file.workspace_id == workspace_id && file.id == file_id)
            .cloned())
    }

    async fn delete_file_workspace_file_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        workspace_id: Uuid,
        file_id: Uuid,
    ) -> Result<Option<StoredFileWorkspaceFile>, SessionStoreError> {
        if self
            .get_file_workspace_for_owner(principal, workspace_id)
            .await?
            .is_none()
        {
            return Ok(None);
        }

        let mut files = self.file_workspace_files.lock().await;
        let Some(index) = files
            .iter()
            .position(|file| file.workspace_id == workspace_id && file.id == file_id)
        else {
            return Ok(None);
        };
        Ok(Some(files.remove(index)))
    }

    async fn create_recording_for_session(
        &self,
        session_id: Uuid,
        format: SessionRecordingFormat,
        previous_recording_id: Option<Uuid>,
    ) -> Result<StoredSessionRecording, SessionStoreError> {
        let mut recordings = self.recordings.lock().await;
        if let Some(active) = recordings
            .iter()
            .find(|recording| recording.session_id == session_id && recording.state.is_active())
        {
            return Err(SessionStoreError::Conflict(format!(
                "session {session_id} already has active recording {}",
                active.id
            )));
        }

        let now = Utc::now();
        let recording = StoredSessionRecording {
            id: Uuid::now_v7(),
            session_id,
            previous_recording_id,
            state: SessionRecordingState::Recording,
            format,
            mime_type: Some(recording_mime_type(format).to_string()),
            bytes: None,
            duration_ms: None,
            error: None,
            termination_reason: None,
            artifact_ref: None,
            started_at: now,
            completed_at: None,
            created_at: now,
            updated_at: now,
        };
        recordings.push(recording.clone());
        Ok(recording)
    }

    async fn list_recordings_for_session(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<StoredSessionRecording>, SessionStoreError> {
        let mut recordings = self
            .recordings
            .lock()
            .await
            .iter()
            .filter(|recording| recording.session_id == session_id)
            .cloned()
            .collect::<Vec<_>>();
        recordings.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(recordings)
    }

    async fn get_recording_for_session(
        &self,
        session_id: Uuid,
        recording_id: Uuid,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        Ok(self
            .recordings
            .lock()
            .await
            .iter()
            .find(|recording| recording.session_id == session_id && recording.id == recording_id)
            .cloned())
    }

    async fn get_latest_recording_for_session(
        &self,
        session_id: Uuid,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        Ok(self
            .recordings
            .lock()
            .await
            .iter()
            .filter(|recording| recording.session_id == session_id)
            .max_by(|left, right| {
                left.updated_at
                    .cmp(&right.updated_at)
                    .then_with(|| left.created_at.cmp(&right.created_at))
            })
            .cloned())
    }

    async fn list_recording_artifact_retention_candidates(
        &self,
        now: DateTime<Utc>,
    ) -> Result<Vec<RecordingArtifactRetentionCandidate>, SessionStoreError> {
        let sessions = self.sessions.lock().await;
        let session_retention = sessions
            .iter()
            .filter_map(|session| {
                session
                    .recording
                    .retention_sec
                    .map(|retention| (session.id, retention))
            })
            .collect::<HashMap<_, _>>();
        let recordings = self.recordings.lock().await;
        let mut candidates = recordings
            .iter()
            .filter_map(|recording| {
                if recording.state != SessionRecordingState::Ready {
                    return None;
                }
                let artifact_ref = recording.artifact_ref.clone()?;
                let completed_at = recording.completed_at?;
                let retention_sec = *session_retention.get(&recording.session_id)?;
                let expires_at = completed_at + ChronoDuration::seconds(i64::from(retention_sec));
                if expires_at > now {
                    return None;
                }
                Some(RecordingArtifactRetentionCandidate {
                    session_id: recording.session_id,
                    recording_id: recording.id,
                    artifact_ref,
                    expires_at,
                })
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| left.expires_at.cmp(&right.expires_at));
        Ok(candidates)
    }

    async fn stop_recording_for_session(
        &self,
        session_id: Uuid,
        recording_id: Uuid,
        termination_reason: SessionRecordingTerminationReason,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        let mut recordings = self.recordings.lock().await;
        let Some(recording) = recordings
            .iter_mut()
            .find(|recording| recording.session_id == session_id && recording.id == recording_id)
        else {
            return Ok(None);
        };

        if !recording.state.is_active() {
            return Err(SessionStoreError::Conflict(format!(
                "recording {recording_id} is not active"
            )));
        }

        recording.state = SessionRecordingState::Finalizing;
        recording.termination_reason = Some(termination_reason);
        recording.updated_at = Utc::now();
        Ok(Some(recording.clone()))
    }

    async fn complete_recording_for_session(
        &self,
        session_id: Uuid,
        recording_id: Uuid,
        request: PersistCompletedSessionRecordingRequest,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        let mut recordings = self.recordings.lock().await;
        let Some(recording) = recordings
            .iter_mut()
            .find(|recording| recording.session_id == session_id && recording.id == recording_id)
        else {
            return Ok(None);
        };

        if !recording.state.is_active() {
            return Err(SessionStoreError::Conflict(format!(
                "recording {recording_id} is not active"
            )));
        }

        let now = Utc::now();
        recording.state = SessionRecordingState::Ready;
        recording.artifact_ref = Some(request.artifact_ref);
        recording.mime_type = request
            .mime_type
            .or_else(|| recording.mime_type.clone())
            .or_else(|| Some(recording_mime_type(recording.format).to_string()));
        recording.bytes = request.bytes;
        recording.duration_ms = request.duration_ms;
        recording.error = None;
        recording.completed_at = Some(now);
        recording.updated_at = now;
        Ok(Some(recording.clone()))
    }

    async fn clear_recording_artifact_path(
        &self,
        session_id: Uuid,
        recording_id: Uuid,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        let mut recordings = self.recordings.lock().await;
        let Some(recording) = recordings
            .iter_mut()
            .find(|recording| recording.session_id == session_id && recording.id == recording_id)
        else {
            return Ok(None);
        };

        recording.artifact_ref = None;
        recording.updated_at = Utc::now();
        Ok(Some(recording.clone()))
    }

    async fn fail_recording_for_session(
        &self,
        session_id: Uuid,
        recording_id: Uuid,
        request: FailSessionRecordingRequest,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        let mut recordings = self.recordings.lock().await;
        let Some(recording) = recordings
            .iter_mut()
            .find(|recording| recording.session_id == session_id && recording.id == recording_id)
        else {
            return Ok(None);
        };

        if matches!(recording.state, SessionRecordingState::Ready) {
            return Err(SessionStoreError::Conflict(format!(
                "recording {recording_id} is already complete"
            )));
        }

        let now = Utc::now();
        recording.state = SessionRecordingState::Failed;
        recording.error = Some(request.error);
        recording.termination_reason = request.termination_reason;
        recording.completed_at = Some(now);
        recording.updated_at = now;
        Ok(Some(recording.clone()))
    }

    async fn upsert_runtime_assignment(
        &self,
        assignment: PersistedSessionRuntimeAssignment,
    ) -> Result<(), SessionStoreError> {
        self.runtime_assignments
            .lock()
            .await
            .insert(assignment.session_id, assignment);
        Ok(())
    }

    async fn clear_runtime_assignment(&self, id: Uuid) -> Result<(), SessionStoreError> {
        self.runtime_assignments.lock().await.remove(&id);
        Ok(())
    }

    async fn upsert_recording_worker_assignment(
        &self,
        assignment: PersistedSessionRecordingWorkerAssignment,
    ) -> Result<(), SessionStoreError> {
        self.recording_worker_assignments
            .lock()
            .await
            .insert(assignment.session_id, assignment);
        Ok(())
    }

    async fn clear_recording_worker_assignment(&self, id: Uuid) -> Result<(), SessionStoreError> {
        self.recording_worker_assignments.lock().await.remove(&id);
        Ok(())
    }

    async fn get_recording_worker_assignment(
        &self,
        id: Uuid,
    ) -> Result<Option<PersistedSessionRecordingWorkerAssignment>, SessionStoreError> {
        Ok(self
            .recording_worker_assignments
            .lock()
            .await
            .get(&id)
            .cloned())
    }

    async fn list_recording_worker_assignments(
        &self,
    ) -> Result<Vec<PersistedSessionRecordingWorkerAssignment>, SessionStoreError> {
        let assignments = self.recording_worker_assignments.lock().await;
        let mut values = assignments.values().cloned().collect::<Vec<_>>();
        values.sort_by_key(|assignment| assignment.session_id);
        Ok(values)
    }

    async fn upsert_workflow_run_worker_assignment(
        &self,
        assignment: PersistedWorkflowRunWorkerAssignment,
    ) -> Result<(), SessionStoreError> {
        self.workflow_run_worker_assignments
            .lock()
            .await
            .insert(assignment.run_id, assignment);
        Ok(())
    }

    async fn clear_workflow_run_worker_assignment(
        &self,
        run_id: Uuid,
    ) -> Result<(), SessionStoreError> {
        self.workflow_run_worker_assignments
            .lock()
            .await
            .remove(&run_id);
        Ok(())
    }

    async fn get_workflow_run_worker_assignment(
        &self,
        run_id: Uuid,
    ) -> Result<Option<PersistedWorkflowRunWorkerAssignment>, SessionStoreError> {
        Ok(self
            .workflow_run_worker_assignments
            .lock()
            .await
            .get(&run_id)
            .cloned())
    }

    async fn list_workflow_run_worker_assignments(
        &self,
    ) -> Result<Vec<PersistedWorkflowRunWorkerAssignment>, SessionStoreError> {
        let assignments = self.workflow_run_worker_assignments.lock().await;
        let mut values = assignments.values().cloned().collect::<Vec<_>>();
        values.sort_by_key(|assignment| assignment.run_id);
        Ok(values)
    }

    async fn list_runtime_assignments(
        &self,
        runtime_binding: &str,
    ) -> Result<Vec<PersistedSessionRuntimeAssignment>, SessionStoreError> {
        let assignments = self.runtime_assignments.lock().await;
        let mut values = assignments
            .values()
            .filter(|assignment| assignment.runtime_binding == runtime_binding)
            .cloned()
            .collect::<Vec<_>>();
        values.sort_by_key(|assignment| assignment.session_id);
        Ok(values)
    }

    async fn mark_session_ready_after_runtime_loss(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let mut sessions = self.sessions.lock().await;
        let Some(session) = sessions.iter_mut().find(|session| session.id == id) else {
            return Ok(None);
        };

        if session.state.is_runtime_candidate() {
            session.state = SessionLifecycleState::Ready;
            session.updated_at = Utc::now();
        }

        Ok(Some(session.clone()))
    }
}

struct PostgresSessionStore {
    client: Mutex<Client>,
    config: SessionStoreConfig,
}

impl PostgresSessionStore {
    async fn migrate(&self) -> Result<(), SessionStoreError> {
        self.client
            .lock()
            .await
            .batch_execute(
                r#"
                CREATE TABLE IF NOT EXISTS control_sessions (
                    id UUID PRIMARY KEY,
                    owner_subject TEXT NOT NULL,
                    owner_issuer TEXT NOT NULL,
                    owner_display_name TEXT NULL,
                    automation_owner_client_id TEXT NULL,
                    automation_owner_issuer TEXT NULL,
                    automation_owner_display_name TEXT NULL,
                    state TEXT NOT NULL,
                    template_id TEXT NULL,
                    owner_mode TEXT NOT NULL,
                    viewport_width INTEGER NOT NULL CHECK (viewport_width > 0 AND viewport_width <= 65535),
                    viewport_height INTEGER NOT NULL CHECK (viewport_height > 0 AND viewport_height <= 65535),
                    idle_timeout_sec INTEGER NULL CHECK (idle_timeout_sec IS NULL OR idle_timeout_sec > 0),
                    labels JSONB NOT NULL DEFAULT '{}'::jsonb,
                    integration_context JSONB NULL,
                    extensions JSONB NOT NULL DEFAULT '[]'::jsonb,
                    recording JSONB NOT NULL DEFAULT '{"mode":"disabled","format":"webm","retention_sec":null}'::jsonb,
                    runtime_binding TEXT NOT NULL DEFAULT 'legacy_single_session',
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    stopped_at TIMESTAMPTZ NULL
                );

                CREATE INDEX IF NOT EXISTS idx_control_sessions_owner_created
                    ON control_sessions (owner_subject, owner_issuer, created_at DESC);

                CREATE INDEX IF NOT EXISTS idx_control_sessions_runtime_state
                    ON control_sessions (runtime_binding, state, created_at DESC);

                CREATE TABLE IF NOT EXISTS control_session_runtimes (
                    session_id UUID PRIMARY KEY REFERENCES control_sessions(id) ON DELETE CASCADE,
                    runtime_binding TEXT NOT NULL,
                    status TEXT NOT NULL,
                    agent_socket_path TEXT NOT NULL,
                    container_name TEXT NULL,
                    cdp_endpoint TEXT NULL,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                );

                CREATE INDEX IF NOT EXISTS idx_control_session_runtimes_binding_updated
                    ON control_session_runtimes (runtime_binding, updated_at DESC);

                CREATE TABLE IF NOT EXISTS control_session_recordings (
                    id UUID PRIMARY KEY,
                    session_id UUID NOT NULL REFERENCES control_sessions(id) ON DELETE CASCADE,
                    previous_recording_id UUID NULL REFERENCES control_session_recordings(id) ON DELETE SET NULL,
                    state TEXT NOT NULL,
                    format TEXT NOT NULL,
                    mime_type TEXT NULL,
                    byte_count BIGINT NULL CHECK (byte_count IS NULL OR byte_count >= 0),
                    duration_ms BIGINT NULL CHECK (duration_ms IS NULL OR duration_ms >= 0),
                    error TEXT NULL,
                    termination_reason TEXT NULL,
                    artifact_path TEXT NULL,
                    started_at TIMESTAMPTZ NOT NULL,
                    completed_at TIMESTAMPTZ NULL,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                );

                CREATE INDEX IF NOT EXISTS idx_control_session_recordings_session_created
                    ON control_session_recordings (session_id, created_at DESC);

                CREATE TABLE IF NOT EXISTS control_session_recording_workers (
                    session_id UUID PRIMARY KEY REFERENCES control_sessions(id) ON DELETE CASCADE,
                    recording_id UUID NOT NULL REFERENCES control_session_recordings(id) ON DELETE CASCADE,
                    status TEXT NOT NULL,
                    process_id BIGINT NULL,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                );

                CREATE INDEX IF NOT EXISTS idx_control_session_recording_workers_updated
                    ON control_session_recording_workers (updated_at DESC);

                CREATE TABLE IF NOT EXISTS control_automation_tasks (
                    id UUID PRIMARY KEY,
                    owner_subject TEXT NOT NULL,
                    owner_issuer TEXT NOT NULL,
                    owner_display_name TEXT NULL,
                    display_name TEXT NULL,
                    executor TEXT NOT NULL,
                    state TEXT NOT NULL,
                    session_id UUID NOT NULL REFERENCES control_sessions(id) ON DELETE CASCADE,
                    session_source TEXT NOT NULL,
                    input JSONB NULL,
                    output JSONB NULL,
                    error TEXT NULL,
                    artifact_refs JSONB NOT NULL DEFAULT '[]'::jsonb,
                    labels JSONB NOT NULL DEFAULT '{}'::jsonb,
                    cancel_requested_at TIMESTAMPTZ NULL,
                    started_at TIMESTAMPTZ NULL,
                    completed_at TIMESTAMPTZ NULL,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                );

                CREATE INDEX IF NOT EXISTS idx_control_automation_tasks_owner_created
                    ON control_automation_tasks (owner_subject, owner_issuer, created_at DESC);

                CREATE INDEX IF NOT EXISTS idx_control_automation_tasks_session_created
                    ON control_automation_tasks (session_id, created_at DESC);

                CREATE TABLE IF NOT EXISTS control_automation_task_events (
                    id UUID PRIMARY KEY,
                    task_id UUID NOT NULL REFERENCES control_automation_tasks(id) ON DELETE CASCADE,
                    event_type TEXT NOT NULL,
                    message TEXT NOT NULL,
                    data JSONB NULL,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                );

                CREATE INDEX IF NOT EXISTS idx_control_automation_task_events_task_created
                    ON control_automation_task_events (task_id, created_at ASC);

                CREATE TABLE IF NOT EXISTS control_automation_task_logs (
                    id UUID PRIMARY KEY,
                    task_id UUID NOT NULL REFERENCES control_automation_tasks(id) ON DELETE CASCADE,
                    stream TEXT NOT NULL,
                    message TEXT NOT NULL,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                );

                CREATE INDEX IF NOT EXISTS idx_control_automation_task_logs_task_created
                    ON control_automation_task_logs (task_id, created_at ASC);

                CREATE TABLE IF NOT EXISTS control_workflow_definitions (
                    id UUID PRIMARY KEY,
                    owner_subject TEXT NOT NULL,
                    owner_issuer TEXT NOT NULL,
                    owner_display_name TEXT NULL,
                    name TEXT NOT NULL,
                    description TEXT NULL,
                    labels JSONB NOT NULL DEFAULT '{}'::jsonb,
                    latest_version TEXT NULL,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                );

                CREATE INDEX IF NOT EXISTS idx_control_workflow_definitions_owner_created
                    ON control_workflow_definitions (owner_subject, owner_issuer, created_at DESC);

                CREATE TABLE IF NOT EXISTS control_workflow_definition_versions (
                    id UUID PRIMARY KEY,
                    workflow_definition_id UUID NOT NULL REFERENCES control_workflow_definitions(id) ON DELETE CASCADE,
                    version TEXT NOT NULL,
                    executor TEXT NOT NULL,
                    entrypoint TEXT NOT NULL,
                    source JSONB NULL,
                    input_schema JSONB NULL,
                    output_schema JSONB NULL,
                    default_session JSONB NULL,
                    allowed_credential_binding_ids JSONB NOT NULL DEFAULT '[]'::jsonb,
                    allowed_extension_ids JSONB NOT NULL DEFAULT '[]'::jsonb,
                    allowed_file_workspace_ids JSONB NOT NULL DEFAULT '[]'::jsonb,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    UNIQUE (workflow_definition_id, version)
                );

                CREATE INDEX IF NOT EXISTS idx_control_workflow_definition_versions_workflow_created
                    ON control_workflow_definition_versions (workflow_definition_id, created_at DESC);

                CREATE TABLE IF NOT EXISTS control_credential_bindings (
                    id UUID PRIMARY KEY,
                    owner_subject TEXT NOT NULL,
                    owner_issuer TEXT NOT NULL,
                    name TEXT NOT NULL,
                    provider TEXT NOT NULL,
                    external_ref TEXT NOT NULL,
                    namespace TEXT NULL,
                    allowed_origins JSONB NOT NULL DEFAULT '[]'::jsonb,
                    injection_mode TEXT NOT NULL,
                    totp JSONB NULL,
                    labels JSONB NOT NULL DEFAULT '{}'::jsonb,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                );

                CREATE INDEX IF NOT EXISTS idx_control_credential_bindings_owner_created
                    ON control_credential_bindings (owner_subject, owner_issuer, created_at DESC);

                CREATE TABLE IF NOT EXISTS control_extensions (
                    id UUID PRIMARY KEY,
                    owner_subject TEXT NOT NULL,
                    owner_issuer TEXT NOT NULL,
                    name TEXT NOT NULL,
                    description TEXT NULL,
                    enabled BOOLEAN NOT NULL DEFAULT TRUE,
                    latest_version TEXT NULL,
                    labels JSONB NOT NULL DEFAULT '{}'::jsonb,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                );

                CREATE INDEX IF NOT EXISTS idx_control_extensions_owner_created
                    ON control_extensions (owner_subject, owner_issuer, created_at DESC);

                CREATE TABLE IF NOT EXISTS control_extension_versions (
                    id UUID PRIMARY KEY,
                    extension_definition_id UUID NOT NULL REFERENCES control_extensions(id) ON DELETE CASCADE,
                    version TEXT NOT NULL,
                    install_path TEXT NOT NULL,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    UNIQUE (extension_definition_id, version)
                );

                CREATE INDEX IF NOT EXISTS idx_control_extension_versions_extension_created
                    ON control_extension_versions (extension_definition_id, created_at DESC);

                CREATE TABLE IF NOT EXISTS control_workflow_runs (
                    id UUID PRIMARY KEY,
                    owner_subject TEXT NOT NULL,
                    owner_issuer TEXT NOT NULL,
                    workflow_definition_id UUID NOT NULL REFERENCES control_workflow_definitions(id) ON DELETE CASCADE,
                    workflow_definition_version_id UUID NOT NULL REFERENCES control_workflow_definition_versions(id) ON DELETE RESTRICT,
                    workflow_version TEXT NOT NULL,
                    session_id UUID NOT NULL REFERENCES control_sessions(id) ON DELETE CASCADE,
                    automation_task_id UUID NOT NULL REFERENCES control_automation_tasks(id) ON DELETE CASCADE,
                    state TEXT NOT NULL DEFAULT 'pending',
                    source_system TEXT NULL,
                    source_reference TEXT NULL,
                    client_request_id TEXT NULL,
                    create_request_fingerprint TEXT NULL,
                    source_snapshot JSONB NULL,
                    extensions JSONB NOT NULL DEFAULT '[]'::jsonb,
                    credential_bindings JSONB NOT NULL DEFAULT '[]'::jsonb,
                    workspace_inputs JSONB NOT NULL DEFAULT '[]'::jsonb,
                    produced_files JSONB NOT NULL DEFAULT '[]'::jsonb,
                    input JSONB NULL,
                    output JSONB NULL,
                    error TEXT NULL,
                    artifact_refs JSONB NOT NULL DEFAULT '[]'::jsonb,
                    labels JSONB NOT NULL DEFAULT '{}'::jsonb,
                    started_at TIMESTAMPTZ NULL,
                    completed_at TIMESTAMPTZ NULL,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                );

                CREATE INDEX IF NOT EXISTS idx_control_workflow_runs_definition_created
                    ON control_workflow_runs (workflow_definition_id, created_at DESC);

                CREATE INDEX IF NOT EXISTS idx_control_workflow_runs_task
                    ON control_workflow_runs (automation_task_id);

                CREATE TABLE IF NOT EXISTS control_workflow_run_workers (
                    run_id UUID PRIMARY KEY REFERENCES control_workflow_runs(id) ON DELETE CASCADE,
                    session_id UUID NOT NULL REFERENCES control_sessions(id) ON DELETE CASCADE,
                    automation_task_id UUID NOT NULL REFERENCES control_automation_tasks(id) ON DELETE CASCADE,
                    status TEXT NOT NULL,
                    process_id BIGINT NULL,
                    container_name TEXT NULL,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                );

                CREATE INDEX IF NOT EXISTS idx_control_workflow_run_workers_updated
                    ON control_workflow_run_workers (updated_at DESC);

                CREATE TABLE IF NOT EXISTS control_workflow_run_events (
                    id UUID PRIMARY KEY,
                    run_id UUID NOT NULL REFERENCES control_workflow_runs(id) ON DELETE CASCADE,
                    event_type TEXT NOT NULL,
                    message TEXT NOT NULL,
                    data JSONB NULL,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                );

                CREATE INDEX IF NOT EXISTS idx_control_workflow_run_events_run_created
                    ON control_workflow_run_events (run_id, created_at ASC);

                CREATE TABLE IF NOT EXISTS control_workflow_run_logs (
                    id UUID PRIMARY KEY,
                    run_id UUID NOT NULL REFERENCES control_workflow_runs(id) ON DELETE CASCADE,
                    stream TEXT NOT NULL,
                    message TEXT NOT NULL,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                );

                CREATE INDEX IF NOT EXISTS idx_control_workflow_run_logs_run_created
                    ON control_workflow_run_logs (run_id, created_at ASC);

                CREATE TABLE IF NOT EXISTS control_workflow_event_subscriptions (
                    id UUID PRIMARY KEY,
                    owner_subject TEXT NOT NULL,
                    owner_issuer TEXT NOT NULL,
                    name TEXT NOT NULL,
                    target_url TEXT NOT NULL,
                    event_types JSONB NOT NULL DEFAULT '[]'::jsonb,
                    signing_secret TEXT NOT NULL,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                );

                CREATE INDEX IF NOT EXISTS idx_control_workflow_event_subscriptions_owner_created
                    ON control_workflow_event_subscriptions (owner_subject, owner_issuer, created_at DESC);

                CREATE TABLE IF NOT EXISTS control_workflow_event_deliveries (
                    id UUID PRIMARY KEY,
                    subscription_id UUID NOT NULL REFERENCES control_workflow_event_subscriptions(id) ON DELETE CASCADE,
                    run_id UUID NOT NULL REFERENCES control_workflow_runs(id) ON DELETE CASCADE,
                    event_id UUID NOT NULL REFERENCES control_workflow_run_events(id) ON DELETE CASCADE,
                    event_type TEXT NOT NULL,
                    target_url TEXT NOT NULL,
                    signing_secret TEXT NOT NULL,
                    payload JSONB NOT NULL,
                    state TEXT NOT NULL,
                    attempt_count INTEGER NOT NULL DEFAULT 0,
                    next_attempt_at TIMESTAMPTZ NULL,
                    last_attempt_at TIMESTAMPTZ NULL,
                    delivered_at TIMESTAMPTZ NULL,
                    last_response_status INTEGER NULL,
                    last_error TEXT NULL,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                );

                CREATE INDEX IF NOT EXISTS idx_control_workflow_event_deliveries_subscription_created
                    ON control_workflow_event_deliveries (subscription_id, created_at ASC);

                CREATE INDEX IF NOT EXISTS idx_control_workflow_event_deliveries_due
                    ON control_workflow_event_deliveries (state, next_attempt_at ASC, created_at ASC);

                CREATE TABLE IF NOT EXISTS control_workflow_event_delivery_attempts (
                    id UUID PRIMARY KEY,
                    delivery_id UUID NOT NULL REFERENCES control_workflow_event_deliveries(id) ON DELETE CASCADE,
                    attempt_number INTEGER NOT NULL,
                    response_status INTEGER NULL,
                    error TEXT NULL,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                );

                CREATE INDEX IF NOT EXISTS idx_control_workflow_event_delivery_attempts_delivery_created
                    ON control_workflow_event_delivery_attempts (delivery_id, created_at ASC);

                CREATE TABLE IF NOT EXISTS control_file_workspaces (
                    id UUID PRIMARY KEY,
                    owner_subject TEXT NOT NULL,
                    owner_issuer TEXT NOT NULL,
                    name TEXT NOT NULL,
                    description TEXT NULL,
                    labels JSONB NOT NULL DEFAULT '{}'::jsonb,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                );

                CREATE INDEX IF NOT EXISTS idx_control_file_workspaces_owner_created
                    ON control_file_workspaces (owner_subject, owner_issuer, created_at DESC);

                CREATE TABLE IF NOT EXISTS control_file_workspace_files (
                    id UUID PRIMARY KEY,
                    workspace_id UUID NOT NULL REFERENCES control_file_workspaces(id) ON DELETE CASCADE,
                    name TEXT NOT NULL,
                    media_type TEXT NULL,
                    byte_count BIGINT NOT NULL CHECK (byte_count >= 0),
                    sha256_hex TEXT NOT NULL,
                    provenance JSONB NULL,
                    artifact_ref TEXT NOT NULL,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                );

                CREATE INDEX IF NOT EXISTS idx_control_file_workspace_files_workspace_created
                    ON control_file_workspace_files (workspace_id, created_at DESC);

                ALTER TABLE control_sessions
                    ADD COLUMN IF NOT EXISTS automation_owner_client_id TEXT NULL;
                ALTER TABLE control_sessions
                    ADD COLUMN IF NOT EXISTS automation_owner_issuer TEXT NULL;
                ALTER TABLE control_sessions
                    ADD COLUMN IF NOT EXISTS automation_owner_display_name TEXT NULL;
                ALTER TABLE control_sessions
                    ADD COLUMN IF NOT EXISTS extensions JSONB NOT NULL DEFAULT '[]'::jsonb;
                ALTER TABLE control_sessions
                    ADD COLUMN IF NOT EXISTS recording JSONB NOT NULL DEFAULT '{"mode":"disabled","format":"webm","retention_sec":null}'::jsonb;
                ALTER TABLE control_session_recordings
                    ADD COLUMN IF NOT EXISTS previous_recording_id UUID NULL REFERENCES control_session_recordings(id) ON DELETE SET NULL;
                ALTER TABLE control_session_recordings
                    ADD COLUMN IF NOT EXISTS termination_reason TEXT NULL;
                ALTER TABLE control_workflow_runs
                    ADD COLUMN IF NOT EXISTS owner_subject TEXT NULL;
                ALTER TABLE control_workflow_runs
                    ADD COLUMN IF NOT EXISTS owner_issuer TEXT NULL;
                ALTER TABLE control_workflow_runs
                    ADD COLUMN IF NOT EXISTS state TEXT NOT NULL DEFAULT 'pending';
                ALTER TABLE control_workflow_runs
                    ADD COLUMN IF NOT EXISTS source_system TEXT NULL;
                ALTER TABLE control_workflow_runs
                    ADD COLUMN IF NOT EXISTS source_reference TEXT NULL;
                ALTER TABLE control_workflow_runs
                    ADD COLUMN IF NOT EXISTS client_request_id TEXT NULL;
                ALTER TABLE control_workflow_runs
                    ADD COLUMN IF NOT EXISTS create_request_fingerprint TEXT NULL;
                ALTER TABLE control_workflow_runs
                    ADD COLUMN IF NOT EXISTS source_snapshot JSONB NULL;
                ALTER TABLE control_workflow_runs
                    ADD COLUMN IF NOT EXISTS extensions JSONB NOT NULL DEFAULT '[]'::jsonb;
                ALTER TABLE control_workflow_runs
                    ADD COLUMN IF NOT EXISTS credential_bindings JSONB NOT NULL DEFAULT '[]'::jsonb;
                ALTER TABLE control_workflow_runs
                    ADD COLUMN IF NOT EXISTS workspace_inputs JSONB NOT NULL DEFAULT '[]'::jsonb;
                ALTER TABLE control_workflow_runs
                    ADD COLUMN IF NOT EXISTS produced_files JSONB NOT NULL DEFAULT '[]'::jsonb;
                ALTER TABLE control_workflow_runs
                    ADD COLUMN IF NOT EXISTS output JSONB NULL;
                ALTER TABLE control_workflow_runs
                    ADD COLUMN IF NOT EXISTS error TEXT NULL;
                ALTER TABLE control_workflow_runs
                    ADD COLUMN IF NOT EXISTS artifact_refs JSONB NOT NULL DEFAULT '[]'::jsonb;
                ALTER TABLE control_workflow_runs
                    ADD COLUMN IF NOT EXISTS started_at TIMESTAMPTZ NULL;
                ALTER TABLE control_workflow_runs
                    ADD COLUMN IF NOT EXISTS completed_at TIMESTAMPTZ NULL;
                UPDATE control_workflow_runs run
                SET owner_subject = task.owner_subject,
                    owner_issuer = task.owner_issuer
                FROM control_automation_tasks task
                WHERE task.id = run.automation_task_id
                  AND (run.owner_subject IS NULL OR run.owner_issuer IS NULL);
                ALTER TABLE control_workflow_runs
                    ALTER COLUMN owner_subject SET NOT NULL;
                ALTER TABLE control_workflow_runs
                    ALTER COLUMN owner_issuer SET NOT NULL;
                CREATE UNIQUE INDEX IF NOT EXISTS idx_control_workflow_runs_owner_client_request
                    ON control_workflow_runs (owner_subject, owner_issuer, client_request_id)
                    WHERE client_request_id IS NOT NULL;
                ALTER TABLE control_workflow_definition_versions
                    ADD COLUMN IF NOT EXISTS source JSONB NULL;
                "#,
            )
            .await
            .map_err(|error| SessionStoreError::Backend(format!("failed to migrate postgres schema: {error}")))
    }

    async fn enqueue_workflow_event_deliveries(
        transaction: &Transaction<'_>,
        run: &StoredWorkflowRun,
        event: &StoredWorkflowRunEvent,
    ) -> Result<(), SessionStoreError> {
        let rows = transaction
            .query(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    target_url,
                    event_types,
                    signing_secret,
                    created_at,
                    updated_at
                FROM control_workflow_event_subscriptions
                WHERE owner_subject = $1
                  AND owner_issuer = $2
                "#,
                &[&run.owner_subject, &run.owner_issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to load workflow event subscriptions for delivery enqueue: {error}"
                ))
            })?;

        for row in rows {
            let subscription = row_to_stored_workflow_event_subscription(&row)?;
            if !workflow_event_type_matches(&subscription.event_types, &event.event_type) {
                continue;
            }
            let delivery_id = Uuid::now_v7();
            let payload =
                build_workflow_event_delivery_payload(subscription.id, delivery_id, run, event);
            transaction
                .execute(
                    r#"
                    INSERT INTO control_workflow_event_deliveries (
                        id,
                        subscription_id,
                        run_id,
                        event_id,
                        event_type,
                        target_url,
                        signing_secret,
                        payload,
                        state,
                        attempt_count,
                        next_attempt_at,
                        last_attempt_at,
                        delivered_at,
                        last_response_status,
                        last_error,
                        created_at,
                        updated_at
                    )
                    VALUES (
                        $1, $2, $3, $4, $5, $6, $7, $8::jsonb, 'pending',
                        0, $9, NULL, NULL, NULL, NULL, $9, $9
                    )
                    "#,
                    &[
                        &delivery_id,
                        &subscription.id,
                        &run.id,
                        &event.id,
                        &event.event_type,
                        &subscription.target_url,
                        &subscription.signing_secret,
                        &payload,
                        &event.created_at,
                    ],
                )
                .await
                .map_err(|error| {
                    SessionStoreError::Backend(format!(
                        "failed to insert workflow event delivery: {error}"
                    ))
                })?;
        }
        Ok(())
    }

    async fn create_session(
        &self,
        principal: &AuthenticatedPrincipal,
        request: CreateSessionRequest,
        owner_mode: SessionOwnerMode,
    ) -> Result<StoredSession, SessionStoreError> {
        let mut client = self.client.lock().await;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;

        let existing = transaction
            .query_opt(
                r#"
                SELECT COUNT(*)::BIGINT AS session_count
                FROM control_sessions
                WHERE runtime_binding = $1
                  AND state IN ('pending', 'starting', 'ready', 'active', 'idle')
                "#,
                &[&self.config.runtime_binding],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to check active sessions: {error}"))
            })?;
        let active_runtime_candidates = existing
            .as_ref()
            .map(|row| row.get::<_, i64>("session_count"))
            .unwrap_or(0);
        if active_runtime_candidates >= self.config.max_runtime_candidates as i64 {
            return Err(SessionStoreError::ActiveSessionConflict {
                max_runtime_sessions: self.config.max_runtime_candidates,
            });
        }

        let viewport = request.viewport.unwrap_or_default();
        let now = Utc::now();
        let labels_value = json_labels(&request.labels);
        let extensions_value = json_applied_extensions(&request.extensions)?;
        let recording_value = json_recording_policy(&request.recording)?;
        let session_id = Uuid::now_v7();
        let row = transaction
            .query_one(
                r#"
                INSERT INTO control_sessions (
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    state,
                    template_id,
                    owner_mode,
                    viewport_width,
                    viewport_height,
                    idle_timeout_sec,
                    labels,
                    integration_context,
                    extensions,
                    recording,
                    runtime_binding,
                    created_at,
                    updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11::jsonb, $12::jsonb, $13::jsonb, $14::jsonb, $15, $16, $16)
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    automation_owner_client_id,
                    automation_owner_issuer,
                    automation_owner_display_name,
                    state,
                    template_id,
                    owner_mode,
                    viewport_width,
                    viewport_height,
                    idle_timeout_sec,
                    labels,
                    integration_context,
                    extensions,
                    recording,
                    created_at,
                    updated_at,
                    stopped_at
                "#,
                &[
                    &session_id,
                    &principal.subject,
                    &principal.issuer,
                    &principal.display_name,
                    &SessionLifecycleState::Ready.as_str(),
                    &request.template_id,
                    &owner_mode.as_str(),
                    &(viewport.width as i32),
                    &(viewport.height as i32),
                    &request.idle_timeout_sec.map(|value| value as i32),
                    &labels_value,
                    &request.integration_context,
                    &extensions_value,
                    &recording_value,
                    &self.config.runtime_binding,
                    &now,
                ],
            )
            .await
            .map_err(|error| SessionStoreError::Backend(format!("failed to insert session: {error}")))?;

        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;

        row_to_stored_session(&row)
    }

    async fn list_sessions_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredSession>, SessionStoreError> {
        let rows = self
            .client
            .lock()
            .await
            .query(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    automation_owner_client_id,
                    automation_owner_issuer,
                    automation_owner_display_name,
                    state,
                    template_id,
                    owner_mode,
                    viewport_width,
                    viewport_height,
                    idle_timeout_sec,
                    labels,
                    integration_context,
                    extensions,
                    recording,
                    created_at,
                    updated_at,
                    stopped_at
                FROM control_sessions
                WHERE owner_subject = $1 AND owner_issuer = $2
                ORDER BY created_at DESC
                "#,
                &[&principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to list sessions: {error}"))
            })?;

        rows.iter().map(row_to_stored_session).collect()
    }

    async fn get_session_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    automation_owner_client_id,
                    automation_owner_issuer,
                    automation_owner_display_name,
                    state,
                    template_id,
                    owner_mode,
                    viewport_width,
                    viewport_height,
                    idle_timeout_sec,
                    labels,
                    integration_context,
                    extensions,
                    recording,
                    created_at,
                    updated_at,
                    stopped_at
                FROM control_sessions
                WHERE id = $1 AND owner_subject = $2 AND owner_issuer = $3
                "#,
                &[&id, &principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to load session: {error}"))
            })?;
        row.as_ref().map(row_to_stored_session).transpose()
    }

    async fn get_session_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    automation_owner_client_id,
                    automation_owner_issuer,
                    automation_owner_display_name,
                    state,
                    template_id,
                    owner_mode,
                    viewport_width,
                    viewport_height,
                    idle_timeout_sec,
                    labels,
                    integration_context,
                    extensions,
                    recording,
                    created_at,
                    updated_at,
                    stopped_at
                FROM control_sessions
                WHERE id = $1
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to load session by id: {error}"))
            })?;
        row.as_ref().map(row_to_stored_session).transpose()
    }

    async fn get_session_for_principal(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    automation_owner_client_id,
                    automation_owner_issuer,
                    automation_owner_display_name,
                    state,
                    template_id,
                    owner_mode,
                    viewport_width,
                    viewport_height,
                    idle_timeout_sec,
                    labels,
                    integration_context,
                    extensions,
                    recording,
                    created_at,
                    updated_at,
                    stopped_at
                FROM control_sessions
                WHERE id = $1
                  AND (
                    (owner_subject = $2 AND owner_issuer = $3)
                    OR (
                        automation_owner_client_id IS NOT NULL
                        AND automation_owner_issuer = $3
                        AND automation_owner_client_id = $4
                    )
                  )
                "#,
                &[
                    &id,
                    &principal.subject,
                    &principal.issuer,
                    &principal.client_id,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to load session for principal: {error}"))
            })?;
        row.as_ref().map(row_to_stored_session).transpose()
    }

    async fn get_runtime_candidate_session(
        &self,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    automation_owner_client_id,
                    automation_owner_issuer,
                    automation_owner_display_name,
                    state,
                    template_id,
                    owner_mode,
                    viewport_width,
                    viewport_height,
                    idle_timeout_sec,
                    labels,
                    integration_context,
                    extensions,
                    recording,
                    created_at,
                    updated_at,
                    stopped_at
                FROM control_sessions
                WHERE runtime_binding = $1
                  AND state IN ('pending', 'starting', 'ready', 'active', 'idle')
                ORDER BY updated_at DESC
                LIMIT 1
                "#,
                &[&self.config.runtime_binding],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to load runtime candidate session: {error}"
                ))
            })?;
        row.as_ref().map(row_to_stored_session).transpose()
    }

    async fn stop_session_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                UPDATE control_sessions
                SET
                    state = 'stopped',
                    updated_at = NOW(),
                    stopped_at = COALESCE(stopped_at, NOW())
                WHERE id = $1
                  AND owner_subject = $2
                  AND owner_issuer = $3
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    automation_owner_client_id,
                    automation_owner_issuer,
                    automation_owner_display_name,
                    state,
                    template_id,
                    owner_mode,
                    viewport_width,
                    viewport_height,
                    idle_timeout_sec,
                    labels,
                    integration_context,
                    extensions,
                    recording,
                    created_at,
                    updated_at,
                    stopped_at
                "#,
                &[&id, &principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to stop session: {error}"))
            })?;
        row.as_ref().map(row_to_stored_session).transpose()
    }

    async fn mark_session_state(
        &self,
        id: Uuid,
        state: SessionLifecycleState,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                UPDATE control_sessions
                SET
                    state = $2,
                    updated_at = NOW()
                WHERE id = $1
                  AND state IN ('pending', 'starting', 'ready', 'active', 'idle')
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    automation_owner_client_id,
                    automation_owner_issuer,
                    automation_owner_display_name,
                    state,
                    template_id,
                    owner_mode,
                    viewport_width,
                    viewport_height,
                    idle_timeout_sec,
                    labels,
                    integration_context,
                    extensions,
                    recording,
                    created_at,
                    updated_at,
                    stopped_at
                "#,
                &[&id, &state.as_str()],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to update session state: {error}"))
            })?;
        row.as_ref().map(row_to_stored_session).transpose()
    }

    async fn stop_session_if_idle(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                UPDATE control_sessions
                SET
                    state = 'stopped',
                    updated_at = NOW(),
                    stopped_at = COALESCE(stopped_at, NOW())
                WHERE id = $1
                  AND state IN ('ready', 'idle')
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    automation_owner_client_id,
                    automation_owner_issuer,
                    automation_owner_display_name,
                    state,
                    template_id,
                    owner_mode,
                    viewport_width,
                    viewport_height,
                    idle_timeout_sec,
                    labels,
                    integration_context,
                    extensions,
                    recording,
                    created_at,
                    updated_at,
                    stopped_at
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to stop idle session: {error}"))
            })?;
        row.as_ref().map(row_to_stored_session).transpose()
    }

    async fn prepare_session_for_connect(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let mut client = self.client.lock().await;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;

        let current_row = transaction
            .query_opt(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    automation_owner_client_id,
                    automation_owner_issuer,
                    automation_owner_display_name,
                    state,
                    template_id,
                    owner_mode,
                    viewport_width,
                    viewport_height,
                    idle_timeout_sec,
                    labels,
                    integration_context,
                    extensions,
                    recording,
                    created_at,
                    updated_at,
                    stopped_at
                FROM control_sessions
                WHERE id = $1
                FOR UPDATE
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to lock session for connect prep: {error}"
                ))
            })?;
        let Some(current_row) = current_row else {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Ok(None);
        };

        let current = row_to_stored_session(&current_row)?;
        if current.state != SessionLifecycleState::Stopped {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Ok(Some(current));
        }

        let existing = transaction
            .query_opt(
                r#"
                SELECT COUNT(*)::BIGINT AS session_count
                FROM control_sessions
                WHERE runtime_binding = $1
                  AND state IN ('pending', 'starting', 'ready', 'active', 'idle')
                "#,
                &[&self.config.runtime_binding],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to check active sessions: {error}"))
            })?;
        let active_runtime_candidates = existing
            .as_ref()
            .map(|row| row.get::<_, i64>("session_count"))
            .unwrap_or(0);
        if active_runtime_candidates >= self.config.max_runtime_candidates as i64 {
            return Err(SessionStoreError::ActiveSessionConflict {
                max_runtime_sessions: self.config.max_runtime_candidates,
            });
        }

        let row = transaction
            .query_one(
                r#"
                UPDATE control_sessions
                SET
                    state = 'ready',
                    updated_at = NOW(),
                    stopped_at = NULL
                WHERE id = $1
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    automation_owner_client_id,
                    automation_owner_issuer,
                    automation_owner_display_name,
                    state,
                    template_id,
                    owner_mode,
                    viewport_width,
                    viewport_height,
                    idle_timeout_sec,
                    labels,
                    integration_context,
                    extensions,
                    recording,
                    created_at,
                    updated_at,
                    stopped_at
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to prepare stopped session for connect: {error}"
                ))
            })?;

        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;

        row_to_stored_session(&row).map(Some)
    }

    async fn set_automation_delegate_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        request: SetAutomationDelegateRequest,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let issuer = request.issuer.unwrap_or_else(|| principal.issuer.clone());
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                UPDATE control_sessions
                SET
                    automation_owner_client_id = $4,
                    automation_owner_issuer = $5,
                    automation_owner_display_name = $6,
                    updated_at = NOW()
                WHERE id = $1
                  AND owner_subject = $2
                  AND owner_issuer = $3
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    automation_owner_client_id,
                    automation_owner_issuer,
                    automation_owner_display_name,
                    state,
                    template_id,
                    owner_mode,
                    viewport_width,
                    viewport_height,
                    idle_timeout_sec,
                    labels,
                    integration_context,
                    extensions,
                    recording,
                    created_at,
                    updated_at,
                    stopped_at
                "#,
                &[
                    &id,
                    &principal.subject,
                    &principal.issuer,
                    &request.client_id,
                    &issuer,
                    &request.display_name,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to set automation delegate: {error}"))
            })?;
        row.as_ref().map(row_to_stored_session).transpose()
    }

    async fn clear_automation_delegate_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                UPDATE control_sessions
                SET
                    automation_owner_client_id = NULL,
                    automation_owner_issuer = NULL,
                    automation_owner_display_name = NULL,
                    updated_at = NOW()
                WHERE id = $1
                  AND owner_subject = $2
                  AND owner_issuer = $3
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    automation_owner_client_id,
                    automation_owner_issuer,
                    automation_owner_display_name,
                    state,
                    template_id,
                    owner_mode,
                    viewport_width,
                    viewport_height,
                    idle_timeout_sec,
                    labels,
                    integration_context,
                    extensions,
                    recording,
                    created_at,
                    updated_at,
                    stopped_at
                "#,
                &[&id, &principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to clear automation delegate: {error}"))
            })?;
        row.as_ref().map(row_to_stored_session).transpose()
    }

    async fn create_automation_task(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistAutomationTaskRequest,
    ) -> Result<StoredAutomationTask, SessionStoreError> {
        let mut client = self.client.lock().await;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;

        let now = Utc::now();
        let task_id = Uuid::now_v7();
        let labels_value = json_labels(&request.labels);
        let artifact_refs_value = Value::Array(Vec::new());
        let row = transaction
            .query_one(
                r#"
                INSERT INTO control_automation_tasks (
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    display_name,
                    executor,
                    state,
                    session_id,
                    session_source,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    cancel_requested_at,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                )
                VALUES (
                    $1, $2, $3, $4, $5, $6, 'pending', $7, $8, $9::jsonb, NULL, NULL,
                    $10::jsonb, $11::jsonb, NULL, NULL, NULL, $12, $12
                )
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    display_name,
                    executor,
                    state,
                    session_id,
                    session_source,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    cancel_requested_at,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                "#,
                &[
                    &task_id,
                    &principal.subject,
                    &principal.issuer,
                    &principal.display_name,
                    &request.display_name,
                    &request.executor,
                    &request.session_id,
                    &request.session_source.as_str(),
                    &request.input,
                    &artifact_refs_value,
                    &labels_value,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to insert automation task: {error}"))
            })?;

        transaction
            .execute(
                r#"
                INSERT INTO control_automation_task_events (
                    id,
                    task_id,
                    event_type,
                    message,
                    data,
                    created_at
                )
                VALUES ($1, $2, $3, $4, $5::jsonb, $6)
                "#,
                &[
                    &Uuid::now_v7(),
                    &task_id,
                    &"automation_task.created",
                    &"automation task created",
                    &Some(serde_json::json!({
                        "session_id": request.session_id,
                        "session_source": request.session_source.as_str(),
                        "executor": request.executor,
                    })),
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to insert automation task event: {error}"
                ))
            })?;

        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;

        row_to_stored_automation_task(&row)
    }

    async fn list_automation_tasks_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredAutomationTask>, SessionStoreError> {
        let rows = self
            .client
            .lock()
            .await
            .query(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    display_name,
                    executor,
                    state,
                    session_id,
                    session_source,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    cancel_requested_at,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                FROM control_automation_tasks
                WHERE owner_subject = $1
                  AND owner_issuer = $2
                ORDER BY created_at DESC
                "#,
                &[&principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to list automation tasks: {error}"))
            })?;

        rows.iter().map(row_to_stored_automation_task).collect()
    }

    async fn get_automation_task_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredAutomationTask>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    display_name,
                    executor,
                    state,
                    session_id,
                    session_source,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    cancel_requested_at,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                FROM control_automation_tasks
                WHERE id = $1
                  AND owner_subject = $2
                  AND owner_issuer = $3
                "#,
                &[&id, &principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to load automation task: {error}"))
            })?;
        row.as_ref().map(row_to_stored_automation_task).transpose()
    }

    async fn get_automation_task_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredAutomationTask>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    display_name,
                    executor,
                    state,
                    session_id,
                    session_source,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    cancel_requested_at,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                FROM control_automation_tasks
                WHERE id = $1
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to load automation task by id: {error}"))
            })?;
        row.as_ref().map(row_to_stored_automation_task).transpose()
    }

    async fn cancel_automation_task_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredAutomationTask>, SessionStoreError> {
        let mut client = self.client.lock().await;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;

        let row = transaction
            .query_opt(
                r#"
                UPDATE control_automation_tasks
                SET
                    state = 'cancelled',
                    cancel_requested_at = NOW(),
                    completed_at = NOW(),
                    updated_at = NOW()
                WHERE id = $1
                  AND owner_subject = $2
                  AND owner_issuer = $3
                  AND state IN ('pending', 'queued', 'starting', 'running', 'awaiting_input')
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    display_name,
                    executor,
                    state,
                    session_id,
                    session_source,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    cancel_requested_at,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                "#,
                &[&id, &principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to cancel automation task: {error}"))
            })?;
        let Some(row) = row else {
            let existing = transaction
                .query_opt(
                    r#"
                    SELECT
                        id,
                        owner_subject,
                        owner_issuer,
                        owner_display_name,
                        display_name,
                        executor,
                        state,
                        session_id,
                        session_source,
                        input,
                        output,
                        error,
                        artifact_refs,
                        labels,
                        cancel_requested_at,
                        started_at,
                        completed_at,
                        created_at,
                        updated_at
                    FROM control_automation_tasks
                    WHERE id = $1
                      AND owner_subject = $2
                      AND owner_issuer = $3
                    "#,
                    &[&id, &principal.subject, &principal.issuer],
                )
                .await
                .map_err(|error| {
                    SessionStoreError::Backend(format!(
                        "failed to load automation task after cancel conflict: {error}"
                    ))
                })?;
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            if existing.is_some() {
                return Err(SessionStoreError::Conflict(format!(
                    "automation task {id} is already terminal"
                )));
            }
            return Ok(None);
        };

        let now = Utc::now();
        transaction
            .execute(
                r#"
                INSERT INTO control_automation_task_events (
                    id,
                    task_id,
                    event_type,
                    message,
                    data,
                    created_at
                )
                VALUES ($1, $2, $3, $4, NULL, $5)
                "#,
                &[
                    &Uuid::now_v7(),
                    &id,
                    &"automation_task.cancelled",
                    &"automation task cancelled",
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to insert automation task cancel event: {error}"
                ))
            })?;
        transaction
            .execute(
                r#"
                INSERT INTO control_automation_task_logs (
                    id,
                    task_id,
                    stream,
                    message,
                    created_at
                )
                VALUES ($1, $2, $3, $4, $5)
                "#,
                &[
                    &Uuid::now_v7(),
                    &id,
                    &AutomationTaskLogStream::System.as_str(),
                    &"automation task cancelled",
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to insert automation task cancel log: {error}"
                ))
            })?;

        let task = row_to_stored_automation_task(&row)?;
        let workflow_run_row = transaction
            .execute(
                r#"
                UPDATE control_workflow_runs
                SET
                    state = $2,
                    output = $3::jsonb,
                    error = $4,
                    artifact_refs = $5::jsonb,
                    started_at = $6,
                    completed_at = $7,
                    updated_at = $8
                WHERE automation_task_id = $1
                "#,
                &[
                    &task.id,
                    &WorkflowRunState::from(task.state).as_str(),
                    &task.output,
                    &task.error,
                    &json_string_array(&task.artifact_refs),
                    &task.started_at,
                    &task.completed_at,
                    &task.updated_at,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to sync workflow run after automation task cancel: {error}"
                ))
            })?;

        let workflow_run_id = if workflow_run_row > 0 {
            transaction
                .query_opt(
                    r#"
                    SELECT id
                    FROM control_workflow_runs
                    WHERE automation_task_id = $1
                    "#,
                    &[&task.id],
                )
                .await
                .map_err(|error| {
                    SessionStoreError::Backend(format!(
                        "failed to load workflow run after automation task cancel: {error}"
                    ))
                })?
                .map(|row| row.get::<_, Uuid>("id"))
        } else {
            None
        };

        if let Some(run_id) = workflow_run_id {
            let run_row = transaction
                .query_one(
                    r#"
                    SELECT
                        id,
                        owner_subject,
                        owner_issuer,
                        workflow_definition_id,
                        workflow_definition_version_id,
                        workflow_version,
                        session_id,
                        automation_task_id,
                        state,
                        source_system,
                        source_reference,
                        client_request_id,
                        create_request_fingerprint,
                        source_snapshot,
                        extensions,
                        credential_bindings,
                        workspace_inputs,
                        produced_files,
                        input,
                        output,
                        error,
                        artifact_refs,
                        labels,
                        started_at,
                        completed_at,
                        created_at,
                        updated_at
                    FROM control_workflow_runs
                    WHERE id = $1
                    "#,
                    &[&run_id],
                )
                .await
                .map_err(|error| {
                    SessionStoreError::Backend(format!(
                        "failed to reload workflow run after automation task cancel: {error}"
                    ))
                })?;
            let run = row_to_stored_workflow_run(&run_row)?;
            let event = StoredWorkflowRunEvent {
                id: Uuid::now_v7(),
                run_id,
                event_type: "workflow_run.cancelled".to_string(),
                message: "workflow run cancelled".to_string(),
                data: None,
                created_at: now,
            };
            transaction
                .execute(
                    r#"
                    INSERT INTO control_workflow_run_events (
                        id,
                        run_id,
                        event_type,
                        message,
                        data,
                        created_at
                    )
                    VALUES ($1, $2, $3, $4, NULL, $5)
                    "#,
                    &[&event.id, &run_id, &event.event_type, &event.message, &now],
                )
                .await
                .map_err(|error| {
                    SessionStoreError::Backend(format!(
                        "failed to insert workflow run cancel event: {error}"
                    ))
                })?;
            Self::enqueue_workflow_event_deliveries(&transaction, &run, &event).await?;
            transaction
                .execute(
                    r#"
                    INSERT INTO control_workflow_run_logs (
                        id,
                        run_id,
                        stream,
                        message,
                        created_at
                    )
                    VALUES ($1, $2, $3, $4, $5)
                    "#,
                    &[
                        &Uuid::now_v7(),
                        &run_id,
                        &AutomationTaskLogStream::System.as_str(),
                        &"workflow run cancelled",
                        &now,
                    ],
                )
                .await
                .map_err(|error| {
                    SessionStoreError::Backend(format!(
                        "failed to insert workflow run cancel log: {error}"
                    ))
                })?;
        }

        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;

        Ok(Some(task))
    }

    async fn list_automation_task_events_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Vec<StoredAutomationTaskEvent>, SessionStoreError> {
        if self
            .get_automation_task_for_owner(principal, id)
            .await?
            .is_none()
        {
            return Ok(Vec::new());
        }
        let rows = self
            .client
            .lock()
            .await
            .query(
                r#"
                SELECT
                    id,
                    task_id,
                    event_type,
                    message,
                    data,
                    created_at
                FROM control_automation_task_events
                WHERE task_id = $1
                ORDER BY created_at ASC, id ASC
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to list automation task events: {error}"
                ))
            })?;
        rows.iter()
            .map(row_to_stored_automation_task_event)
            .collect()
    }

    async fn list_automation_task_logs_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Vec<StoredAutomationTaskLog>, SessionStoreError> {
        if self
            .get_automation_task_for_owner(principal, id)
            .await?
            .is_none()
        {
            return Ok(Vec::new());
        }
        let rows = self
            .client
            .lock()
            .await
            .query(
                r#"
                SELECT
                    id,
                    task_id,
                    stream,
                    message,
                    created_at
                FROM control_automation_task_logs
                WHERE task_id = $1
                ORDER BY created_at ASC, id ASC
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to list automation task logs: {error}"))
            })?;
        rows.iter().map(row_to_stored_automation_task_log).collect()
    }

    async fn transition_automation_task(
        &self,
        id: Uuid,
        request: AutomationTaskTransitionRequest,
    ) -> Result<Option<StoredAutomationTask>, SessionStoreError> {
        let mut client = self.client.lock().await;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;

        let current_row = transaction
            .query_opt(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    display_name,
                    executor,
                    state,
                    session_id,
                    session_source,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    cancel_requested_at,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                FROM control_automation_tasks
                WHERE id = $1
                FOR UPDATE
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to lock automation task for transition: {error}"
                ))
            })?;
        let Some(current_row) = current_row else {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Ok(None);
        };
        let current = row_to_stored_automation_task(&current_row)?;
        if current.state.is_terminal() {
            return Err(SessionStoreError::Conflict(format!(
                "automation task {id} is already terminal"
            )));
        }
        if !current.state.can_transition_to(request.state) {
            return Err(SessionStoreError::Conflict(format!(
                "automation task {id} cannot transition from {} to {}",
                current.state.as_str(),
                request.state.as_str()
            )));
        }

        let now = Utc::now();
        let started_at = if matches!(
            request.state,
            AutomationTaskState::Starting
                | AutomationTaskState::Running
                | AutomationTaskState::AwaitingInput
        ) {
            current.started_at.or(Some(now))
        } else {
            current.started_at
        };
        let completed_at = if request.state.is_terminal() {
            Some(now)
        } else {
            current.completed_at
        };
        let artifact_refs = json_string_array(&request.artifact_refs);
        let row = transaction
            .query_one(
                r#"
                UPDATE control_automation_tasks
                SET
                    state = $2,
                    output = $3::jsonb,
                    error = $4,
                    artifact_refs = $5::jsonb,
                    started_at = $6,
                    completed_at = $7,
                    updated_at = $8
                WHERE id = $1
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    display_name,
                    executor,
                    state,
                    session_id,
                    session_source,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    cancel_requested_at,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                "#,
                &[
                    &id,
                    &request.state.as_str(),
                    &request.output,
                    &request.error,
                    &artifact_refs,
                    &started_at,
                    &completed_at,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to update automation task state: {error}"
                ))
            })?;

        transaction
            .execute(
                r#"
                INSERT INTO control_automation_task_events (
                    id,
                    task_id,
                    event_type,
                    message,
                    data,
                    created_at
                )
                VALUES ($1, $2, $3, $4, $5::jsonb, $6)
                "#,
                &[
                    &Uuid::now_v7(),
                    &id,
                    &request.event_type,
                    &request.event_message,
                    &request.event_data,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to insert automation task transition event: {error}"
                ))
            })?;

        let task = row_to_stored_automation_task(&row)?;
        transaction
            .execute(
                r#"
                UPDATE control_workflow_runs
                SET
                    state = $2,
                    output = $3::jsonb,
                    error = $4,
                    artifact_refs = $5::jsonb,
                    started_at = $6,
                    completed_at = $7,
                    updated_at = $8
                WHERE automation_task_id = $1
                "#,
                &[
                    &task.id,
                    &WorkflowRunState::from(task.state).as_str(),
                    &task.output,
                    &task.error,
                    &json_string_array(&task.artifact_refs),
                    &task.started_at,
                    &task.completed_at,
                    &task.updated_at,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to sync workflow run after automation task transition: {error}"
                ))
            })?;

        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;

        Ok(Some(task))
    }

    async fn append_automation_task_log(
        &self,
        id: Uuid,
        stream: AutomationTaskLogStream,
        message: String,
    ) -> Result<Option<StoredAutomationTaskLog>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                INSERT INTO control_automation_task_logs (
                    id,
                    task_id,
                    stream,
                    message,
                    created_at
                )
                SELECT $2, $1, $3, $4, $5
                WHERE EXISTS (
                    SELECT 1
                    FROM control_automation_tasks
                    WHERE id = $1
                )
                RETURNING
                    id,
                    task_id,
                    stream,
                    message,
                    created_at
                "#,
                &[
                    &id,
                    &Uuid::now_v7(),
                    &stream.as_str(),
                    &message,
                    &Utc::now(),
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to append automation task log: {error}"))
            })?;
        row.as_ref()
            .map(row_to_stored_automation_task_log)
            .transpose()
    }

    async fn create_workflow_definition(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistWorkflowDefinitionRequest,
    ) -> Result<StoredWorkflowDefinition, SessionStoreError> {
        let now = Utc::now();
        let labels_value = json_labels(&request.labels);
        let row = self
            .client
            .lock()
            .await
            .query_one(
                r#"
                INSERT INTO control_workflow_definitions (
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    name,
                    description,
                    labels,
                    latest_version,
                    created_at,
                    updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7::jsonb, NULL, $8, $8)
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    name,
                    description,
                    labels,
                    latest_version,
                    created_at,
                    updated_at
                "#,
                &[
                    &Uuid::now_v7(),
                    &principal.subject,
                    &principal.issuer,
                    &principal.display_name,
                    &request.name,
                    &request.description,
                    &labels_value,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to insert workflow definition: {error}"))
            })?;
        row_to_stored_workflow_definition(&row)
    }

    async fn list_workflow_definitions_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredWorkflowDefinition>, SessionStoreError> {
        let rows = self
            .client
            .lock()
            .await
            .query(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    name,
                    description,
                    labels,
                    latest_version,
                    created_at,
                    updated_at
                FROM control_workflow_definitions
                WHERE owner_subject = $1
                  AND owner_issuer = $2
                ORDER BY created_at DESC
                "#,
                &[&principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to list workflow definitions: {error}"))
            })?;
        rows.iter().map(row_to_stored_workflow_definition).collect()
    }

    async fn get_workflow_definition_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowDefinition>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    name,
                    description,
                    labels,
                    latest_version,
                    created_at,
                    updated_at
                FROM control_workflow_definitions
                WHERE id = $1
                  AND owner_subject = $2
                  AND owner_issuer = $3
                "#,
                &[&id, &principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to load workflow definition: {error}"))
            })?;
        row.as_ref()
            .map(row_to_stored_workflow_definition)
            .transpose()
    }

    async fn create_workflow_definition_version(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistWorkflowDefinitionVersionRequest,
    ) -> Result<StoredWorkflowDefinitionVersion, SessionStoreError> {
        let mut client = self.client.lock().await;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;

        let visible = transaction
            .query_opt(
                r#"
                SELECT id
                FROM control_workflow_definitions
                WHERE id = $1
                  AND owner_subject = $2
                  AND owner_issuer = $3
                "#,
                &[
                    &request.workflow_definition_id,
                    &principal.subject,
                    &principal.issuer,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to validate workflow definition ownership: {error}"
                ))
            })?;
        if visible.is_none() {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Err(SessionStoreError::NotFound(format!(
                "workflow definition {} not found",
                request.workflow_definition_id
            )));
        }

        let now = Utc::now();
        let source_value = json_workflow_source(request.source.as_ref())?;
        let row = transaction
            .query_one(
                r#"
                INSERT INTO control_workflow_definition_versions (
                    id,
                    workflow_definition_id,
                    version,
                    executor,
                    entrypoint,
                    source,
                    input_schema,
                    output_schema,
                    default_session,
                    allowed_credential_binding_ids,
                    allowed_extension_ids,
                    allowed_file_workspace_ids,
                    created_at
                )
                VALUES (
                    $1, $2, $3, $4, $5, $6::jsonb, $7::jsonb, $8::jsonb, $9::jsonb,
                    $10::jsonb, $11::jsonb, $12::jsonb, $13
                )
                RETURNING
                    id,
                    workflow_definition_id,
                    version,
                    executor,
                    entrypoint,
                    source,
                    input_schema,
                    output_schema,
                    default_session,
                    allowed_credential_binding_ids,
                    allowed_extension_ids,
                    allowed_file_workspace_ids,
                    created_at
                "#,
                &[
                    &Uuid::now_v7(),
                    &request.workflow_definition_id,
                    &request.version,
                    &request.executor,
                    &request.entrypoint,
                    &source_value,
                    &request.input_schema,
                    &request.output_schema,
                    &request.default_session,
                    &json_string_array(&request.allowed_credential_binding_ids),
                    &json_string_array(&request.allowed_extension_ids),
                    &json_string_array(&request.allowed_file_workspace_ids),
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                if let Some(code) = error.code() {
                    if code.code() == "23505" {
                        return SessionStoreError::Conflict(format!(
                            "workflow version {} already exists",
                            request.version
                        ));
                    }
                }
                SessionStoreError::Backend(format!(
                    "failed to insert workflow definition version: {error}"
                ))
            })?;

        transaction
            .execute(
                r#"
                UPDATE control_workflow_definitions
                SET latest_version = $2, updated_at = $3
                WHERE id = $1
                "#,
                &[&request.workflow_definition_id, &request.version, &now],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to update workflow definition latest_version: {error}"
                ))
            })?;

        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;
        row_to_stored_workflow_definition_version(&row)
    }

    async fn get_workflow_definition_version_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        workflow_definition_id: Uuid,
        version: &str,
    ) -> Result<Option<StoredWorkflowDefinitionVersion>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                SELECT
                    version.id,
                    version.workflow_definition_id,
                    version.version,
                    version.executor,
                    version.entrypoint,
                    version.source,
                    version.input_schema,
                    version.output_schema,
                    version.default_session,
                    version.allowed_credential_binding_ids,
                    version.allowed_extension_ids,
                    version.allowed_file_workspace_ids,
                    version.created_at
                FROM control_workflow_definition_versions version
                JOIN control_workflow_definitions workflow
                  ON workflow.id = version.workflow_definition_id
                WHERE version.workflow_definition_id = $1
                  AND version.version = $2
                  AND workflow.owner_subject = $3
                  AND workflow.owner_issuer = $4
                "#,
                &[
                    &workflow_definition_id,
                    &version,
                    &principal.subject,
                    &principal.issuer,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to load workflow definition version: {error}"
                ))
            })?;
        row.as_ref()
            .map(row_to_stored_workflow_definition_version)
            .transpose()
    }

    async fn get_workflow_definition_version_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowDefinitionVersion>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                SELECT
                    id,
                    workflow_definition_id,
                    version,
                    executor,
                    entrypoint,
                    source,
                    input_schema,
                    output_schema,
                    default_session,
                    allowed_credential_binding_ids,
                    allowed_extension_ids,
                    allowed_file_workspace_ids,
                    created_at
                FROM control_workflow_definition_versions
                WHERE id = $1
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to load workflow definition version by id: {error}"
                ))
            })?;
        row.as_ref()
            .map(row_to_stored_workflow_definition_version)
            .transpose()
    }

    async fn create_workflow_run(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistWorkflowRunRequest,
    ) -> Result<CreateWorkflowRunResult, SessionStoreError> {
        let mut client = self.client.lock().await;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;

        let workflow_row = transaction
            .query_opt(
                r#"
                SELECT id
                FROM control_workflow_definitions
                WHERE id = $1
                  AND owner_subject = $2
                  AND owner_issuer = $3
                "#,
                &[
                    &request.workflow_definition_id,
                    &principal.subject,
                    &principal.issuer,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to validate workflow definition for run: {error}"
                ))
            })?;
        if workflow_row.is_none() {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Err(SessionStoreError::NotFound(format!(
                "workflow definition {} not found",
                request.workflow_definition_id
            )));
        }

        let version_row = transaction
            .query_opt(
                r#"
                SELECT id, workflow_definition_id, version
                FROM control_workflow_definition_versions
                WHERE id = $1
                "#,
                &[&request.workflow_definition_version_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to validate workflow definition version for run: {error}"
                ))
            })?;
        let Some(version_row) = version_row else {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Err(SessionStoreError::NotFound(format!(
                "workflow definition version {} not found",
                request.workflow_definition_version_id
            )));
        };
        let version_workflow_id: Uuid = version_row.get("workflow_definition_id");
        let version_name: String = version_row.get("version");
        if version_workflow_id != request.workflow_definition_id
            || version_name != request.workflow_version
        {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Err(SessionStoreError::InvalidRequest(
                "workflow run version must belong to the requested workflow definition".to_string(),
            ));
        }

        let task_row = transaction
            .query_opt(
                r#"
                SELECT id, session_id
                FROM control_automation_tasks
                WHERE id = $1
                  AND owner_subject = $2
                  AND owner_issuer = $3
                "#,
                &[
                    &request.automation_task_id,
                    &principal.subject,
                    &principal.issuer,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to validate automation task for workflow run: {error}"
                ))
            })?;
        let Some(task_row) = task_row else {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Err(SessionStoreError::NotFound(format!(
                "automation task {} not found",
                request.automation_task_id
            )));
        };
        let task_session_id: Uuid = task_row.get("session_id");
        if task_session_id != request.session_id {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Err(SessionStoreError::InvalidRequest(
                "workflow run session_id must match the bound automation task session".to_string(),
            ));
        }

        let now = Utc::now();
        let source_snapshot = json_workflow_run_source_snapshot(request.source_snapshot.as_ref())?;
        let extensions = json_applied_extensions(&request.extensions)?;
        let credential_bindings =
            json_workflow_run_credential_bindings(&request.credential_bindings)?;
        let workspace_inputs = json_workflow_run_workspace_inputs(&request.workspace_inputs)?;
        let produced_files = json_workflow_run_produced_files(&Vec::new())?;
        if let Some(client_request_id) = request.client_request_id.as_deref() {
            let existing_row = transaction
                .query_opt(
                    r#"
                    SELECT
                        id,
                        owner_subject,
                        owner_issuer,
                        workflow_definition_id,
                        workflow_definition_version_id,
                        workflow_version,
                        session_id,
                        automation_task_id,
                        state,
                        source_system,
                        source_reference,
                        client_request_id,
                        create_request_fingerprint,
                        source_snapshot,
                        extensions,
                        credential_bindings,
                        workspace_inputs,
                        produced_files,
                        input,
                        output,
                        error,
                        artifact_refs,
                        labels,
                        started_at,
                        completed_at,
                        created_at,
                        updated_at
                    FROM control_workflow_runs
                    WHERE owner_subject = $1
                      AND owner_issuer = $2
                      AND client_request_id = $3
                    "#,
                    &[&principal.subject, &principal.issuer, &client_request_id],
                )
                .await
                .map_err(|error| {
                    SessionStoreError::Backend(format!(
                        "failed to check existing workflow run by client_request_id: {error}"
                    ))
                })?;
            if let Some(existing_row) = existing_row {
                let existing_run = row_to_stored_workflow_run(&existing_row)?;
                transaction.commit().await.map_err(|error| {
                    SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
                })?;
                if existing_run.create_request_fingerprint == request.create_request_fingerprint {
                    return Ok(CreateWorkflowRunResult {
                        run: existing_run,
                        created: false,
                    });
                }
                return Err(SessionStoreError::Conflict(format!(
                    "workflow run client_request_id {} is already bound to a different request",
                    client_request_id
                )));
            }
        }
        let row = transaction
            .query_one(
                r#"
                INSERT INTO control_workflow_runs (
                    id,
                    owner_subject,
                    owner_issuer,
                    workflow_definition_id,
                    workflow_definition_version_id,
                    workflow_version,
                    session_id,
                    automation_task_id,
                    state,
                    source_system,
                    source_reference,
                    client_request_id,
                    create_request_fingerprint,
                    source_snapshot,
                    extensions,
                    credential_bindings,
                    workspace_inputs,
                    produced_files,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                )
                VALUES (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9,
                    $10, $11, $12, $13,
                    $14::jsonb, $15::jsonb, $16::jsonb, $17::jsonb, $18::jsonb, $19::jsonb, NULL, NULL, $20::jsonb, $21::jsonb, NULL, NULL, $22, $22
                )
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    workflow_definition_id,
                    workflow_definition_version_id,
                    workflow_version,
                    session_id,
                    automation_task_id,
                    state,
                    source_system,
                    source_reference,
                    client_request_id,
                    create_request_fingerprint,
                    source_snapshot,
                    extensions,
                    credential_bindings,
                    workspace_inputs,
                    produced_files,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                "#,
                &[
                    &Uuid::now_v7(),
                    &principal.subject,
                    &principal.issuer,
                    &request.workflow_definition_id,
                    &request.workflow_definition_version_id,
                    &request.workflow_version,
                    &request.session_id,
                    &request.automation_task_id,
                    &WorkflowRunState::Pending.as_str(),
                    &request.source_system,
                    &request.source_reference,
                    &request.client_request_id,
                    &request.create_request_fingerprint,
                    &source_snapshot,
                    &extensions,
                    &credential_bindings,
                    &workspace_inputs,
                    &produced_files,
                    &request.input,
                    &json_string_array(&Vec::new()),
                    &json_labels(&request.labels),
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to insert workflow run: {error}"))
            })?;
        let run = row_to_stored_workflow_run(&row)?;
        let run_id = run.id;
        let event_id = Uuid::now_v7();
        let event = StoredWorkflowRunEvent {
            id: event_id,
            run_id,
            event_type: "workflow_run.created".to_string(),
            message: "workflow run created".to_string(),
            data: Some(serde_json::json!({
                "workflow_definition_id": request.workflow_definition_id,
                "workflow_definition_version_id": request.workflow_definition_version_id,
                "workflow_version": request.workflow_version,
                "session_id": request.session_id,
                "automation_task_id": request.automation_task_id,
                "source_system": request.source_system,
                "source_reference": request.source_reference,
                "client_request_id": request.client_request_id,
            })),
            created_at: now,
        };

        transaction
            .execute(
                r#"
                INSERT INTO control_workflow_run_events (
                    id,
                    run_id,
                    event_type,
                    message,
                    data,
                    created_at
                )
                VALUES ($1, $2, $3, $4, $5::jsonb, $6)
                "#,
                &[
                    &event_id,
                    &run_id,
                    &"workflow_run.created",
                    &"workflow run created",
                    &event.data,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to insert workflow run event: {error}"))
            })?;
        Self::enqueue_workflow_event_deliveries(&transaction, &run, &event).await?;

        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;
        Ok(CreateWorkflowRunResult { run, created: true })
    }

    async fn get_workflow_run_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                SELECT
                    run.id,
                    run.owner_subject,
                    run.owner_issuer,
                    run.workflow_definition_id,
                    run.workflow_definition_version_id,
                    run.workflow_version,
                    run.session_id,
                    run.automation_task_id,
                    run.state,
                    run.source_system,
                    run.source_reference,
                    run.client_request_id,
                    run.create_request_fingerprint,
                    run.source_snapshot,
                    run.extensions,
                    run.credential_bindings,
                    run.workspace_inputs,
                    run.produced_files,
                    run.input,
                    run.output,
                    run.error,
                    run.artifact_refs,
                    run.labels,
                    run.started_at,
                    run.completed_at,
                    run.created_at,
                    run.updated_at
                FROM control_workflow_runs run
                JOIN control_workflow_definitions workflow
                  ON workflow.id = run.workflow_definition_id
                WHERE run.id = $1
                  AND workflow.owner_subject = $2
                  AND workflow.owner_issuer = $3
                "#,
                &[&id, &principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to load workflow run: {error}"))
            })?;
        row.as_ref().map(row_to_stored_workflow_run).transpose()
    }

    async fn get_workflow_run_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    workflow_definition_id,
                    workflow_definition_version_id,
                    workflow_version,
                    session_id,
                    automation_task_id,
                    state,
                    source_system,
                    source_reference,
                    client_request_id,
                    create_request_fingerprint,
                    source_snapshot,
                    extensions,
                    credential_bindings,
                    workspace_inputs,
                    produced_files,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                FROM control_workflow_runs
                WHERE id = $1
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to load workflow run by id: {error}"))
            })?;
        row.as_ref().map(row_to_stored_workflow_run).transpose()
    }

    async fn list_dispatchable_workflow_runs(
        &self,
    ) -> Result<Vec<StoredWorkflowRun>, SessionStoreError> {
        let rows = self
            .client
            .lock()
            .await
            .query(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    workflow_definition_id,
                    workflow_definition_version_id,
                    workflow_version,
                    session_id,
                    automation_task_id,
                    state,
                    source_system,
                    source_reference,
                    client_request_id,
                    create_request_fingerprint,
                    source_snapshot,
                    extensions,
                    credential_bindings,
                    workspace_inputs,
                    produced_files,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                FROM control_workflow_runs
                WHERE state IN ('pending', 'queued')
                ORDER BY created_at ASC, id ASC
                "#,
                &[],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to list dispatchable workflow runs: {error}"
                ))
            })?;
        rows.into_iter()
            .map(|row| row_to_stored_workflow_run(&row))
            .collect()
    }

    async fn find_workflow_run_by_client_request_id_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        client_request_id: &str,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    workflow_definition_id,
                    workflow_definition_version_id,
                    workflow_version,
                    session_id,
                    automation_task_id,
                    state,
                    source_system,
                    source_reference,
                    client_request_id,
                    create_request_fingerprint,
                    source_snapshot,
                    extensions,
                    credential_bindings,
                    workspace_inputs,
                    produced_files,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                FROM control_workflow_runs
                WHERE owner_subject = $1
                  AND owner_issuer = $2
                  AND client_request_id = $3
                "#,
                &[&principal.subject, &principal.issuer, &client_request_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to find workflow run by client_request_id: {error}"
                ))
            })?;
        row.as_ref().map(row_to_stored_workflow_run).transpose()
    }

    async fn create_workflow_event_subscription(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistWorkflowEventSubscriptionRequest,
    ) -> Result<StoredWorkflowEventSubscription, SessionStoreError> {
        let now = Utc::now();
        let row = self
            .client
            .lock()
            .await
            .query_one(
                r#"
                INSERT INTO control_workflow_event_subscriptions (
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    target_url,
                    event_types,
                    signing_secret,
                    created_at,
                    updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6::jsonb, $7, $8, $8)
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    target_url,
                    event_types,
                    signing_secret,
                    created_at,
                    updated_at
                "#,
                &[
                    &Uuid::now_v7(),
                    &principal.subject,
                    &principal.issuer,
                    &request.name,
                    &request.target_url,
                    &json_string_array(&request.event_types),
                    &request.signing_secret,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to insert workflow event subscription: {error}"
                ))
            })?;
        row_to_stored_workflow_event_subscription(&row)
    }

    async fn list_workflow_event_subscriptions_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredWorkflowEventSubscription>, SessionStoreError> {
        let rows = self
            .client
            .lock()
            .await
            .query(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    target_url,
                    event_types,
                    signing_secret,
                    created_at,
                    updated_at
                FROM control_workflow_event_subscriptions
                WHERE owner_subject = $1
                  AND owner_issuer = $2
                ORDER BY created_at DESC, id DESC
                "#,
                &[&principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to list workflow event subscriptions: {error}"
                ))
            })?;
        rows.iter()
            .map(row_to_stored_workflow_event_subscription)
            .collect()
    }

    async fn get_workflow_event_subscription_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowEventSubscription>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    target_url,
                    event_types,
                    signing_secret,
                    created_at,
                    updated_at
                FROM control_workflow_event_subscriptions
                WHERE id = $1
                  AND owner_subject = $2
                  AND owner_issuer = $3
                "#,
                &[&id, &principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to load workflow event subscription: {error}"
                ))
            })?;
        row.as_ref()
            .map(row_to_stored_workflow_event_subscription)
            .transpose()
    }

    async fn delete_workflow_event_subscription_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowEventSubscription>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                DELETE FROM control_workflow_event_subscriptions
                WHERE id = $1
                  AND owner_subject = $2
                  AND owner_issuer = $3
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    target_url,
                    event_types,
                    signing_secret,
                    created_at,
                    updated_at
                "#,
                &[&id, &principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to delete workflow event subscription: {error}"
                ))
            })?;
        row.as_ref()
            .map(row_to_stored_workflow_event_subscription)
            .transpose()
    }

    async fn list_workflow_event_deliveries_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        subscription_id: Uuid,
    ) -> Result<Vec<StoredWorkflowEventDelivery>, SessionStoreError> {
        if self
            .get_workflow_event_subscription_for_owner(principal, subscription_id)
            .await?
            .is_none()
        {
            return Ok(Vec::new());
        }
        let rows = self
            .client
            .lock()
            .await
            .query(
                r#"
                SELECT
                    id,
                    subscription_id,
                    run_id,
                    event_id,
                    event_type,
                    target_url,
                    signing_secret,
                    payload,
                    state,
                    attempt_count,
                    next_attempt_at,
                    last_attempt_at,
                    delivered_at,
                    last_response_status,
                    last_error,
                    created_at,
                    updated_at
                FROM control_workflow_event_deliveries
                WHERE subscription_id = $1
                ORDER BY created_at ASC, event_id ASC, id ASC
                "#,
                &[&subscription_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to list workflow event deliveries: {error}"
                ))
            })?;
        rows.iter()
            .map(row_to_stored_workflow_event_delivery)
            .collect()
    }

    async fn list_workflow_event_delivery_attempts_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        subscription_id: Uuid,
    ) -> Result<Vec<StoredWorkflowEventDeliveryAttempt>, SessionStoreError> {
        if self
            .get_workflow_event_subscription_for_owner(principal, subscription_id)
            .await?
            .is_none()
        {
            return Ok(Vec::new());
        }
        let rows = self
            .client
            .lock()
            .await
            .query(
                r#"
                SELECT
                    attempt.id,
                    attempt.delivery_id,
                    attempt.attempt_number,
                    attempt.response_status,
                    attempt.error,
                    attempt.created_at
                FROM control_workflow_event_delivery_attempts attempt
                JOIN control_workflow_event_deliveries delivery
                  ON delivery.id = attempt.delivery_id
                WHERE delivery.subscription_id = $1
                ORDER BY attempt.created_at ASC, attempt.id ASC
                "#,
                &[&subscription_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to list workflow event delivery attempts: {error}"
                ))
            })?;
        rows.iter()
            .map(row_to_stored_workflow_event_delivery_attempt)
            .collect()
    }

    async fn requeue_inflight_workflow_event_deliveries(&self) -> Result<(), SessionStoreError> {
        self.client
            .lock()
            .await
            .execute(
                r#"
                UPDATE control_workflow_event_deliveries
                SET
                    state = 'pending',
                    next_attempt_at = NOW(),
                    updated_at = NOW()
                WHERE state = 'delivering'
                "#,
                &[],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to requeue inflight workflow event deliveries: {error}"
                ))
            })?;
        Ok(())
    }

    async fn claim_due_workflow_event_deliveries(
        &self,
        limit: usize,
        now: DateTime<Utc>,
    ) -> Result<Vec<StoredWorkflowEventDelivery>, SessionStoreError> {
        let limit = i64::try_from(limit).map_err(|error| {
            SessionStoreError::InvalidRequest(format!(
                "workflow event delivery limit is out of range: {error}"
            ))
        })?;
        let rows = self
            .client
            .lock()
            .await
            .query(
                r#"
                WITH claimed AS (
                    SELECT id
                    FROM control_workflow_event_deliveries
                    WHERE state = 'pending'
                      AND (next_attempt_at IS NULL OR next_attempt_at <= $2)
                    ORDER BY created_at ASC, event_id ASC, id ASC
                    FOR UPDATE SKIP LOCKED
                    LIMIT $1
                )
                UPDATE control_workflow_event_deliveries delivery
                SET
                    state = 'delivering',
                    updated_at = $2
                FROM claimed
                WHERE delivery.id = claimed.id
                RETURNING
                    delivery.id,
                    delivery.subscription_id,
                    delivery.run_id,
                    delivery.event_id,
                    delivery.event_type,
                    delivery.target_url,
                    delivery.signing_secret,
                    delivery.payload,
                    delivery.state,
                    delivery.attempt_count,
                    delivery.next_attempt_at,
                    delivery.last_attempt_at,
                    delivery.delivered_at,
                    delivery.last_response_status,
                    delivery.last_error,
                    delivery.created_at,
                    delivery.updated_at
                "#,
                &[&limit, &now],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to claim due workflow event deliveries: {error}"
                ))
            })?;
        let mut deliveries = rows
            .iter()
            .map(row_to_stored_workflow_event_delivery)
            .collect::<Result<Vec<_>, _>>()?;
        deliveries.sort_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| left.event_id.cmp(&right.event_id))
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(deliveries)
    }

    async fn record_workflow_event_delivery_attempt(
        &self,
        delivery_id: Uuid,
        request: RecordWorkflowEventDeliveryAttemptRequest,
    ) -> Result<Option<StoredWorkflowEventDelivery>, SessionStoreError> {
        let response_status = request.response_status.map(i32::from);
        let attempt_number = i32::try_from(request.attempt_number).map_err(|error| {
            SessionStoreError::InvalidRequest(format!(
                "workflow event delivery attempt_number is out of range: {error}"
            ))
        })?;
        let mut client = self.client.lock().await;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;
        let row = transaction
            .query_opt(
                r#"
                UPDATE control_workflow_event_deliveries
                SET
                    state = $2,
                    attempt_count = $3,
                    next_attempt_at = $4,
                    last_attempt_at = $5,
                    delivered_at = $6,
                    last_response_status = $7,
                    last_error = $8,
                    updated_at = $5
                WHERE id = $1
                RETURNING
                    id,
                    subscription_id,
                    run_id,
                    event_id,
                    event_type,
                    target_url,
                    signing_secret,
                    payload,
                    state,
                    attempt_count,
                    next_attempt_at,
                    last_attempt_at,
                    delivered_at,
                    last_response_status,
                    last_error,
                    created_at,
                    updated_at
                "#,
                &[
                    &delivery_id,
                    &request.state.as_str(),
                    &attempt_number,
                    &request.next_attempt_at,
                    &request.attempted_at,
                    &request.delivered_at,
                    &response_status,
                    &request.error,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to update workflow event delivery attempt: {error}"
                ))
            })?;
        let Some(row) = row else {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Ok(None);
        };
        transaction
            .execute(
                r#"
                INSERT INTO control_workflow_event_delivery_attempts (
                    id,
                    delivery_id,
                    attempt_number,
                    response_status,
                    error,
                    created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6)
                "#,
                &[
                    &Uuid::now_v7(),
                    &delivery_id,
                    &attempt_number,
                    &response_status,
                    &request.error,
                    &request.attempted_at,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to insert workflow event delivery attempt: {error}"
                ))
            })?;
        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;
        Ok(Some(row_to_stored_workflow_event_delivery(&row)?))
    }

    async fn list_workflow_run_events_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Vec<StoredWorkflowRunEvent>, SessionStoreError> {
        if self
            .get_workflow_run_for_owner(principal, id)
            .await?
            .is_none()
        {
            return Ok(Vec::new());
        }
        let rows = self
            .client
            .lock()
            .await
            .query(
                r#"
                SELECT
                    id,
                    run_id,
                    event_type,
                    message,
                    data,
                    created_at
                FROM control_workflow_run_events
                WHERE run_id = $1
                ORDER BY created_at ASC, id ASC
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to list workflow run events: {error}"))
            })?;
        rows.iter().map(row_to_stored_workflow_run_event).collect()
    }

    async fn list_workflow_run_events(
        &self,
        id: Uuid,
    ) -> Result<Vec<StoredWorkflowRunEvent>, SessionStoreError> {
        let rows = self
            .client
            .lock()
            .await
            .query(
                r#"
                SELECT
                    id,
                    run_id,
                    event_type,
                    message,
                    data,
                    created_at
                FROM control_workflow_run_events
                WHERE run_id = $1
                ORDER BY created_at ASC, id ASC
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to list workflow run events: {error}"))
            })?;
        rows.iter().map(row_to_stored_workflow_run_event).collect()
    }

    async fn list_workflow_run_logs_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Vec<StoredWorkflowRunLog>, SessionStoreError> {
        if self
            .get_workflow_run_for_owner(principal, id)
            .await?
            .is_none()
        {
            return Ok(Vec::new());
        }
        let rows = self
            .client
            .lock()
            .await
            .query(
                r#"
                SELECT
                    id,
                    run_id,
                    stream,
                    message,
                    created_at
                FROM control_workflow_run_logs
                WHERE run_id = $1
                ORDER BY created_at ASC, id ASC
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to list workflow run logs: {error}"))
            })?;
        rows.iter().map(row_to_stored_workflow_run_log).collect()
    }

    async fn append_workflow_run_event_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        request: PersistWorkflowRunEventRequest,
    ) -> Result<Option<StoredWorkflowRunEvent>, SessionStoreError> {
        let Some(run) = self.get_workflow_run_for_owner(principal, id).await? else {
            return Ok(None);
        };
        let now = Utc::now();
        let mut client = self.client.lock().await;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;
        let event_id = Uuid::now_v7();
        let event = StoredWorkflowRunEvent {
            id: event_id,
            run_id: id,
            event_type: request.event_type,
            message: request.message,
            data: request.data,
            created_at: now,
        };
        let row = transaction
            .query_opt(
                r#"
                WITH inserted AS (
                    INSERT INTO control_workflow_run_events (
                        id,
                        run_id,
                        event_type,
                        message,
                        data,
                        created_at
                    )
                    VALUES ($2, $1, $3, $4, $5::jsonb, $6)
                    RETURNING
                        id,
                        run_id,
                        event_type,
                        message,
                        data,
                        created_at
                )
                UPDATE control_workflow_runs
                SET updated_at = $6
                WHERE id = $1
                RETURNING (SELECT id FROM inserted) AS inserted_id
                "#,
                &[
                    &id,
                    &event_id,
                    &event.event_type,
                    &event.message,
                    &event.data,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to append workflow run event: {error}"))
            })?;
        let Some(row) = row else {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Ok(None);
        };
        let inserted_id: Uuid = row.get("inserted_id");
        if inserted_id != event.id {
            return Err(SessionStoreError::Backend(
                "workflow run event insert returned unexpected id".to_string(),
            ));
        }
        Self::enqueue_workflow_event_deliveries(&transaction, &run, &event).await?;
        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;
        Ok(Some(event))
    }

    async fn append_workflow_run_event(
        &self,
        id: Uuid,
        request: PersistWorkflowRunEventRequest,
    ) -> Result<Option<StoredWorkflowRunEvent>, SessionStoreError> {
        let Some(run) = self.get_workflow_run_by_id(id).await? else {
            return Ok(None);
        };
        let now = Utc::now();
        let mut client = self.client.lock().await;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;
        let event_id = Uuid::now_v7();
        let event = StoredWorkflowRunEvent {
            id: event_id,
            run_id: id,
            event_type: request.event_type,
            message: request.message,
            data: request.data,
            created_at: now,
        };
        let row = transaction
            .query_opt(
                r#"
                WITH inserted AS (
                    INSERT INTO control_workflow_run_events (
                        id,
                        run_id,
                        event_type,
                        message,
                        data,
                        created_at
                    )
                    VALUES ($2, $1, $3, $4, $5::jsonb, $6)
                    RETURNING
                        id,
                        run_id,
                        event_type,
                        message,
                        data,
                        created_at
                )
                UPDATE control_workflow_runs
                SET updated_at = $6
                WHERE id = $1
                RETURNING (SELECT id FROM inserted) AS inserted_id
                "#,
                &[
                    &id,
                    &event_id,
                    &event.event_type,
                    &event.message,
                    &event.data,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to append workflow run event: {error}"))
            })?;
        let Some(row) = row else {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Ok(None);
        };
        let inserted_id: Uuid = row.get("inserted_id");
        if inserted_id != event.id {
            return Err(SessionStoreError::Backend(
                "workflow run event insert returned unexpected id".to_string(),
            ));
        }
        Self::enqueue_workflow_event_deliveries(&transaction, &run, &event).await?;
        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;
        Ok(Some(event))
    }

    async fn transition_workflow_run(
        &self,
        id: Uuid,
        request: WorkflowRunTransitionRequest,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        let mut client = self.client.lock().await;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;

        let run_row = transaction
            .query_opt(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    workflow_definition_id,
                    workflow_definition_version_id,
                    workflow_version,
                    session_id,
                    automation_task_id,
                    state,
                    source_system,
                    source_reference,
                    client_request_id,
                    create_request_fingerprint,
                    source_snapshot,
                    extensions,
                    credential_bindings,
                    workspace_inputs,
                    produced_files,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                FROM control_workflow_runs
                WHERE id = $1
                FOR UPDATE
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to lock workflow run for transition: {error}"
                ))
            })?;
        let Some(run_row) = run_row else {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Ok(None);
        };
        let current_run = row_to_stored_workflow_run(&run_row)?;

        let task_row = transaction
            .query_one(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    display_name,
                    executor,
                    state,
                    session_id,
                    session_source,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    cancel_requested_at,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                FROM control_automation_tasks
                WHERE id = $1
                FOR UPDATE
                "#,
                &[&current_run.automation_task_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to lock automation task for workflow run transition: {error}"
                ))
            })?;
        let current_task = row_to_stored_automation_task(&task_row)?;
        let task_state: AutomationTaskState = request.state.into();
        if current_task.state.is_terminal() {
            return Err(SessionStoreError::Conflict(format!(
                "automation task {} is already terminal",
                current_task.id
            )));
        }
        if !current_task.state.can_transition_to(task_state) {
            return Err(SessionStoreError::Conflict(format!(
                "automation task {} cannot transition from {} to {}",
                current_task.id,
                current_task.state.as_str(),
                task_state.as_str()
            )));
        }

        let now = Utc::now();
        let started_at = if matches!(
            task_state,
            AutomationTaskState::Starting
                | AutomationTaskState::Running
                | AutomationTaskState::AwaitingInput
        ) {
            current_task.started_at.or(Some(now))
        } else {
            current_task.started_at
        };
        let completed_at = if task_state.is_terminal() {
            Some(now)
        } else {
            current_task.completed_at
        };
        let artifact_refs = json_string_array(&request.artifact_refs);
        let task_row = transaction
            .query_one(
                r#"
                UPDATE control_automation_tasks
                SET
                    state = $2,
                    output = $3::jsonb,
                    error = $4,
                    artifact_refs = $5::jsonb,
                    started_at = $6,
                    completed_at = $7,
                    updated_at = $8
                WHERE id = $1
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    display_name,
                    executor,
                    state,
                    session_id,
                    session_source,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    cancel_requested_at,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                "#,
                &[
                    &current_task.id,
                    &task_state.as_str(),
                    &request.output,
                    &request.error,
                    &artifact_refs,
                    &started_at,
                    &completed_at,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to update automation task for workflow run transition: {error}"
                ))
            })?;
        let task = row_to_stored_automation_task(&task_row)?;

        let task_message = request.message.clone().unwrap_or_else(|| {
            automation_task_default_message_for_run_state(request.state).to_string()
        });
        transaction
            .execute(
                r#"
                INSERT INTO control_automation_task_events (
                    id,
                    task_id,
                    event_type,
                    message,
                    data,
                    created_at
                )
                VALUES ($1, $2, $3, $4, $5::jsonb, $6)
                "#,
                &[
                    &Uuid::now_v7(),
                    &task.id,
                    &automation_task_event_type_for_run_state(request.state),
                    &task_message,
                    &request.data,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to insert automation task event for workflow run transition: {error}"
                ))
            })?;

        let run_row = transaction
            .query_one(
                r#"
                UPDATE control_workflow_runs
                SET
                    state = $2,
                    output = $3::jsonb,
                    error = $4,
                    artifact_refs = $5::jsonb,
                    started_at = $6,
                    completed_at = $7,
                    updated_at = $8
                WHERE id = $1
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    workflow_definition_id,
                    workflow_definition_version_id,
                    workflow_version,
                    session_id,
                    automation_task_id,
                    state,
                    source_system,
                    source_reference,
                    client_request_id,
                    create_request_fingerprint,
                    source_snapshot,
                    extensions,
                    credential_bindings,
                    workspace_inputs,
                    produced_files,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                "#,
                &[
                    &id,
                    &request.state.as_str(),
                    &task.output,
                    &task.error,
                    &json_string_array(&task.artifact_refs),
                    &task.started_at,
                    &task.completed_at,
                    &task.updated_at,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to update workflow run state: {error}"))
            })?;

        let run_message = request
            .message
            .unwrap_or_else(|| workflow_run_default_message(request.state).to_string());
        let event = StoredWorkflowRunEvent {
            id: Uuid::now_v7(),
            run_id: id,
            event_type: workflow_run_event_type(request.state).to_string(),
            message: run_message.clone(),
            data: request.data.clone(),
            created_at: now,
        };
        transaction
            .execute(
                r#"
                INSERT INTO control_workflow_run_events (
                    id,
                    run_id,
                    event_type,
                    message,
                    data,
                    created_at
                )
                VALUES ($1, $2, $3, $4, $5::jsonb, $6)
                "#,
                &[
                    &event.id,
                    &id,
                    &event.event_type,
                    &run_message,
                    &event.data,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to insert workflow run transition event: {error}"
                ))
            })?;
        let run = row_to_stored_workflow_run(&run_row)?;
        Self::enqueue_workflow_event_deliveries(&transaction, &run, &event).await?;

        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;

        Ok(Some(run))
    }

    async fn reconcile_workflow_run_from_task(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        let mut client = self.client.lock().await;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;

        let run_row = transaction
            .query_opt(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    workflow_definition_id,
                    workflow_definition_version_id,
                    workflow_version,
                    session_id,
                    automation_task_id,
                    state,
                    source_system,
                    source_reference,
                    client_request_id,
                    create_request_fingerprint,
                    source_snapshot,
                    extensions,
                    credential_bindings,
                    workspace_inputs,
                    produced_files,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                FROM control_workflow_runs
                WHERE id = $1
                FOR UPDATE
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to lock workflow run for reconciliation: {error}"
                ))
            })?;
        let Some(run_row) = run_row else {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Ok(None);
        };
        let current_run = row_to_stored_workflow_run(&run_row)?;

        let task_row = transaction
            .query_one(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    display_name,
                    executor,
                    state,
                    session_id,
                    session_source,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    cancel_requested_at,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                FROM control_automation_tasks
                WHERE id = $1
                FOR UPDATE
                "#,
                &[&current_run.automation_task_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to lock automation task for workflow run reconciliation: {error}"
                ))
            })?;
        let current_task = row_to_stored_automation_task(&task_row)?;
        if !current_task.state.is_terminal() {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Ok(Some(current_run));
        }

        let target_state: WorkflowRunState = current_task.state.into();
        if current_run.state == target_state
            && current_run.output == current_task.output
            && current_run.error == current_task.error
            && current_run.artifact_refs == current_task.artifact_refs
            && current_run.started_at == current_task.started_at
            && current_run.completed_at == current_task.completed_at
        {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Ok(Some(current_run));
        }

        let now = Utc::now();
        let artifact_refs = json_string_array(&current_task.artifact_refs);
        let run_row = transaction
            .query_one(
                r#"
                UPDATE control_workflow_runs
                SET
                    state = $2,
                    output = $3::jsonb,
                    error = $4,
                    artifact_refs = $5::jsonb,
                    started_at = $6,
                    completed_at = $7,
                    updated_at = $8
                WHERE id = $1
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    workflow_definition_id,
                    workflow_definition_version_id,
                    workflow_version,
                    session_id,
                    automation_task_id,
                    state,
                    source_system,
                    source_reference,
                    client_request_id,
                    create_request_fingerprint,
                    source_snapshot,
                    extensions,
                    credential_bindings,
                    workspace_inputs,
                    produced_files,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                "#,
                &[
                    &id,
                    &target_state.as_str(),
                    &current_task.output,
                    &current_task.error,
                    &artifact_refs,
                    &current_task.started_at,
                    &current_task.completed_at,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to reconcile workflow run state from automation task: {error}"
                ))
            })?;

        let event = StoredWorkflowRunEvent {
            id: Uuid::now_v7(),
            run_id: id,
            event_type: workflow_run_event_type(target_state).to_string(),
            message: "workflow run reconciled from terminal automation task state".to_string(),
            data: Some(serde_json::json!({
                "reconciled_from": "automation_task"
            })),
            created_at: now,
        };
        transaction
            .execute(
                r#"
                INSERT INTO control_workflow_run_events (
                    id,
                    run_id,
                    event_type,
                    message,
                    data,
                    created_at
                )
                VALUES ($1, $2, $3, $4, $5::jsonb, $6)
                "#,
                &[
                    &event.id,
                    &id,
                    &event.event_type,
                    &event.message,
                    &event.data,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to append workflow run reconciliation event: {error}"
                ))
            })?;
        let run = row_to_stored_workflow_run(&run_row)?;
        Self::enqueue_workflow_event_deliveries(&transaction, &run, &event).await?;

        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;

        Ok(Some(run))
    }

    async fn list_awaiting_input_workflow_runs(
        &self,
    ) -> Result<Vec<StoredWorkflowRun>, SessionStoreError> {
        let rows = self
            .client
            .lock()
            .await
            .query(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    workflow_definition_id,
                    workflow_definition_version_id,
                    workflow_version,
                    session_id,
                    automation_task_id,
                    state,
                    source_system,
                    source_reference,
                    client_request_id,
                    create_request_fingerprint,
                    source_snapshot,
                    extensions,
                    credential_bindings,
                    workspace_inputs,
                    produced_files,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                FROM control_workflow_runs
                WHERE state = 'awaiting_input'
                ORDER BY updated_at ASC, id ASC
                "#,
                &[],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to list awaiting-input workflow runs: {error}"
                ))
            })?;
        rows.into_iter()
            .map(|row| row_to_stored_workflow_run(&row))
            .collect()
    }

    async fn append_workflow_run_log(
        &self,
        id: Uuid,
        request: PersistWorkflowRunLogRequest,
    ) -> Result<Option<StoredWorkflowRunLog>, SessionStoreError> {
        let now = Utc::now();
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                WITH inserted AS (
                    INSERT INTO control_workflow_run_logs (
                        id,
                        run_id,
                        stream,
                        message,
                        created_at
                    )
                    SELECT $2, $1, $3, $4, $5
                    WHERE EXISTS (
                        SELECT 1
                        FROM control_workflow_runs
                        WHERE id = $1
                    )
                    RETURNING
                        id,
                        run_id,
                        stream,
                        message,
                        created_at
                )
                UPDATE control_workflow_runs
                SET updated_at = $5
                WHERE id = $1
                  AND EXISTS (SELECT 1 FROM inserted)
                RETURNING (SELECT id FROM inserted) AS inserted_id
                "#,
                &[
                    &id,
                    &Uuid::now_v7(),
                    &request.stream.as_str(),
                    &request.message,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to append workflow run log: {error}"))
            })?;
        let Some(row) = row else {
            return Ok(None);
        };
        let inserted_id: Option<Uuid> = row.get("inserted_id");
        let Some(inserted_id) = inserted_id else {
            return Ok(None);
        };
        let log_row = self
            .client
            .lock()
            .await
            .query_one(
                r#"
                SELECT
                    id,
                    run_id,
                    stream,
                    message,
                    created_at
                FROM control_workflow_run_logs
                WHERE id = $1
                "#,
                &[&inserted_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to reload workflow run log: {error}"))
            })?;
        row_to_stored_workflow_run_log(&log_row).map(Some)
    }

    async fn append_workflow_run_produced_file(
        &self,
        id: Uuid,
        request: PersistWorkflowRunProducedFileRequest,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        let mut client = self.client.lock().await;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;

        let run_row = transaction
            .query_opt(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    workflow_definition_id,
                    workflow_definition_version_id,
                    workflow_version,
                    session_id,
                    automation_task_id,
                    state,
                    source_system,
                    source_reference,
                    client_request_id,
                    create_request_fingerprint,
                    source_snapshot,
                    extensions,
                    credential_bindings,
                    workspace_inputs,
                    produced_files,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                FROM control_workflow_runs
                WHERE id = $1
                FOR UPDATE
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to lock workflow run for produced file append: {error}"
                ))
            })?;
        let Some(run_row) = run_row else {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Ok(None);
        };

        let mut run = row_to_stored_workflow_run(&run_row)?;
        if run
            .produced_files
            .iter()
            .any(|file| file.file_id == request.file_id)
        {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Err(SessionStoreError::Conflict(format!(
                "workflow run {id} already contains produced file {}",
                request.file_id
            )));
        }

        let now = Utc::now();
        let produced_file = WorkflowRunProducedFile {
            workspace_id: request.workspace_id,
            file_id: request.file_id,
            file_name: request.file_name,
            media_type: request.media_type,
            byte_count: request.byte_count,
            sha256_hex: request.sha256_hex,
            provenance: request.provenance,
            artifact_ref: request.artifact_ref,
            created_at: now,
        };
        run.produced_files.push(produced_file.clone());

        let row = transaction
            .query_one(
                r#"
                UPDATE control_workflow_runs
                SET
                    produced_files = $2::jsonb,
                    updated_at = $3
                WHERE id = $1
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    workflow_definition_id,
                    workflow_definition_version_id,
                    workflow_version,
                    session_id,
                    automation_task_id,
                    state,
                    source_system,
                    source_reference,
                    client_request_id,
                    create_request_fingerprint,
                    source_snapshot,
                    extensions,
                    credential_bindings,
                    workspace_inputs,
                    produced_files,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                "#,
                &[
                    &id,
                    &json_workflow_run_produced_files(&run.produced_files)?,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to update workflow run produced files: {error}"
                ))
            })?;

        let event = StoredWorkflowRunEvent {
            id: Uuid::now_v7(),
            run_id: id,
            event_type: "workflow_run.produced_file_added".to_string(),
            message: format!(
                "workflow run produced file {} stored in workspace {}",
                produced_file.file_id, produced_file.workspace_id
            ),
            data: Some(serde_json::json!({
                "workspace_id": produced_file.workspace_id,
                "file_id": produced_file.file_id,
                "file_name": produced_file.file_name,
            })),
            created_at: now,
        };
        transaction
            .execute(
                r#"
                INSERT INTO control_workflow_run_events (
                    id,
                    run_id,
                    event_type,
                    message,
                    data,
                    created_at
                )
                VALUES ($1, $2, $3, $4, $5::jsonb, $6)
                "#,
                &[
                    &event.id,
                    &id,
                    &event.event_type,
                    &event.message,
                    &event.data,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to insert workflow produced file event: {error}"
                ))
            })?;
        let updated_run = row_to_stored_workflow_run(&row)?;
        Self::enqueue_workflow_event_deliveries(&transaction, &updated_run, &event).await?;

        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;

        Ok(Some(updated_run))
    }

    async fn list_workflow_run_log_retention_candidates(
        &self,
        now: DateTime<Utc>,
        retention: ChronoDuration,
    ) -> Result<Vec<WorkflowRunLogRetentionCandidate>, SessionStoreError> {
        let retention_secs = retention.num_seconds() as f64;
        let rows = self
            .client
            .lock()
            .await
            .query(
                r#"
                SELECT
                    run.id AS run_id,
                    run.automation_task_id,
                    run.session_id,
                    run.completed_at
                FROM control_workflow_runs run
                WHERE run.completed_at IS NOT NULL
                  AND EXTRACT(EPOCH FROM ($1 - run.completed_at)) >= $2::DOUBLE PRECISION
                  AND (
                    EXISTS (
                        SELECT 1
                        FROM control_workflow_run_logs logs
                        WHERE logs.run_id = run.id
                    )
                    OR EXISTS (
                        SELECT 1
                        FROM control_automation_task_logs logs
                        WHERE logs.task_id = run.automation_task_id
                    )
                  )
                ORDER BY run.completed_at ASC, run.id ASC
                "#,
                &[&now, &retention_secs],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to list workflow run log retention candidates: {}",
                    describe_postgres_error(&error)
                ))
            })?;
        Ok(rows
            .iter()
            .map(|row| {
                let completed_at: DateTime<Utc> = row.get("completed_at");
                WorkflowRunLogRetentionCandidate {
                    run_id: row.get("run_id"),
                    automation_task_id: row.get("automation_task_id"),
                    session_id: row.get("session_id"),
                    expires_at: completed_at + retention,
                }
            })
            .collect())
    }

    async fn delete_workflow_run_logs(
        &self,
        run_id: Uuid,
        automation_task_id: Uuid,
    ) -> Result<usize, SessionStoreError> {
        let mut client = self.client.lock().await;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;
        let run_deleted = transaction
            .execute(
                "DELETE FROM control_workflow_run_logs WHERE run_id = $1",
                &[&run_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to delete workflow run logs for {run_id}: {error}"
                ))
            })? as usize;
        let task_deleted = transaction
            .execute(
                "DELETE FROM control_automation_task_logs WHERE task_id = $1",
                &[&automation_task_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to delete automation task logs for {automation_task_id}: {error}"
                ))
            })? as usize;
        let now = Utc::now();
        transaction
            .execute(
                "UPDATE control_workflow_runs SET updated_at = $2 WHERE id = $1",
                &[&run_id, &now],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to update workflow run after log deletion: {error}"
                ))
            })?;
        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;
        Ok(run_deleted + task_deleted)
    }

    async fn list_workflow_run_output_retention_candidates(
        &self,
        now: DateTime<Utc>,
        retention: ChronoDuration,
    ) -> Result<Vec<WorkflowRunOutputRetentionCandidate>, SessionStoreError> {
        let retention_secs = retention.num_seconds() as f64;
        let rows = self
            .client
            .lock()
            .await
            .query(
                r#"
                SELECT
                    run.id AS run_id,
                    run.session_id,
                    run.completed_at
                FROM control_workflow_runs run
                WHERE run.completed_at IS NOT NULL
                  AND run.output IS NOT NULL
                  AND EXTRACT(EPOCH FROM ($1 - run.completed_at)) >= $2::DOUBLE PRECISION
                ORDER BY run.completed_at ASC, run.id ASC
                "#,
                &[&now, &retention_secs],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to list workflow run output retention candidates: {}",
                    describe_postgres_error(&error)
                ))
            })?;
        Ok(rows
            .iter()
            .map(|row| {
                let completed_at: DateTime<Utc> = row.get("completed_at");
                WorkflowRunOutputRetentionCandidate {
                    run_id: row.get("run_id"),
                    session_id: row.get("session_id"),
                    expires_at: completed_at + retention,
                }
            })
            .collect())
    }

    async fn clear_workflow_run_output(
        &self,
        run_id: Uuid,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                UPDATE control_workflow_runs
                SET
                    output = NULL,
                    updated_at = $2
                WHERE id = $1
                RETURNING
                    id,
                    workflow_definition_id,
                    workflow_definition_version_id,
                    workflow_version,
                    session_id,
                    automation_task_id,
                    state,
                    source_snapshot,
                    extensions,
                    credential_bindings,
                    workspace_inputs,
                    produced_files,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                "#,
                &[&run_id, &Utc::now()],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to clear workflow run output: {error}"))
            })?;
        row.as_ref().map(row_to_stored_workflow_run).transpose()
    }

    async fn create_credential_binding(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistCredentialBindingRequest,
    ) -> Result<StoredCredentialBinding, SessionStoreError> {
        let now = Utc::now();
        let totp = request
            .totp
            .as_ref()
            .map(|totp| {
                serde_json::to_value(totp).map_err(|error| {
                    SessionStoreError::Backend(format!(
                        "failed to encode credential binding totp metadata: {error}"
                    ))
                })
            })
            .transpose()?;
        let row = self
            .client
            .lock()
            .await
            .query_one(
                r#"
                INSERT INTO control_credential_bindings (
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    provider,
                    external_ref,
                    namespace,
                    allowed_origins,
                    injection_mode,
                    totp,
                    labels,
                    created_at,
                    updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8::jsonb, $9, $10::jsonb, $11::jsonb, $12, $12)
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    provider,
                    external_ref,
                    namespace,
                    allowed_origins,
                    injection_mode,
                    totp,
                    labels,
                    created_at,
                    updated_at
                "#,
                &[
                    &request.id,
                    &principal.subject,
                    &principal.issuer,
                    &request.name,
                    &request.provider.as_str(),
                    &request.external_ref,
                    &request.namespace,
                    &json_string_array(&request.allowed_origins),
                    &request.injection_mode.as_str(),
                    &totp,
                    &json_labels(&request.labels),
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to create credential binding: {error}"))
            })?;
        row_to_stored_credential_binding(&row)
    }

    async fn list_credential_bindings_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredCredentialBinding>, SessionStoreError> {
        let rows = self
            .client
            .lock()
            .await
            .query(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    provider,
                    external_ref,
                    namespace,
                    allowed_origins,
                    injection_mode,
                    totp,
                    labels,
                    created_at,
                    updated_at
                FROM control_credential_bindings
                WHERE owner_subject = $1
                  AND owner_issuer = $2
                ORDER BY created_at DESC
                "#,
                &[&principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to list credential bindings: {error}"))
            })?;
        rows.iter().map(row_to_stored_credential_binding).collect()
    }

    async fn get_credential_binding_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredCredentialBinding>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    provider,
                    external_ref,
                    namespace,
                    allowed_origins,
                    injection_mode,
                    totp,
                    labels,
                    created_at,
                    updated_at
                FROM control_credential_bindings
                WHERE id = $1
                  AND owner_subject = $2
                  AND owner_issuer = $3
                "#,
                &[&id, &principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to fetch credential binding: {error}"))
            })?;
        row.map(|row| row_to_stored_credential_binding(&row))
            .transpose()
    }

    async fn create_extension_definition(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistExtensionDefinitionRequest,
    ) -> Result<StoredExtensionDefinition, SessionStoreError> {
        let now = Utc::now();
        let row = self
            .client
            .lock()
            .await
            .query_one(
                r#"
                INSERT INTO control_extensions (
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    description,
                    enabled,
                    latest_version,
                    labels,
                    created_at,
                    updated_at
                )
                VALUES ($1, $2, $3, $4, $5, TRUE, NULL, $6::jsonb, $7, $7)
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    description,
                    enabled,
                    latest_version,
                    labels,
                    created_at,
                    updated_at
                "#,
                &[
                    &Uuid::now_v7(),
                    &principal.subject,
                    &principal.issuer,
                    &request.name,
                    &request.description,
                    &json_labels(&request.labels),
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to create extension: {error}"))
            })?;
        row_to_stored_extension_definition(&row)
    }

    async fn list_extension_definitions_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredExtensionDefinition>, SessionStoreError> {
        let rows = self
            .client
            .lock()
            .await
            .query(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    description,
                    enabled,
                    latest_version,
                    labels,
                    created_at,
                    updated_at
                FROM control_extensions
                WHERE owner_subject = $1
                  AND owner_issuer = $2
                ORDER BY created_at DESC
                "#,
                &[&principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to list extensions: {error}"))
            })?;
        rows.iter()
            .map(row_to_stored_extension_definition)
            .collect()
    }

    async fn get_extension_definition_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredExtensionDefinition>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    description,
                    enabled,
                    latest_version,
                    labels,
                    created_at,
                    updated_at
                FROM control_extensions
                WHERE id = $1
                  AND owner_subject = $2
                  AND owner_issuer = $3
                "#,
                &[&id, &principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to fetch extension: {error}"))
            })?;
        row.map(|row| row_to_stored_extension_definition(&row))
            .transpose()
    }

    async fn set_extension_definition_enabled_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        enabled: bool,
    ) -> Result<Option<StoredExtensionDefinition>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                UPDATE control_extensions
                SET enabled = $4, updated_at = NOW()
                WHERE id = $1
                  AND owner_subject = $2
                  AND owner_issuer = $3
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    description,
                    enabled,
                    latest_version,
                    labels,
                    created_at,
                    updated_at
                "#,
                &[&id, &principal.subject, &principal.issuer, &enabled],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to update extension: {error}"))
            })?;
        row.map(|row| row_to_stored_extension_definition(&row))
            .transpose()
    }

    async fn create_extension_version_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistExtensionVersionRequest,
    ) -> Result<StoredExtensionVersion, SessionStoreError> {
        let mut client = self.client.lock().await;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;
        let definition = transaction
            .query_opt(
                r#"
                SELECT id
                FROM control_extensions
                WHERE id = $1
                  AND owner_subject = $2
                  AND owner_issuer = $3
                "#,
                &[
                    &request.extension_definition_id,
                    &principal.subject,
                    &principal.issuer,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to validate extension: {error}"))
            })?;
        if definition.is_none() {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Err(SessionStoreError::NotFound(format!(
                "extension {} not found",
                request.extension_definition_id
            )));
        }

        let now = Utc::now();
        let row = transaction
            .query_one(
                r#"
                INSERT INTO control_extension_versions (
                    id,
                    extension_definition_id,
                    version,
                    install_path,
                    created_at
                )
                VALUES ($1, $2, $3, $4, $5)
                RETURNING
                    id,
                    extension_definition_id,
                    version,
                    install_path,
                    created_at
                "#,
                &[
                    &Uuid::now_v7(),
                    &request.extension_definition_id,
                    &request.version,
                    &request.install_path,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                if error.code().is_some_and(|code| code.code() == "23505") {
                    return SessionStoreError::Conflict(format!(
                        "extension {} already has version {}",
                        request.extension_definition_id, request.version
                    ));
                }
                SessionStoreError::Backend(format!("failed to create extension version: {error}"))
            })?;

        transaction
            .execute(
                r#"
                UPDATE control_extensions
                SET latest_version = $2, updated_at = $3
                WHERE id = $1
                "#,
                &[&request.extension_definition_id, &request.version, &now],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to update extension latest_version: {error}"
                ))
            })?;

        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;
        row_to_stored_extension_version(&row)
    }

    async fn get_latest_extension_version_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        extension_definition_id: Uuid,
    ) -> Result<Option<StoredExtensionVersion>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                SELECT
                    version.id,
                    version.extension_definition_id,
                    version.version,
                    version.install_path,
                    version.created_at
                FROM control_extension_versions version
                JOIN control_extensions extension
                  ON extension.id = version.extension_definition_id
                WHERE version.extension_definition_id = $1
                  AND extension.owner_subject = $2
                  AND extension.owner_issuer = $3
                ORDER BY version.created_at DESC, version.id DESC
                LIMIT 1
                "#,
                &[
                    &extension_definition_id,
                    &principal.subject,
                    &principal.issuer,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to fetch latest extension version: {error}"
                ))
            })?;
        row.map(|row| row_to_stored_extension_version(&row))
            .transpose()
    }

    async fn create_file_workspace(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistFileWorkspaceRequest,
    ) -> Result<StoredFileWorkspace, SessionStoreError> {
        let now = Utc::now();
        let row = self
            .client
            .lock()
            .await
            .query_one(
                r#"
                INSERT INTO control_file_workspaces (
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    description,
                    labels,
                    created_at,
                    updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $7)
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    description,
                    labels,
                    created_at,
                    updated_at
                "#,
                &[
                    &Uuid::now_v7(),
                    &principal.subject,
                    &principal.issuer,
                    &request.name,
                    &request.description,
                    &json_labels(&request.labels),
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to create file workspace: {error}"))
            })?;
        row_to_stored_file_workspace(&row)
    }

    async fn list_file_workspaces_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredFileWorkspace>, SessionStoreError> {
        let rows = self
            .client
            .lock()
            .await
            .query(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    description,
                    labels,
                    created_at,
                    updated_at
                FROM control_file_workspaces
                WHERE owner_subject = $1
                  AND owner_issuer = $2
                ORDER BY created_at DESC
                "#,
                &[&principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to list file workspaces: {error}"))
            })?;
        rows.iter().map(row_to_stored_file_workspace).collect()
    }

    async fn get_file_workspace_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredFileWorkspace>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    description,
                    labels,
                    created_at,
                    updated_at
                FROM control_file_workspaces
                WHERE id = $1
                  AND owner_subject = $2
                  AND owner_issuer = $3
                "#,
                &[&id, &principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to fetch file workspace: {error}"))
            })?;
        row.as_ref().map(row_to_stored_file_workspace).transpose()
    }

    async fn create_file_workspace_file_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistFileWorkspaceFileRequest,
    ) -> Result<StoredFileWorkspaceFile, SessionStoreError> {
        let Some(_) = self
            .get_file_workspace_for_owner(principal, request.workspace_id)
            .await?
        else {
            return Err(SessionStoreError::NotFound(format!(
                "file workspace {} not found",
                request.workspace_id
            )));
        };

        let now = Utc::now();
        let row = self
            .client
            .lock()
            .await
            .query_one(
                r#"
                INSERT INTO control_file_workspace_files (
                    id,
                    workspace_id,
                    name,
                    media_type,
                    byte_count,
                    sha256_hex,
                    provenance,
                    artifact_ref,
                    created_at,
                    updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9)
                RETURNING
                    id,
                    workspace_id,
                    name,
                    media_type,
                    byte_count,
                    sha256_hex,
                    provenance,
                    artifact_ref,
                    created_at,
                    updated_at
                "#,
                &[
                    &request.id,
                    &request.workspace_id,
                    &request.name,
                    &request.media_type,
                    &(request.byte_count as i64),
                    &request.sha256_hex,
                    &request.provenance,
                    &request.artifact_ref,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to create workspace file: {error}"))
            })?;
        row_to_stored_file_workspace_file(&row)
    }

    async fn list_file_workspace_files_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        workspace_id: Uuid,
    ) -> Result<Vec<StoredFileWorkspaceFile>, SessionStoreError> {
        if self
            .get_file_workspace_for_owner(principal, workspace_id)
            .await?
            .is_none()
        {
            return Ok(Vec::new());
        }

        let rows = self
            .client
            .lock()
            .await
            .query(
                r#"
                SELECT
                    file.id,
                    file.workspace_id,
                    file.name,
                    file.media_type,
                    file.byte_count,
                    file.sha256_hex,
                    file.provenance,
                    file.artifact_ref,
                    file.created_at,
                    file.updated_at
                FROM control_file_workspace_files file
                WHERE file.workspace_id = $1
                ORDER BY file.created_at DESC
                "#,
                &[&workspace_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to list workspace files: {error}"))
            })?;
        rows.iter().map(row_to_stored_file_workspace_file).collect()
    }

    async fn get_file_workspace_file_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        workspace_id: Uuid,
        file_id: Uuid,
    ) -> Result<Option<StoredFileWorkspaceFile>, SessionStoreError> {
        let Some(_) = self
            .get_file_workspace_for_owner(principal, workspace_id)
            .await?
        else {
            return Ok(None);
        };

        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                SELECT
                    id,
                    workspace_id,
                    name,
                    media_type,
                    byte_count,
                    sha256_hex,
                    provenance,
                    artifact_ref,
                    created_at,
                    updated_at
                FROM control_file_workspace_files
                WHERE workspace_id = $1
                  AND id = $2
                "#,
                &[&workspace_id, &file_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to fetch workspace file: {error}"))
            })?;
        row.as_ref()
            .map(row_to_stored_file_workspace_file)
            .transpose()
    }

    async fn delete_file_workspace_file_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        workspace_id: Uuid,
        file_id: Uuid,
    ) -> Result<Option<StoredFileWorkspaceFile>, SessionStoreError> {
        let Some(_) = self
            .get_file_workspace_for_owner(principal, workspace_id)
            .await?
        else {
            return Ok(None);
        };

        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                DELETE FROM control_file_workspace_files
                WHERE workspace_id = $1
                  AND id = $2
                RETURNING
                    id,
                    workspace_id,
                    name,
                    media_type,
                    byte_count,
                    sha256_hex,
                    provenance,
                    artifact_ref,
                    created_at,
                    updated_at
                "#,
                &[&workspace_id, &file_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to delete workspace file: {error}"))
            })?;
        row.as_ref()
            .map(row_to_stored_file_workspace_file)
            .transpose()
    }

    async fn create_recording_for_session(
        &self,
        session_id: Uuid,
        format: SessionRecordingFormat,
        previous_recording_id: Option<Uuid>,
    ) -> Result<StoredSessionRecording, SessionStoreError> {
        let mut client = self.client.lock().await;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;

        let active = transaction
            .query_opt(
                r#"
                SELECT id
                FROM control_session_recordings
                WHERE session_id = $1
                  AND state IN ('starting', 'recording', 'finalizing')
                ORDER BY updated_at DESC, created_at DESC
                LIMIT 1
                FOR UPDATE
                "#,
                &[&session_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to check active recordings: {error}"))
            })?;
        if let Some(active) = active {
            let active_id: Uuid = active.get("id");
            return Err(SessionStoreError::Conflict(format!(
                "session {session_id} already has active recording {active_id}"
            )));
        }

        let now = Utc::now();
        let recording_id = Uuid::now_v7();
        let row = transaction
            .query_one(
                r#"
                INSERT INTO control_session_recordings (
                    id,
                    session_id,
                    previous_recording_id,
                    state,
                    format,
                    mime_type,
                    started_at,
                    created_at,
                    updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $7, $7)
                RETURNING
                    id,
                    session_id,
                    previous_recording_id,
                    state,
                    format,
                    mime_type,
                    byte_count,
                    duration_ms,
                    error,
                    termination_reason,
                    artifact_path AS artifact_ref,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                "#,
                &[
                    &recording_id,
                    &session_id,
                    &previous_recording_id,
                    &SessionRecordingState::Recording.as_str(),
                    &format.as_str(),
                    &recording_mime_type(format),
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to insert recording: {error}"))
            })?;

        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;

        row_to_stored_session_recording(&row)
    }

    async fn list_recordings_for_session(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<StoredSessionRecording>, SessionStoreError> {
        let rows = self
            .client
            .lock()
            .await
            .query(
                r#"
                SELECT
                    id,
                    session_id,
                    previous_recording_id,
                    state,
                    format,
                    mime_type,
                    byte_count,
                    duration_ms,
                    error,
                    termination_reason,
                    artifact_path AS artifact_ref,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                FROM control_session_recordings
                WHERE session_id = $1
                ORDER BY created_at DESC, updated_at DESC
                "#,
                &[&session_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to list session recordings: {error}"))
            })?;

        rows.iter().map(row_to_stored_session_recording).collect()
    }

    async fn get_recording_for_session(
        &self,
        session_id: Uuid,
        recording_id: Uuid,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                SELECT
                    id,
                    session_id,
                    previous_recording_id,
                    state,
                    format,
                    mime_type,
                    byte_count,
                    duration_ms,
                    error,
                    termination_reason,
                    artifact_path AS artifact_ref,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                FROM control_session_recordings
                WHERE session_id = $1 AND id = $2
                "#,
                &[&session_id, &recording_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to load session recording: {error}"))
            })?;
        row.as_ref()
            .map(row_to_stored_session_recording)
            .transpose()
    }

    async fn get_latest_recording_for_session(
        &self,
        session_id: Uuid,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                SELECT
                    id,
                    session_id,
                    previous_recording_id,
                    state,
                    format,
                    mime_type,
                    byte_count,
                    duration_ms,
                    error,
                    termination_reason,
                    artifact_path AS artifact_ref,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                FROM control_session_recordings
                WHERE session_id = $1
                ORDER BY updated_at DESC, created_at DESC
                LIMIT 1
                "#,
                &[&session_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to load latest recording: {error}"))
            })?;
        row.as_ref()
            .map(row_to_stored_session_recording)
            .transpose()
    }

    async fn list_recording_artifact_retention_candidates(
        &self,
        now: DateTime<Utc>,
    ) -> Result<Vec<RecordingArtifactRetentionCandidate>, SessionStoreError> {
        let rows = self
            .client
            .lock()
            .await
            .query(
                r#"
                SELECT
                    r.session_id,
                    r.id AS recording_id,
                    r.artifact_path AS artifact_ref,
                    r.completed_at,
                    ((s.recording ->> 'retention_sec')::INTEGER) AS retention_sec
                FROM control_session_recordings r
                INNER JOIN control_sessions s
                    ON s.id = r.session_id
                WHERE r.state = 'ready'
                  AND r.artifact_path IS NOT NULL
                  AND r.completed_at IS NOT NULL
                  AND (s.recording ->> 'retention_sec') IS NOT NULL
                ORDER BY r.completed_at ASC, r.created_at ASC
                "#,
                &[],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to list recording artifact retention candidates: {error}"
                ))
            })?;

        let mut candidates = rows
            .iter()
            .filter_map(|row| {
                let completed_at = row.get::<_, DateTime<Utc>>("completed_at");
                let retention_sec = row.get::<_, i32>("retention_sec");
                let expires_at = completed_at + ChronoDuration::seconds(i64::from(retention_sec));
                if expires_at > now {
                    return None;
                }
                Some(RecordingArtifactRetentionCandidate {
                    session_id: row.get("session_id"),
                    recording_id: row.get("recording_id"),
                    artifact_ref: row.get("artifact_ref"),
                    expires_at,
                })
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| left.expires_at.cmp(&right.expires_at));
        Ok(candidates)
    }

    async fn stop_recording_for_session(
        &self,
        session_id: Uuid,
        recording_id: Uuid,
        termination_reason: SessionRecordingTerminationReason,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                UPDATE control_session_recordings
                SET
                    state = 'finalizing',
                    termination_reason = $3,
                    updated_at = NOW()
                WHERE session_id = $1
                  AND id = $2
                  AND state IN ('starting', 'recording', 'finalizing')
                RETURNING
                    id,
                    session_id,
                    previous_recording_id,
                    state,
                    format,
                    mime_type,
                    byte_count,
                    duration_ms,
                    error,
                    termination_reason,
                    artifact_path AS artifact_ref,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                "#,
                &[&session_id, &recording_id, &termination_reason.as_str()],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to stop recording: {error}"))
            })?;
        if let Some(row) = row {
            return row_to_stored_session_recording(&row).map(Some);
        }

        let existing = self
            .get_recording_for_session(session_id, recording_id)
            .await?;
        if existing.is_some() {
            return Err(SessionStoreError::Conflict(format!(
                "recording {recording_id} is not active"
            )));
        }
        Ok(None)
    }

    async fn complete_recording_for_session(
        &self,
        session_id: Uuid,
        recording_id: Uuid,
        request: PersistCompletedSessionRecordingRequest,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                UPDATE control_session_recordings
                SET
                    state = 'ready',
                    artifact_path = $3,
                    mime_type = COALESCE($4, mime_type),
                    byte_count = $5,
                    duration_ms = $6,
                    error = NULL,
                    completed_at = NOW(),
                    updated_at = NOW()
                WHERE session_id = $1
                  AND id = $2
                  AND state IN ('starting', 'recording', 'finalizing')
                RETURNING
                    id,
                    session_id,
                    previous_recording_id,
                    state,
                    format,
                    mime_type,
                    byte_count,
                    duration_ms,
                    error,
                    termination_reason,
                    artifact_path AS artifact_ref,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                "#,
                &[
                    &session_id,
                    &recording_id,
                    &request.artifact_ref,
                    &request.mime_type,
                    &request.bytes.map(|value| value as i64),
                    &request.duration_ms.map(|value| value as i64),
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to complete recording: {error}"))
            })?;
        if let Some(row) = row {
            return row_to_stored_session_recording(&row).map(Some);
        }

        let existing = self
            .get_recording_for_session(session_id, recording_id)
            .await?;
        if existing.is_some() {
            return Err(SessionStoreError::Conflict(format!(
                "recording {recording_id} is not active"
            )));
        }
        Ok(None)
    }

    async fn clear_recording_artifact_path(
        &self,
        session_id: Uuid,
        recording_id: Uuid,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                UPDATE control_session_recordings
                SET
                    artifact_path = NULL,
                    updated_at = NOW()
                WHERE session_id = $1
                  AND id = $2
                RETURNING
                    id,
                    session_id,
                    previous_recording_id,
                    state,
                    format,
                    mime_type,
                    byte_count,
                    duration_ms,
                    error,
                    termination_reason,
                    artifact_path AS artifact_ref,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                "#,
                &[&session_id, &recording_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to clear recording artifact path: {error}"
                ))
            })?;
        row.as_ref()
            .map(row_to_stored_session_recording)
            .transpose()
    }

    async fn fail_recording_for_session(
        &self,
        session_id: Uuid,
        recording_id: Uuid,
        request: FailSessionRecordingRequest,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                UPDATE control_session_recordings
                SET
                    state = 'failed',
                    error = $3,
                    termination_reason = $4,
                    completed_at = NOW(),
                    updated_at = NOW()
                WHERE session_id = $1
                  AND id = $2
                  AND state IN ('starting', 'recording', 'finalizing', 'failed')
                RETURNING
                    id,
                    session_id,
                    previous_recording_id,
                    state,
                    format,
                    mime_type,
                    byte_count,
                    duration_ms,
                    error,
                    termination_reason,
                    artifact_path AS artifact_ref,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                "#,
                &[
                    &session_id,
                    &recording_id,
                    &request.error,
                    &request
                        .termination_reason
                        .map(|reason| reason.as_str().to_string()),
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to fail recording: {error}"))
            })?;
        if let Some(row) = row {
            return row_to_stored_session_recording(&row).map(Some);
        }

        let existing = self
            .get_recording_for_session(session_id, recording_id)
            .await?;
        if let Some(existing) = existing {
            if matches!(existing.state, SessionRecordingState::Ready) {
                return Err(SessionStoreError::Conflict(format!(
                    "recording {recording_id} is already complete"
                )));
            }
        } else {
            return Ok(None);
        }

        self.get_recording_for_session(session_id, recording_id)
            .await
    }

    async fn upsert_runtime_assignment(
        &self,
        assignment: PersistedSessionRuntimeAssignment,
    ) -> Result<(), SessionStoreError> {
        self.client
            .lock()
            .await
            .execute(
                r#"
                INSERT INTO control_session_runtimes (
                    session_id,
                    runtime_binding,
                    status,
                    agent_socket_path,
                    container_name,
                    cdp_endpoint,
                    created_at,
                    updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, NOW(), NOW())
                ON CONFLICT (session_id)
                DO UPDATE SET
                    runtime_binding = EXCLUDED.runtime_binding,
                    status = EXCLUDED.status,
                    agent_socket_path = EXCLUDED.agent_socket_path,
                    container_name = EXCLUDED.container_name,
                    cdp_endpoint = EXCLUDED.cdp_endpoint,
                    updated_at = NOW()
                "#,
                &[
                    &assignment.session_id,
                    &assignment.runtime_binding,
                    &assignment.status.as_str(),
                    &assignment.agent_socket_path,
                    &assignment.container_name,
                    &assignment.cdp_endpoint,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to upsert runtime assignment: {error}"))
            })?;
        Ok(())
    }

    async fn clear_runtime_assignment(&self, id: Uuid) -> Result<(), SessionStoreError> {
        self.client
            .lock()
            .await
            .execute(
                "DELETE FROM control_session_runtimes WHERE session_id = $1",
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to clear runtime assignment: {error}"))
            })?;
        Ok(())
    }

    async fn upsert_recording_worker_assignment(
        &self,
        assignment: PersistedSessionRecordingWorkerAssignment,
    ) -> Result<(), SessionStoreError> {
        let process_id = assignment.process_id.map(i64::from);
        self.client
            .lock()
            .await
            .execute(
                r#"
                INSERT INTO control_session_recording_workers (
                    session_id,
                    recording_id,
                    status,
                    process_id,
                    created_at,
                    updated_at
                )
                VALUES ($1, $2, $3, $4, NOW(), NOW())
                ON CONFLICT (session_id)
                DO UPDATE SET
                    recording_id = EXCLUDED.recording_id,
                    status = EXCLUDED.status,
                    process_id = EXCLUDED.process_id,
                    updated_at = NOW()
                "#,
                &[
                    &assignment.session_id,
                    &assignment.recording_id,
                    &assignment.status.as_str(),
                    &process_id,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to upsert recording worker assignment: {error}"
                ))
            })?;
        Ok(())
    }

    async fn clear_recording_worker_assignment(&self, id: Uuid) -> Result<(), SessionStoreError> {
        self.client
            .lock()
            .await
            .execute(
                "DELETE FROM control_session_recording_workers WHERE session_id = $1",
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to clear recording worker assignment: {error}"
                ))
            })?;
        Ok(())
    }

    async fn get_recording_worker_assignment(
        &self,
        id: Uuid,
    ) -> Result<Option<PersistedSessionRecordingWorkerAssignment>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                SELECT
                    session_id,
                    recording_id,
                    status,
                    process_id
                FROM control_session_recording_workers
                WHERE session_id = $1
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to load recording worker assignment: {error}"
                ))
            })?;
        row.as_ref()
            .map(row_to_recording_worker_assignment)
            .transpose()
    }

    async fn list_recording_worker_assignments(
        &self,
    ) -> Result<Vec<PersistedSessionRecordingWorkerAssignment>, SessionStoreError> {
        let rows = self
            .client
            .lock()
            .await
            .query(
                r#"
                SELECT
                    session_id,
                    recording_id,
                    status,
                    process_id
                FROM control_session_recording_workers
                ORDER BY updated_at DESC, created_at DESC
                "#,
                &[],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to list recording worker assignments: {error}"
                ))
            })?;

        rows.iter()
            .map(row_to_recording_worker_assignment)
            .collect()
    }

    async fn upsert_workflow_run_worker_assignment(
        &self,
        assignment: PersistedWorkflowRunWorkerAssignment,
    ) -> Result<(), SessionStoreError> {
        let process_id = assignment.process_id.map(i64::from);
        self.client
            .lock()
            .await
            .execute(
                r#"
                INSERT INTO control_workflow_run_workers (
                    run_id,
                    session_id,
                    automation_task_id,
                    status,
                    process_id,
                    container_name,
                    created_at,
                    updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, NOW(), NOW())
                ON CONFLICT (run_id)
                DO UPDATE SET
                    session_id = EXCLUDED.session_id,
                    automation_task_id = EXCLUDED.automation_task_id,
                    status = EXCLUDED.status,
                    process_id = EXCLUDED.process_id,
                    container_name = EXCLUDED.container_name,
                    updated_at = NOW()
                "#,
                &[
                    &assignment.run_id,
                    &assignment.session_id,
                    &assignment.automation_task_id,
                    &assignment.status.as_str(),
                    &process_id,
                    &assignment.container_name,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to upsert workflow run worker assignment: {error}"
                ))
            })?;
        Ok(())
    }

    async fn clear_workflow_run_worker_assignment(
        &self,
        run_id: Uuid,
    ) -> Result<(), SessionStoreError> {
        self.client
            .lock()
            .await
            .execute(
                "DELETE FROM control_workflow_run_workers WHERE run_id = $1",
                &[&run_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to clear workflow run worker assignment: {error}"
                ))
            })?;
        Ok(())
    }

    async fn get_workflow_run_worker_assignment(
        &self,
        run_id: Uuid,
    ) -> Result<Option<PersistedWorkflowRunWorkerAssignment>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                SELECT
                    run_id,
                    session_id,
                    automation_task_id,
                    status,
                    process_id,
                    container_name
                FROM control_workflow_run_workers
                WHERE run_id = $1
                "#,
                &[&run_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to load workflow run worker assignment: {error}"
                ))
            })?;
        row.as_ref()
            .map(row_to_workflow_run_worker_assignment)
            .transpose()
    }

    async fn list_workflow_run_worker_assignments(
        &self,
    ) -> Result<Vec<PersistedWorkflowRunWorkerAssignment>, SessionStoreError> {
        let rows = self
            .client
            .lock()
            .await
            .query(
                r#"
                SELECT
                    run_id,
                    session_id,
                    automation_task_id,
                    status,
                    process_id,
                    container_name
                FROM control_workflow_run_workers
                ORDER BY updated_at DESC, created_at DESC
                "#,
                &[],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to list workflow run worker assignments: {error}"
                ))
            })?;

        rows.iter()
            .map(row_to_workflow_run_worker_assignment)
            .collect()
    }

    async fn list_runtime_assignments(
        &self,
        runtime_binding: &str,
    ) -> Result<Vec<PersistedSessionRuntimeAssignment>, SessionStoreError> {
        let rows = self
            .client
            .lock()
            .await
            .query(
                r#"
                SELECT
                    session_id,
                    runtime_binding,
                    status,
                    agent_socket_path,
                    container_name,
                    cdp_endpoint
                FROM control_session_runtimes
                WHERE runtime_binding = $1
                ORDER BY updated_at DESC, created_at DESC
                "#,
                &[&runtime_binding],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to list runtime assignments: {error}"))
            })?;

        rows.iter().map(row_to_runtime_assignment).collect()
    }

    async fn mark_session_ready_after_runtime_loss(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                UPDATE control_sessions
                SET
                    state = 'ready',
                    updated_at = NOW()
                WHERE id = $1
                  AND state IN ('pending', 'starting', 'ready', 'active', 'idle')
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    automation_owner_client_id,
                    automation_owner_issuer,
                    automation_owner_display_name,
                    state,
                    template_id,
                    owner_mode,
                    viewport_width,
                    viewport_height,
                    idle_timeout_sec,
                    labels,
                    integration_context,
                    extensions,
                    recording,
                    created_at,
                    updated_at,
                    stopped_at
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to restore session to ready after runtime loss: {error}"
                ))
            })?;
        row.as_ref().map(row_to_stored_session).transpose()
    }
}

fn session_visible_to_principal(
    session: &StoredSession,
    principal: &AuthenticatedPrincipal,
) -> bool {
    if session.owner.subject == principal.subject && session.owner.issuer == principal.issuer {
        return true;
    }

    let Some(delegate) = &session.automation_delegate else {
        return false;
    };

    principal.client_id.as_deref() == Some(delegate.client_id.as_str())
        && principal.issuer == delegate.issuer
}

fn task_visible_to_principal(session: &StoredSession, principal: &AuthenticatedPrincipal) -> bool {
    session.owner.subject == principal.subject && session.owner.issuer == principal.issuer
}

#[cfg(test)]
mod tests {
    use super::*;

    fn principal(subject: &str) -> AuthenticatedPrincipal {
        AuthenticatedPrincipal {
            subject: subject.to_string(),
            issuer: "https://issuer.example".to_string(),
            display_name: Some(subject.to_string()),
            client_id: None,
        }
    }

    fn service_principal(subject: &str, client_id: &str) -> AuthenticatedPrincipal {
        AuthenticatedPrincipal {
            subject: subject.to_string(),
            issuer: "https://issuer.example".to_string(),
            display_name: Some(client_id.to_string()),
            client_id: Some(client_id.to_string()),
        }
    }

    #[tokio::test]
    async fn in_memory_store_scopes_sessions_to_owner() {
        let store = SessionStore::in_memory();
        let alpha = principal("alpha");
        let bravo = principal("bravo");

        let created = store
            .create_session(
                &alpha,
                CreateSessionRequest {
                    template_id: Some("default".to_string()),
                    owner_mode: None,
                    viewport: Some(SessionViewport {
                        width: 1920,
                        height: 1080,
                    }),
                    idle_timeout_sec: Some(600),
                    labels: HashMap::from([("suite".to_string(), "smoke".to_string())]),
                    integration_context: Some(serde_json::json!({ "ticket": "BPANE-6" })),
                    extension_ids: Vec::new(),
                    extensions: Vec::new(),
                    recording: SessionRecordingPolicy {
                        mode: SessionRecordingMode::Manual,
                        format: SessionRecordingFormat::Webm,
                        retention_sec: Some(86_400),
                    },
                },
                SessionOwnerMode::Collaborative,
            )
            .await
            .unwrap();

        let alpha_sessions = store.list_sessions_for_owner(&alpha).await.unwrap();
        assert_eq!(alpha_sessions.len(), 1);
        assert_eq!(alpha_sessions[0].id, created.id);
        assert_eq!(alpha_sessions[0].recording, created.recording);
        assert_eq!(created.recording.mode, SessionRecordingMode::Manual);
        assert_eq!(created.recording.format, SessionRecordingFormat::Webm);
        assert_eq!(created.recording.retention_sec, Some(86_400));

        let bravo_sessions = store.list_sessions_for_owner(&bravo).await.unwrap();
        assert!(bravo_sessions.is_empty());
        assert!(store
            .get_session_for_owner(&bravo, created.id)
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn in_memory_store_limits_legacy_runtime_to_one_active_session() {
        let store = SessionStore::in_memory();
        let alpha = principal("alpha");

        store
            .create_session(
                &alpha,
                CreateSessionRequest {
                    template_id: None,
                    owner_mode: None,
                    viewport: None,
                    idle_timeout_sec: None,
                    labels: HashMap::new(),
                    integration_context: None,
                    extension_ids: Vec::new(),
                    extensions: Vec::new(),
                    recording: SessionRecordingPolicy::default(),
                },
                SessionOwnerMode::Collaborative,
            )
            .await
            .unwrap();

        let error = store
            .create_session(
                &alpha,
                CreateSessionRequest {
                    template_id: None,
                    owner_mode: None,
                    viewport: None,
                    idle_timeout_sec: None,
                    labels: HashMap::new(),
                    integration_context: None,
                    extension_ids: Vec::new(),
                    extensions: Vec::new(),
                    recording: SessionRecordingPolicy::default(),
                },
                SessionOwnerMode::Collaborative,
            )
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            SessionStoreError::ActiveSessionConflict {
                max_runtime_sessions: 1
            }
        ));
    }

    #[tokio::test]
    async fn in_memory_store_respects_runtime_pool_capacity() {
        let store = SessionStore::in_memory_with_config(SessionManagerProfile {
            runtime_binding: "docker_runtime_pool".to_string(),
            compatibility_mode: "session_runtime_pool".to_string(),
            max_runtime_sessions: 2,
            supports_legacy_global_routes: false,
            supports_session_extensions: true,
        });
        let alpha = principal("alpha");

        for _ in 0..2 {
            store
                .create_session(
                    &alpha,
                    CreateSessionRequest {
                        template_id: None,
                        owner_mode: None,
                        viewport: None,
                        idle_timeout_sec: None,
                        labels: HashMap::new(),
                        integration_context: None,
                        extension_ids: Vec::new(),
                        extensions: Vec::new(),
                        recording: SessionRecordingPolicy::default(),
                    },
                    SessionOwnerMode::Collaborative,
                )
                .await
                .unwrap();
        }

        let error = store
            .create_session(
                &alpha,
                CreateSessionRequest {
                    template_id: None,
                    owner_mode: None,
                    viewport: None,
                    idle_timeout_sec: None,
                    labels: HashMap::new(),
                    integration_context: None,
                    extension_ids: Vec::new(),
                    extensions: Vec::new(),
                    recording: SessionRecordingPolicy::default(),
                },
                SessionOwnerMode::Collaborative,
            )
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            SessionStoreError::ActiveSessionConflict {
                max_runtime_sessions: 2
            }
        ));
    }

    #[tokio::test]
    async fn in_memory_store_allows_delegated_client_to_load_session() {
        let store = SessionStore::in_memory();
        let owner = principal("owner");
        let delegate = service_principal("service-account-id", "bpane-mcp-bridge");

        let created = store
            .create_session(
                &owner,
                CreateSessionRequest {
                    template_id: None,
                    owner_mode: None,
                    viewport: None,
                    idle_timeout_sec: None,
                    labels: HashMap::new(),
                    integration_context: None,
                    extension_ids: Vec::new(),
                    extensions: Vec::new(),
                    recording: SessionRecordingPolicy::default(),
                },
                SessionOwnerMode::Collaborative,
            )
            .await
            .unwrap();

        let updated = store
            .set_automation_delegate_for_owner(
                &owner,
                created.id,
                SetAutomationDelegateRequest {
                    client_id: "bpane-mcp-bridge".to_string(),
                    issuer: None,
                    display_name: Some("BrowserPane MCP bridge".to_string()),
                },
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            updated.automation_delegate.as_ref().unwrap().client_id,
            "bpane-mcp-bridge"
        );

        let visible = store
            .get_session_for_principal(&delegate, created.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(visible.id, created.id);
    }

    #[tokio::test]
    async fn in_memory_store_tracks_automation_task_lifecycle_logs_and_events() {
        let store = SessionStore::in_memory();
        let owner = principal("owner");
        let session = store
            .create_session(
                &owner,
                CreateSessionRequest {
                    template_id: None,
                    owner_mode: None,
                    viewport: None,
                    idle_timeout_sec: None,
                    labels: HashMap::new(),
                    integration_context: None,
                    extension_ids: Vec::new(),
                    extensions: Vec::new(),
                    recording: SessionRecordingPolicy::default(),
                },
                SessionOwnerMode::Collaborative,
            )
            .await
            .unwrap();

        let task = store
            .create_automation_task(
                &owner,
                PersistAutomationTaskRequest {
                    display_name: Some("demo task".to_string()),
                    executor: "playwright".to_string(),
                    session_id: session.id,
                    session_source: AutomationTaskSessionSource::ExistingSession,
                    input: Some(serde_json::json!({ "step": "login" })),
                    labels: HashMap::from([("suite".to_string(), "workflow".to_string())]),
                },
            )
            .await
            .unwrap();
        assert_eq!(task.state, AutomationTaskState::Pending);

        let running = store
            .transition_automation_task(
                task.id,
                AutomationTaskTransitionRequest {
                    state: AutomationTaskState::Running,
                    output: None,
                    error: None,
                    artifact_refs: Vec::new(),
                    event_type: "automation_task.running".to_string(),
                    event_message: "task entered running state".to_string(),
                    event_data: None,
                },
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(running.state, AutomationTaskState::Running);
        assert!(running.started_at.is_some());

        let log = store
            .append_automation_task_log(
                task.id,
                AutomationTaskLogStream::Stdout,
                "step 1 complete".to_string(),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(log.stream, AutomationTaskLogStream::Stdout);

        let succeeded = store
            .transition_automation_task(
                task.id,
                AutomationTaskTransitionRequest {
                    state: AutomationTaskState::Succeeded,
                    output: Some(serde_json::json!({ "result": "ok" })),
                    error: None,
                    artifact_refs: vec!["artifact://trace.zip".to_string()],
                    event_type: "automation_task.succeeded".to_string(),
                    event_message: "task completed successfully".to_string(),
                    event_data: Some(serde_json::json!({ "duration_ms": 1200 })),
                },
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(succeeded.state, AutomationTaskState::Succeeded);
        assert!(succeeded.completed_at.is_some());
        assert_eq!(succeeded.artifact_refs.len(), 1);

        let events = store
            .list_automation_task_events_for_owner(&owner, task.id)
            .await
            .unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].event_type, "automation_task.created");
        assert_eq!(events[2].event_type, "automation_task.succeeded");

        let logs = store
            .list_automation_task_logs_for_owner(&owner, task.id)
            .await
            .unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].message, "step 1 complete");

        let error = store
            .transition_automation_task(
                task.id,
                AutomationTaskTransitionRequest {
                    state: AutomationTaskState::Running,
                    output: None,
                    error: None,
                    artifact_refs: Vec::new(),
                    event_type: "automation_task.running".to_string(),
                    event_message: "task should not resume".to_string(),
                    event_data: None,
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(error, SessionStoreError::Conflict(_)));
    }

    #[tokio::test]
    async fn in_memory_store_stops_unused_ready_sessions_and_idle_sessions() {
        let store = SessionStore::in_memory();
        let owner = principal("owner");
        let created = store
            .create_session(
                &owner,
                CreateSessionRequest {
                    template_id: None,
                    owner_mode: None,
                    viewport: None,
                    idle_timeout_sec: Some(300),
                    labels: HashMap::new(),
                    integration_context: None,
                    extension_ids: Vec::new(),
                    extensions: Vec::new(),
                    recording: SessionRecordingPolicy::default(),
                },
                SessionOwnerMode::Collaborative,
            )
            .await
            .unwrap();

        let stopped_ready = store
            .stop_session_if_idle(created.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stopped_ready.state, SessionLifecycleState::Stopped);

        let created = store
            .create_session(
                &owner,
                CreateSessionRequest {
                    template_id: None,
                    owner_mode: None,
                    viewport: None,
                    idle_timeout_sec: None,
                    labels: HashMap::new(),
                    integration_context: None,
                    extension_ids: Vec::new(),
                    extensions: Vec::new(),
                    recording: SessionRecordingPolicy::default(),
                },
                SessionOwnerMode::Collaborative,
            )
            .await
            .unwrap();

        let active = store
            .mark_session_active(created.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(active.state, SessionLifecycleState::Active);

        let idle = store.mark_session_idle(created.id).await.unwrap().unwrap();
        assert_eq!(idle.state, SessionLifecycleState::Idle);

        let stopped = store
            .stop_session_if_idle(created.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stopped.state, SessionLifecycleState::Stopped);

        let after = store
            .mark_session_active(created.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(after.state, SessionLifecycleState::Stopped);
    }

    #[tokio::test]
    async fn in_memory_store_can_prepare_a_stopped_session_for_reconnect() {
        let store = SessionStore::in_memory();
        let owner = principal("owner");
        let created = store
            .create_session(
                &owner,
                CreateSessionRequest {
                    template_id: None,
                    owner_mode: None,
                    viewport: None,
                    idle_timeout_sec: Some(300),
                    labels: HashMap::new(),
                    integration_context: None,
                    extension_ids: Vec::new(),
                    extensions: Vec::new(),
                    recording: SessionRecordingPolicy::default(),
                },
                SessionOwnerMode::Collaborative,
            )
            .await
            .unwrap();

        let stopped = store
            .stop_session_if_idle(created.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stopped.state, SessionLifecycleState::Stopped);

        let resumed = store
            .prepare_session_for_connect(created.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(resumed.state, SessionLifecycleState::Ready);
        assert!(resumed.stopped_at.is_none());
    }

    #[tokio::test]
    async fn reconnect_prep_respects_runtime_pool_capacity() {
        let store = SessionStore::in_memory_with_config(SessionManagerProfile {
            runtime_binding: "docker_runtime_pool".to_string(),
            compatibility_mode: "session_runtime_pool".to_string(),
            max_runtime_sessions: 1,
            supports_legacy_global_routes: false,
            supports_session_extensions: true,
        });
        let owner = principal("owner");

        let ready = store
            .create_session(
                &owner,
                CreateSessionRequest {
                    template_id: None,
                    owner_mode: None,
                    viewport: None,
                    idle_timeout_sec: None,
                    labels: HashMap::new(),
                    integration_context: None,
                    extension_ids: Vec::new(),
                    extensions: Vec::new(),
                    recording: SessionRecordingPolicy::default(),
                },
                SessionOwnerMode::Collaborative,
            )
            .await
            .unwrap();
        assert_eq!(ready.state, SessionLifecycleState::Ready);

        let stopped = store
            .stop_session_for_owner(&owner, ready.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stopped.state, SessionLifecycleState::Stopped);

        let replacement = store
            .create_session(
                &owner,
                CreateSessionRequest {
                    template_id: None,
                    owner_mode: None,
                    viewport: None,
                    idle_timeout_sec: None,
                    labels: HashMap::new(),
                    integration_context: None,
                    extension_ids: Vec::new(),
                    extensions: Vec::new(),
                    recording: SessionRecordingPolicy::default(),
                },
                SessionOwnerMode::Collaborative,
            )
            .await
            .unwrap();
        assert_eq!(replacement.state, SessionLifecycleState::Ready);

        let error = store
            .prepare_session_for_connect(stopped.id)
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            SessionStoreError::ActiveSessionConflict {
                max_runtime_sessions: 1
            }
        ));
    }

    #[tokio::test]
    async fn in_memory_store_persists_runtime_assignments_and_can_clear_them() {
        let store = SessionStore::in_memory_with_config(SessionManagerProfile {
            runtime_binding: "docker_runtime_pool".to_string(),
            compatibility_mode: "session_runtime_pool".to_string(),
            max_runtime_sessions: 2,
            supports_legacy_global_routes: false,
            supports_session_extensions: true,
        });
        let owner = principal("owner");
        let session = store
            .create_session(
                &owner,
                CreateSessionRequest {
                    template_id: None,
                    owner_mode: None,
                    viewport: None,
                    idle_timeout_sec: None,
                    labels: HashMap::new(),
                    integration_context: None,
                    extension_ids: Vec::new(),
                    extensions: Vec::new(),
                    recording: SessionRecordingPolicy::default(),
                },
                SessionOwnerMode::Collaborative,
            )
            .await
            .unwrap();

        store
            .upsert_runtime_assignment(PersistedSessionRuntimeAssignment {
                session_id: session.id,
                runtime_binding: "docker_runtime_pool".to_string(),
                status: SessionRuntimeAssignmentStatus::Ready,
                agent_socket_path: format!("/run/bpane/sessions/{}.sock", session.id),
                container_name: Some(format!("bpane-runtime-{}", session.id.as_simple())),
                cdp_endpoint: Some(format!(
                    "http://bpane-runtime-{}:9223",
                    session.id.as_simple()
                )),
            })
            .await
            .unwrap();

        let assignments = store
            .list_runtime_assignments("docker_runtime_pool")
            .await
            .unwrap();
        assert_eq!(assignments.len(), 1);
        assert_eq!(assignments[0].session_id, session.id);
        assert_eq!(assignments[0].status, SessionRuntimeAssignmentStatus::Ready);

        store.clear_runtime_assignment(session.id).await.unwrap();
        assert!(store
            .list_runtime_assignments("docker_runtime_pool")
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn in_memory_store_persists_recording_worker_assignments() {
        let store = SessionStore::in_memory();
        let owner = principal("owner");
        let session = store
            .create_session(
                &owner,
                CreateSessionRequest {
                    template_id: None,
                    owner_mode: None,
                    viewport: None,
                    idle_timeout_sec: None,
                    labels: HashMap::new(),
                    integration_context: None,
                    extension_ids: Vec::new(),
                    extensions: Vec::new(),
                    recording: SessionRecordingPolicy {
                        mode: SessionRecordingMode::Always,
                        format: SessionRecordingFormat::Webm,
                        retention_sec: None,
                    },
                },
                SessionOwnerMode::Collaborative,
            )
            .await
            .unwrap();
        let recording = store
            .create_recording_for_session(session.id, SessionRecordingFormat::Webm, None)
            .await
            .unwrap();

        store
            .upsert_recording_worker_assignment(PersistedSessionRecordingWorkerAssignment {
                session_id: session.id,
                recording_id: recording.id,
                status: SessionRecordingWorkerAssignmentStatus::Running,
                process_id: Some(4242),
            })
            .await
            .unwrap();

        let loaded = store
            .get_recording_worker_assignment(session.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(loaded.recording_id, recording.id);
        assert_eq!(
            loaded.status,
            SessionRecordingWorkerAssignmentStatus::Running
        );
        assert_eq!(loaded.process_id, Some(4242));

        let listed = store.list_recording_worker_assignments().await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].session_id, session.id);

        store
            .clear_recording_worker_assignment(session.id)
            .await
            .unwrap();
        assert!(store
            .list_recording_worker_assignments()
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn in_memory_store_persists_workflow_run_worker_assignments() {
        let store = SessionStore::in_memory();
        let owner = principal("owner");
        let session = store
            .create_session(
                &owner,
                CreateSessionRequest {
                    template_id: None,
                    owner_mode: None,
                    viewport: None,
                    idle_timeout_sec: None,
                    labels: HashMap::new(),
                    integration_context: None,
                    extension_ids: Vec::new(),
                    extensions: Vec::new(),
                    recording: SessionRecordingPolicy::default(),
                },
                SessionOwnerMode::Collaborative,
            )
            .await
            .unwrap();
        let task = store
            .create_automation_task(
                &owner,
                PersistAutomationTaskRequest {
                    display_name: Some("Workflow Task".to_string()),
                    executor: "playwright".to_string(),
                    session_id: session.id,
                    session_source: AutomationTaskSessionSource::CreatedSession,
                    input: None,
                    labels: HashMap::new(),
                },
            )
            .await
            .unwrap();
        let workflow = store
            .create_workflow_definition(
                &owner,
                PersistWorkflowDefinitionRequest {
                    name: "Smoke Workflow".to_string(),
                    description: None,
                    labels: HashMap::new(),
                },
            )
            .await
            .unwrap();
        let version = store
            .create_workflow_definition_version(
                &owner,
                PersistWorkflowDefinitionVersionRequest {
                    workflow_definition_id: workflow.id,
                    version: "v1".to_string(),
                    executor: "playwright".to_string(),
                    entrypoint: "workflows/smoke/run.mjs".to_string(),
                    source: None,
                    input_schema: None,
                    output_schema: None,
                    default_session: None,
                    allowed_credential_binding_ids: Vec::new(),
                    allowed_extension_ids: Vec::new(),
                    allowed_file_workspace_ids: Vec::new(),
                },
            )
            .await
            .unwrap();
        let run = store
            .create_workflow_run(
                &owner,
                PersistWorkflowRunRequest {
                    workflow_definition_id: workflow.id,
                    workflow_definition_version_id: version.id,
                    workflow_version: version.version.clone(),
                    session_id: session.id,
                    automation_task_id: task.id,
                    source_system: None,
                    source_reference: None,
                    client_request_id: None,
                    create_request_fingerprint: None,
                    source_snapshot: None,
                    extensions: Vec::new(),
                    credential_bindings: Vec::new(),
                    workspace_inputs: Vec::new(),
                    input: None,
                    labels: HashMap::new(),
                },
            )
            .await
            .unwrap()
            .run;

        store
            .upsert_workflow_run_worker_assignment(PersistedWorkflowRunWorkerAssignment {
                run_id: run.id,
                session_id: session.id,
                automation_task_id: task.id,
                status: WorkflowRunWorkerAssignmentStatus::Running,
                process_id: Some(5151),
                container_name: Some("bpane-workflow-test".to_string()),
            })
            .await
            .unwrap();

        let loaded = store
            .get_workflow_run_worker_assignment(run.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(loaded.session_id, session.id);
        assert_eq!(loaded.automation_task_id, task.id);
        assert_eq!(loaded.status, WorkflowRunWorkerAssignmentStatus::Running);
        assert_eq!(loaded.process_id, Some(5151));
        assert_eq!(
            loaded.container_name.as_deref(),
            Some("bpane-workflow-test")
        );

        let listed = store.list_workflow_run_worker_assignments().await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].run_id, run.id);

        store
            .clear_workflow_run_worker_assignment(run.id)
            .await
            .unwrap();
        assert!(store
            .list_workflow_run_worker_assignments()
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn in_memory_store_deduplicates_workflow_runs_by_client_request_id() {
        let store = SessionStore::in_memory();
        let owner = principal("owner");
        let workflow = store
            .create_workflow_definition(
                &owner,
                PersistWorkflowDefinitionRequest {
                    name: "Workflow".to_string(),
                    description: None,
                    labels: HashMap::new(),
                },
            )
            .await
            .unwrap();
        let version = store
            .create_workflow_definition_version(
                &owner,
                PersistWorkflowDefinitionVersionRequest {
                    workflow_definition_id: workflow.id,
                    version: "v1".to_string(),
                    executor: "playwright".to_string(),
                    entrypoint: "workflows/run.mjs".to_string(),
                    source: None,
                    input_schema: None,
                    output_schema: None,
                    default_session: None,
                    allowed_credential_binding_ids: Vec::new(),
                    allowed_extension_ids: Vec::new(),
                    allowed_file_workspace_ids: Vec::new(),
                },
            )
            .await
            .unwrap();

        let session_one = store
            .create_session(
                &owner,
                CreateSessionRequest {
                    template_id: None,
                    owner_mode: None,
                    viewport: None,
                    idle_timeout_sec: None,
                    labels: HashMap::new(),
                    integration_context: None,
                    extension_ids: Vec::new(),
                    extensions: Vec::new(),
                    recording: SessionRecordingPolicy::default(),
                },
                SessionOwnerMode::Collaborative,
            )
            .await
            .unwrap();
        let task_one = store
            .create_automation_task(
                &owner,
                PersistAutomationTaskRequest {
                    display_name: Some("Workflow Task".to_string()),
                    executor: "playwright".to_string(),
                    session_id: session_one.id,
                    session_source: AutomationTaskSessionSource::CreatedSession,
                    input: Some(serde_json::json!({ "customer_id": "cust-42" })),
                    labels: HashMap::new(),
                },
            )
            .await
            .unwrap();

        let first = store
            .create_workflow_run(
                &owner,
                PersistWorkflowRunRequest {
                    workflow_definition_id: workflow.id,
                    workflow_definition_version_id: version.id,
                    workflow_version: version.version.clone(),
                    session_id: session_one.id,
                    automation_task_id: task_one.id,
                    source_system: Some("camunda-prod".to_string()),
                    source_reference: Some("task-1".to_string()),
                    client_request_id: Some("job-123-attempt-1".to_string()),
                    create_request_fingerprint: Some("fingerprint-a".to_string()),
                    source_snapshot: None,
                    extensions: Vec::new(),
                    credential_bindings: Vec::new(),
                    workspace_inputs: Vec::new(),
                    input: Some(serde_json::json!({ "customer_id": "cust-42" })),
                    labels: HashMap::new(),
                },
            )
            .await
            .unwrap();
        assert!(first.created);

        let second = store
            .create_workflow_run(
                &owner,
                PersistWorkflowRunRequest {
                    workflow_definition_id: workflow.id,
                    workflow_definition_version_id: version.id,
                    workflow_version: version.version.clone(),
                    session_id: session_one.id,
                    automation_task_id: task_one.id,
                    source_system: Some("camunda-prod".to_string()),
                    source_reference: Some("task-1".to_string()),
                    client_request_id: Some("job-123-attempt-1".to_string()),
                    create_request_fingerprint: Some("fingerprint-a".to_string()),
                    source_snapshot: None,
                    extensions: Vec::new(),
                    credential_bindings: Vec::new(),
                    workspace_inputs: Vec::new(),
                    input: Some(serde_json::json!({ "customer_id": "cust-42" })),
                    labels: HashMap::new(),
                },
            )
            .await
            .unwrap();

        assert!(!second.created);
        assert_eq!(second.run.id, first.run.id);
        assert_eq!(second.run.session_id, first.run.session_id);
        assert_eq!(second.run.automation_task_id, first.run.automation_task_id);
        assert_eq!(
            store
                .find_workflow_run_by_client_request_id_for_owner(&owner, "job-123-attempt-1")
                .await
                .unwrap()
                .unwrap()
                .id,
            first.run.id
        );
    }

    #[tokio::test]
    async fn in_memory_store_reconciles_workflow_run_from_terminal_task_state() {
        let store = SessionStore::in_memory();
        let owner = principal("owner");
        let session = store
            .create_session(
                &owner,
                CreateSessionRequest {
                    template_id: None,
                    owner_mode: None,
                    viewport: None,
                    idle_timeout_sec: None,
                    labels: HashMap::new(),
                    integration_context: None,
                    extension_ids: Vec::new(),
                    extensions: Vec::new(),
                    recording: SessionRecordingPolicy::default(),
                },
                SessionOwnerMode::Collaborative,
            )
            .await
            .unwrap();
        let task = store
            .create_automation_task(
                &owner,
                PersistAutomationTaskRequest {
                    display_name: Some("Workflow Task".to_string()),
                    executor: "playwright".to_string(),
                    session_id: session.id,
                    session_source: AutomationTaskSessionSource::CreatedSession,
                    input: None,
                    labels: HashMap::new(),
                },
            )
            .await
            .unwrap();
        let workflow = store
            .create_workflow_definition(
                &owner,
                PersistWorkflowDefinitionRequest {
                    name: "Workflow".to_string(),
                    description: None,
                    labels: HashMap::new(),
                },
            )
            .await
            .unwrap();
        let version = store
            .create_workflow_definition_version(
                &owner,
                PersistWorkflowDefinitionVersionRequest {
                    workflow_definition_id: workflow.id,
                    version: "v1".to_string(),
                    executor: "playwright".to_string(),
                    entrypoint: "workflows/run.mjs".to_string(),
                    source: None,
                    input_schema: None,
                    output_schema: None,
                    default_session: None,
                    allowed_credential_binding_ids: Vec::new(),
                    allowed_extension_ids: Vec::new(),
                    allowed_file_workspace_ids: Vec::new(),
                },
            )
            .await
            .unwrap();
        let run = store
            .create_workflow_run(
                &owner,
                PersistWorkflowRunRequest {
                    workflow_definition_id: workflow.id,
                    workflow_definition_version_id: version.id,
                    workflow_version: version.version.clone(),
                    session_id: session.id,
                    automation_task_id: task.id,
                    source_system: None,
                    source_reference: None,
                    client_request_id: None,
                    create_request_fingerprint: None,
                    source_snapshot: None,
                    extensions: Vec::new(),
                    credential_bindings: Vec::new(),
                    workspace_inputs: Vec::new(),
                    input: None,
                    labels: HashMap::new(),
                },
            )
            .await
            .unwrap()
            .run;

        store
            .cancel_automation_task_for_owner(&owner, task.id)
            .await
            .unwrap()
            .unwrap();

        let reconciled = store
            .reconcile_workflow_run_from_task(run.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(reconciled.state, WorkflowRunState::Cancelled);
        assert!(reconciled.completed_at.is_some());
    }

    #[tokio::test]
    async fn in_memory_store_cancels_queued_automation_task_and_workflow_run() {
        let store = SessionStore::in_memory();
        let owner = principal("owner");
        let session = store
            .create_session(
                &owner,
                CreateSessionRequest {
                    template_id: None,
                    owner_mode: None,
                    viewport: None,
                    idle_timeout_sec: None,
                    labels: HashMap::new(),
                    integration_context: None,
                    extension_ids: Vec::new(),
                    extensions: Vec::new(),
                    recording: SessionRecordingPolicy::default(),
                },
                SessionOwnerMode::Collaborative,
            )
            .await
            .unwrap();
        let task = store
            .create_automation_task(
                &owner,
                PersistAutomationTaskRequest {
                    display_name: Some("Queued Task".to_string()),
                    executor: "playwright".to_string(),
                    session_id: session.id,
                    session_source: AutomationTaskSessionSource::CreatedSession,
                    input: None,
                    labels: HashMap::new(),
                },
            )
            .await
            .unwrap();
        let workflow = store
            .create_workflow_definition(
                &owner,
                PersistWorkflowDefinitionRequest {
                    name: "Queued Workflow".to_string(),
                    description: None,
                    labels: HashMap::new(),
                },
            )
            .await
            .unwrap();
        let version = store
            .create_workflow_definition_version(
                &owner,
                PersistWorkflowDefinitionVersionRequest {
                    workflow_definition_id: workflow.id,
                    version: "v1".to_string(),
                    executor: "playwright".to_string(),
                    entrypoint: "workflows/run.mjs".to_string(),
                    source: None,
                    input_schema: None,
                    output_schema: None,
                    default_session: None,
                    allowed_credential_binding_ids: Vec::new(),
                    allowed_extension_ids: Vec::new(),
                    allowed_file_workspace_ids: Vec::new(),
                },
            )
            .await
            .unwrap();
        let run = store
            .create_workflow_run(
                &owner,
                PersistWorkflowRunRequest {
                    workflow_definition_id: workflow.id,
                    workflow_definition_version_id: version.id,
                    workflow_version: version.version.clone(),
                    session_id: session.id,
                    automation_task_id: task.id,
                    source_system: None,
                    source_reference: None,
                    client_request_id: None,
                    create_request_fingerprint: None,
                    source_snapshot: None,
                    extensions: Vec::new(),
                    credential_bindings: Vec::new(),
                    workspace_inputs: Vec::new(),
                    input: None,
                    labels: HashMap::new(),
                },
            )
            .await
            .unwrap()
            .run;

        let queued = store
            .transition_automation_task(
                task.id,
                AutomationTaskTransitionRequest {
                    state: AutomationTaskState::Queued,
                    output: None,
                    error: None,
                    artifact_refs: Vec::new(),
                    event_type: "automation_task.queued".to_string(),
                    event_message: "task queued while waiting for worker capacity".to_string(),
                    event_data: None,
                },
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(queued.state, AutomationTaskState::Queued);

        let cancelled = store
            .cancel_automation_task_for_owner(&owner, task.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(cancelled.state, AutomationTaskState::Cancelled);
        assert!(cancelled.cancel_requested_at.is_some());
        assert!(cancelled.completed_at.is_some());

        let current_run = store.get_workflow_run_by_id(run.id).await.unwrap().unwrap();
        assert_eq!(current_run.state, WorkflowRunState::Cancelled);
        assert!(current_run.completed_at.is_some());

        let events = store
            .list_workflow_run_events_for_owner(&owner, run.id)
            .await
            .unwrap();
        assert!(events
            .iter()
            .any(|event| event.event_type == "workflow_run.cancelled"));
    }

    #[tokio::test]
    async fn in_memory_store_rejects_conflicting_workflow_run_client_request_id() {
        let store = SessionStore::in_memory();
        let owner = principal("owner");
        let session = store
            .create_session(
                &owner,
                CreateSessionRequest {
                    template_id: None,
                    owner_mode: None,
                    viewport: None,
                    idle_timeout_sec: None,
                    labels: HashMap::new(),
                    integration_context: None,
                    extension_ids: Vec::new(),
                    extensions: Vec::new(),
                    recording: SessionRecordingPolicy::default(),
                },
                SessionOwnerMode::Collaborative,
            )
            .await
            .unwrap();
        let task = store
            .create_automation_task(
                &owner,
                PersistAutomationTaskRequest {
                    display_name: Some("Workflow Task".to_string()),
                    executor: "playwright".to_string(),
                    session_id: session.id,
                    session_source: AutomationTaskSessionSource::CreatedSession,
                    input: None,
                    labels: HashMap::new(),
                },
            )
            .await
            .unwrap();
        let workflow = store
            .create_workflow_definition(
                &owner,
                PersistWorkflowDefinitionRequest {
                    name: "Workflow".to_string(),
                    description: None,
                    labels: HashMap::new(),
                },
            )
            .await
            .unwrap();
        let version = store
            .create_workflow_definition_version(
                &owner,
                PersistWorkflowDefinitionVersionRequest {
                    workflow_definition_id: workflow.id,
                    version: "v1".to_string(),
                    executor: "playwright".to_string(),
                    entrypoint: "workflows/run.mjs".to_string(),
                    source: None,
                    input_schema: None,
                    output_schema: None,
                    default_session: None,
                    allowed_credential_binding_ids: Vec::new(),
                    allowed_extension_ids: Vec::new(),
                    allowed_file_workspace_ids: Vec::new(),
                },
            )
            .await
            .unwrap();

        let created = store
            .create_workflow_run(
                &owner,
                PersistWorkflowRunRequest {
                    workflow_definition_id: workflow.id,
                    workflow_definition_version_id: version.id,
                    workflow_version: version.version.clone(),
                    session_id: session.id,
                    automation_task_id: task.id,
                    source_system: Some("camunda-prod".to_string()),
                    source_reference: Some("task-1".to_string()),
                    client_request_id: Some("job-123-attempt-1".to_string()),
                    create_request_fingerprint: Some("fingerprint-a".to_string()),
                    source_snapshot: None,
                    extensions: Vec::new(),
                    credential_bindings: Vec::new(),
                    workspace_inputs: Vec::new(),
                    input: None,
                    labels: HashMap::new(),
                },
            )
            .await
            .unwrap();
        assert!(created.created);

        let error = store
            .create_workflow_run(
                &owner,
                PersistWorkflowRunRequest {
                    workflow_definition_id: workflow.id,
                    workflow_definition_version_id: version.id,
                    workflow_version: version.version.clone(),
                    session_id: session.id,
                    automation_task_id: task.id,
                    source_system: Some("camunda-prod".to_string()),
                    source_reference: Some("task-2".to_string()),
                    client_request_id: Some("job-123-attempt-1".to_string()),
                    create_request_fingerprint: Some("fingerprint-b".to_string()),
                    source_snapshot: None,
                    extensions: Vec::new(),
                    credential_bindings: Vec::new(),
                    workspace_inputs: Vec::new(),
                    input: Some(serde_json::json!({ "customer_id": "cust-77" })),
                    labels: HashMap::new(),
                },
            )
            .await
            .unwrap_err();
        assert!(
            matches!(error, SessionStoreError::Conflict(message) if message.contains("client_request_id"))
        );
    }

    #[tokio::test]
    async fn in_memory_store_can_restore_runtime_candidate_to_ready_after_runtime_loss() {
        let store = SessionStore::in_memory();
        let owner = principal("owner");
        let session = store
            .create_session(
                &owner,
                CreateSessionRequest {
                    template_id: None,
                    owner_mode: None,
                    viewport: None,
                    idle_timeout_sec: None,
                    labels: HashMap::new(),
                    integration_context: None,
                    extension_ids: Vec::new(),
                    extensions: Vec::new(),
                    recording: SessionRecordingPolicy::default(),
                },
                SessionOwnerMode::Collaborative,
            )
            .await
            .unwrap();

        let active = store
            .mark_session_active(session.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(active.state, SessionLifecycleState::Active);

        let restored = store
            .mark_session_ready_after_runtime_loss(session.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(restored.state, SessionLifecycleState::Ready);
    }

    #[tokio::test]
    async fn in_memory_store_creates_and_stops_recording_metadata() {
        let store = SessionStore::in_memory();
        let owner = principal("owner");
        let session = store
            .create_session(
                &owner,
                CreateSessionRequest {
                    template_id: None,
                    owner_mode: None,
                    viewport: None,
                    idle_timeout_sec: None,
                    labels: HashMap::new(),
                    integration_context: None,
                    extension_ids: Vec::new(),
                    extensions: Vec::new(),
                    recording: SessionRecordingPolicy {
                        mode: SessionRecordingMode::Manual,
                        format: SessionRecordingFormat::Webm,
                        retention_sec: None,
                    },
                },
                SessionOwnerMode::Collaborative,
            )
            .await
            .unwrap();

        let created = store
            .create_recording_for_session(session.id, SessionRecordingFormat::Webm, None)
            .await
            .unwrap();
        assert_eq!(created.session_id, session.id);
        assert_eq!(created.previous_recording_id, None);
        assert_eq!(created.state, SessionRecordingState::Recording);
        assert_eq!(created.mime_type.as_deref(), Some("video/webm"));

        let listed = store.list_recordings_for_session(session.id).await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, created.id);

        let stopped = store
            .stop_recording_for_session(
                session.id,
                created.id,
                SessionRecordingTerminationReason::ManualStop,
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stopped.state, SessionRecordingState::Finalizing);
        assert_eq!(
            stopped.termination_reason,
            Some(SessionRecordingTerminationReason::ManualStop)
        );

        let latest = store
            .get_latest_recording_for_session(session.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(latest.id, created.id);
        assert_eq!(latest.state, SessionRecordingState::Finalizing);
        assert_eq!(
            latest.termination_reason,
            Some(SessionRecordingTerminationReason::ManualStop)
        );

        let completed = store
            .complete_recording_for_session(
                session.id,
                created.id,
                PersistCompletedSessionRecordingRequest {
                    artifact_ref: "local_fs:session/recording.webm".to_string(),
                    mime_type: Some("video/webm".to_string()),
                    bytes: Some(123),
                    duration_ms: Some(456),
                },
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(completed.state, SessionRecordingState::Ready);
        assert_eq!(
            completed.artifact_ref.as_deref(),
            Some("local_fs:session/recording.webm")
        );
        assert_eq!(completed.bytes, Some(123));
        assert_eq!(completed.duration_ms, Some(456));

        let failed = store
            .create_recording_for_session(
                session.id,
                SessionRecordingFormat::Webm,
                Some(created.id),
            )
            .await
            .unwrap();
        let failed = store
            .fail_recording_for_session(
                session.id,
                failed.id,
                FailSessionRecordingRequest {
                    error: "boom".to_string(),
                    termination_reason: Some(SessionRecordingTerminationReason::WorkerExit),
                },
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(failed.state, SessionRecordingState::Failed);
        assert_eq!(failed.previous_recording_id, Some(created.id));
        assert_eq!(failed.error.as_deref(), Some("boom"));
        assert_eq!(
            failed.termination_reason,
            Some(SessionRecordingTerminationReason::WorkerExit)
        );
    }

    #[tokio::test]
    async fn in_memory_store_lists_and_clears_expired_recording_artifacts() {
        let store = SessionStore::in_memory();
        let owner = principal("owner");
        let session = store
            .create_session(
                &owner,
                CreateSessionRequest {
                    template_id: None,
                    owner_mode: None,
                    viewport: None,
                    idle_timeout_sec: None,
                    labels: HashMap::new(),
                    integration_context: None,
                    extension_ids: Vec::new(),
                    extensions: Vec::new(),
                    recording: SessionRecordingPolicy {
                        mode: SessionRecordingMode::Manual,
                        format: SessionRecordingFormat::Webm,
                        retention_sec: Some(60),
                    },
                },
                SessionOwnerMode::Collaborative,
            )
            .await
            .unwrap();

        let created = store
            .create_recording_for_session(session.id, SessionRecordingFormat::Webm, None)
            .await
            .unwrap();
        let completed = store
            .complete_recording_for_session(
                session.id,
                created.id,
                PersistCompletedSessionRecordingRequest {
                    artifact_ref: "local_fs:session/recording.webm".to_string(),
                    mime_type: Some("video/webm".to_string()),
                    bytes: Some(123),
                    duration_ms: Some(456),
                },
            )
            .await
            .unwrap()
            .unwrap();

        let candidates = store
            .list_recording_artifact_retention_candidates(
                completed.completed_at.unwrap() + chrono::Duration::seconds(61),
            )
            .await
            .unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].session_id, session.id);
        assert_eq!(candidates[0].recording_id, created.id);
        assert_eq!(
            candidates[0].artifact_ref,
            "local_fs:session/recording.webm"
        );

        let cleared = store
            .clear_recording_artifact_path(session.id, created.id)
            .await
            .unwrap()
            .unwrap();
        assert!(cleared.artifact_ref.is_none());

        let candidates = store
            .list_recording_artifact_retention_candidates(
                completed.completed_at.unwrap() + chrono::Duration::seconds(61),
            )
            .await
            .unwrap();
        assert!(candidates.is_empty());
    }

    #[test]
    fn rejects_non_object_integration_context() {
        let error = validate_create_request(&CreateSessionRequest {
            template_id: None,
            owner_mode: None,
            viewport: None,
            idle_timeout_sec: None,
            labels: HashMap::new(),
            integration_context: Some(Value::String("bad".to_string())),
            extension_ids: Vec::new(),
            extensions: Vec::new(),
            recording: SessionRecordingPolicy::default(),
        })
        .unwrap_err();

        assert!(matches!(error, SessionStoreError::InvalidRequest(_)));
    }

    #[test]
    fn rejects_zero_recording_retention() {
        let error = validate_create_request(&CreateSessionRequest {
            template_id: None,
            owner_mode: None,
            viewport: None,
            idle_timeout_sec: None,
            labels: HashMap::new(),
            integration_context: None,
            extension_ids: Vec::new(),
            extensions: Vec::new(),
            recording: SessionRecordingPolicy {
                mode: SessionRecordingMode::Manual,
                format: SessionRecordingFormat::Webm,
                retention_sec: Some(0),
            },
        })
        .unwrap_err();

        assert!(matches!(error, SessionStoreError::InvalidRequest(_)));
    }

    #[test]
    fn rejects_empty_recording_failure_message() {
        let error = validate_fail_recording_request(&FailSessionRecordingRequest {
            error: "".to_string(),
            termination_reason: None,
        })
        .unwrap_err();

        assert!(matches!(error, SessionStoreError::InvalidRequest(_)));
    }
}
