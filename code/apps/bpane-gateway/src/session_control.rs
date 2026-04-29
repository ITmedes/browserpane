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

mod in_memory;
mod rows;
mod validation;

use in_memory::*;
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
            backend: SessionStoreBackend::InMemory(Arc::new(InMemorySessionStore::new(
                SessionStoreConfig::from(runtime_profile),
            ))),
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
mod tests;
