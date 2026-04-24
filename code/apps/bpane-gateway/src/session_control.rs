use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::Context;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value};
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use tokio_postgres::{Client, Connection, NoTls, Row, Socket};
use uuid::Uuid;

use crate::auth::AuthenticatedPrincipal;
use crate::automation_task::{
    AutomationTaskLogStream, AutomationTaskSessionSource, AutomationTaskState,
    AutomationTaskTransitionRequest, PersistAutomationTaskRequest, StoredAutomationTask,
    StoredAutomationTaskEvent, StoredAutomationTaskLog,
};
use crate::session_manager::{
    PersistedSessionRuntimeAssignment, SessionManagerProfile, SessionRuntimeAccess,
    SessionRuntimeAssignmentStatus,
};

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

#[derive(Debug, Deserialize)]
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
    pub recording: SessionRecordingPolicy,
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
                recordings: Mutex::new(Vec::new()),
                runtime_assignments: Mutex::new(HashMap::new()),
                recording_worker_assignments: Mutex::new(HashMap::new()),
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

fn validate_create_request(request: &CreateSessionRequest) -> Result<(), SessionStoreError> {
    if let Some(viewport) = &request.viewport {
        if viewport.width == 0 || viewport.height == 0 {
            return Err(SessionStoreError::InvalidRequest(
                "viewport width and height must be greater than zero".to_string(),
            ));
        }
    }
    if let Some(idle_timeout_sec) = request.idle_timeout_sec {
        if idle_timeout_sec == 0 {
            return Err(SessionStoreError::InvalidRequest(
                "idle_timeout_sec must be greater than zero when provided".to_string(),
            ));
        }
    }
    if let Some(integration_context) = &request.integration_context {
        if !integration_context.is_object() {
            return Err(SessionStoreError::InvalidRequest(
                "integration_context must be a JSON object when provided".to_string(),
            ));
        }
    }
    if let Some(retention_sec) = request.recording.retention_sec {
        if retention_sec == 0 {
            return Err(SessionStoreError::InvalidRequest(
                "recording.retention_sec must be greater than zero when provided".to_string(),
            ));
        }
    }
    Ok(())
}

fn validate_automation_delegate_request(
    request: &SetAutomationDelegateRequest,
) -> Result<(), SessionStoreError> {
    if request.client_id.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "client_id must not be empty".to_string(),
        ));
    }
    if let Some(issuer) = &request.issuer {
        if issuer.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "issuer must not be empty when provided".to_string(),
            ));
        }
    }
    Ok(())
}

fn validate_persist_automation_task_request(
    request: &PersistAutomationTaskRequest,
) -> Result<(), SessionStoreError> {
    if let Some(display_name) = &request.display_name {
        if display_name.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "display_name must not be empty when provided".to_string(),
            ));
        }
    }
    if request.executor.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "executor must not be empty".to_string(),
        ));
    }
    for artifact_ref in &request.labels {
        if artifact_ref.0.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "task label keys must not be empty".to_string(),
            ));
        }
    }
    Ok(())
}

fn validate_automation_task_transition_request(
    request: &AutomationTaskTransitionRequest,
) -> Result<(), SessionStoreError> {
    if request.event_type.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "event_type must not be empty".to_string(),
        ));
    }
    if request.event_message.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "event_message must not be empty".to_string(),
        ));
    }
    match request.state {
        AutomationTaskState::Succeeded => {
            if request.error.is_some() {
                return Err(SessionStoreError::InvalidRequest(
                    "succeeded automation tasks must not carry an error".to_string(),
                ));
            }
        }
        AutomationTaskState::Failed | AutomationTaskState::TimedOut => {
            let Some(error) = request.error.as_deref() else {
                return Err(SessionStoreError::InvalidRequest(
                    "failed or timed_out automation tasks require an error".to_string(),
                ));
            };
            if error.trim().is_empty() {
                return Err(SessionStoreError::InvalidRequest(
                    "automation task error must not be empty".to_string(),
                ));
            }
        }
        AutomationTaskState::Cancelled => {
            if request.error.is_some() {
                return Err(SessionStoreError::InvalidRequest(
                    "cancelled automation tasks must not carry an error".to_string(),
                ));
            }
        }
        _ => {}
    }
    for artifact_ref in &request.artifact_refs {
        if artifact_ref.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "artifact_refs entries must not be empty".to_string(),
            ));
        }
    }
    Ok(())
}

fn validate_automation_task_log_message(message: &str) -> Result<(), SessionStoreError> {
    if message.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "automation task log message must not be empty".to_string(),
        ));
    }
    Ok(())
}

fn validate_complete_recording_request(
    request: &CompleteSessionRecordingRequest,
) -> Result<(), SessionStoreError> {
    if request.source_path.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "source_path must not be empty".to_string(),
        ));
    }
    if let Some(mime_type) = &request.mime_type {
        if mime_type.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "mime_type must not be empty when provided".to_string(),
            ));
        }
    }
    Ok(())
}

fn validate_persist_completed_recording_request(
    request: &PersistCompletedSessionRecordingRequest,
) -> Result<(), SessionStoreError> {
    if request.artifact_ref.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "artifact_ref must not be empty".to_string(),
        ));
    }
    if let Some(mime_type) = &request.mime_type {
        if mime_type.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "mime_type must not be empty when provided".to_string(),
            ));
        }
    }
    Ok(())
}

fn validate_fail_recording_request(
    request: &FailSessionRecordingRequest,
) -> Result<(), SessionStoreError> {
    if request.error.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "error must not be empty".to_string(),
        ));
    }
    Ok(())
}

struct InMemorySessionStore {
    sessions: Mutex<Vec<StoredSession>>,
    automation_tasks: Mutex<Vec<StoredAutomationTask>>,
    automation_task_events: Mutex<Vec<StoredAutomationTaskEvent>>,
    automation_task_logs: Mutex<Vec<StoredAutomationTaskLog>>,
    recordings: Mutex<Vec<StoredSessionRecording>>,
    runtime_assignments: Mutex<HashMap<Uuid, PersistedSessionRuntimeAssignment>>,
    recording_worker_assignments: Mutex<HashMap<Uuid, PersistedSessionRecordingWorkerAssignment>>,
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

                ALTER TABLE control_sessions
                    ADD COLUMN IF NOT EXISTS automation_owner_client_id TEXT NULL;
                ALTER TABLE control_sessions
                    ADD COLUMN IF NOT EXISTS automation_owner_issuer TEXT NULL;
                ALTER TABLE control_sessions
                    ADD COLUMN IF NOT EXISTS automation_owner_display_name TEXT NULL;
                ALTER TABLE control_sessions
                    ADD COLUMN IF NOT EXISTS recording JSONB NOT NULL DEFAULT '{"mode":"disabled","format":"webm","retention_sec":null}'::jsonb;
                ALTER TABLE control_session_recordings
                    ADD COLUMN IF NOT EXISTS previous_recording_id UUID NULL REFERENCES control_session_recordings(id) ON DELETE SET NULL;
                ALTER TABLE control_session_recordings
                    ADD COLUMN IF NOT EXISTS termination_reason TEXT NULL;
                "#,
            )
            .await
            .map_err(|error| SessionStoreError::Backend(format!("failed to migrate postgres schema: {error}")))
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
                    recording,
                    runtime_binding,
                    created_at,
                    updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11::jsonb, $12::jsonb, $13::jsonb, $14, $15, $15)
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
                  AND state IN ('pending', 'starting', 'running', 'awaiting_input')
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

        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;

        row_to_stored_automation_task(&row).map(Some)
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

        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;

        row_to_stored_automation_task(&row).map(Some)
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

fn json_labels(labels: &HashMap<String, String>) -> Value {
    let mut object = JsonMap::new();
    for (key, value) in labels {
        object.insert(key.clone(), Value::String(value.clone()));
    }
    Value::Object(object)
}

fn json_string_array(values: &[String]) -> Value {
    Value::Array(
        values
            .iter()
            .cloned()
            .map(Value::String)
            .collect::<Vec<_>>(),
    )
}

fn recording_mime_type(format: SessionRecordingFormat) -> &'static str {
    match format {
        SessionRecordingFormat::Webm => "video/webm",
    }
}

fn json_recording_policy(recording: &SessionRecordingPolicy) -> Result<Value, SessionStoreError> {
    serde_json::to_value(recording).map_err(|error| {
        SessionStoreError::Backend(format!("failed to encode recording policy: {error}"))
    })
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

fn row_to_stored_session(row: &Row) -> Result<StoredSession, SessionStoreError> {
    let state = row
        .get::<_, String>("state")
        .parse::<SessionLifecycleState>()
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?;
    let owner_mode = row
        .get::<_, String>("owner_mode")
        .parse::<SessionOwnerMode>()
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?;
    let labels_value: Value = row.get("labels");
    let labels = labels_value
        .as_object()
        .context("labels column must be a JSON object")
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?
        .iter()
        .map(|(key, value)| {
            Ok((
                key.clone(),
                value
                    .as_str()
                    .context("label values must be strings")
                    .map_err(|error| SessionStoreError::Backend(error.to_string()))?
                    .to_string(),
            ))
        })
        .collect::<Result<HashMap<_, _>, SessionStoreError>>()?;
    let recording = serde_json::from_value::<SessionRecordingPolicy>(row.get("recording"))
        .map_err(|error| {
            SessionStoreError::Backend(format!("failed to decode recording policy: {error}"))
        })?;

    let width = row.get::<_, i32>("viewport_width");
    let height = row.get::<_, i32>("viewport_height");
    let automation_owner_client_id = row.get::<_, Option<String>>("automation_owner_client_id");
    let automation_owner_issuer = row.get::<_, Option<String>>("automation_owner_issuer");

    Ok(StoredSession {
        id: row.get("id"),
        state,
        template_id: row.get("template_id"),
        owner_mode,
        viewport: SessionViewport {
            width: width as u16,
            height: height as u16,
        },
        owner: SessionOwner {
            subject: row.get("owner_subject"),
            issuer: row.get("owner_issuer"),
            display_name: row.get("owner_display_name"),
        },
        automation_delegate: match (automation_owner_client_id, automation_owner_issuer) {
            (Some(client_id), Some(issuer)) => Some(SessionAutomationDelegate {
                client_id,
                issuer,
                display_name: row.get("automation_owner_display_name"),
            }),
            _ => None,
        },
        idle_timeout_sec: row
            .get::<_, Option<i32>>("idle_timeout_sec")
            .map(|value| value as u32),
        labels,
        integration_context: row.get("integration_context"),
        recording,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        stopped_at: row.get("stopped_at"),
    })
}

fn row_to_stored_session_recording(row: &Row) -> Result<StoredSessionRecording, SessionStoreError> {
    let state = row
        .get::<_, String>("state")
        .parse::<SessionRecordingState>()
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?;
    let format = row
        .get::<_, String>("format")
        .parse::<SessionRecordingFormat>()
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?;
    let termination_reason = row
        .get::<_, Option<String>>("termination_reason")
        .map(|value| {
            value
                .parse::<SessionRecordingTerminationReason>()
                .map_err(|error| SessionStoreError::Backend(error.to_string()))
        })
        .transpose()?;

    Ok(StoredSessionRecording {
        id: row.get("id"),
        session_id: row.get("session_id"),
        previous_recording_id: row.get("previous_recording_id"),
        state,
        format,
        mime_type: row.get("mime_type"),
        bytes: row
            .get::<_, Option<i64>>("byte_count")
            .map(|value| value as u64),
        duration_ms: row
            .get::<_, Option<i64>>("duration_ms")
            .map(|value| value as u64),
        error: row.get("error"),
        termination_reason,
        artifact_ref: row.get("artifact_ref"),
        started_at: row.get("started_at"),
        completed_at: row.get("completed_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn row_to_stored_automation_task(row: &Row) -> Result<StoredAutomationTask, SessionStoreError> {
    let state = row
        .get::<_, String>("state")
        .parse::<AutomationTaskState>()
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?;
    let session_source = row
        .get::<_, String>("session_source")
        .parse::<AutomationTaskSessionSource>()
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?;
    let labels_value: Value = row.get("labels");
    let labels = labels_value
        .as_object()
        .context("automation task labels column must be a JSON object")
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?
        .iter()
        .map(|(key, value)| {
            Ok((
                key.clone(),
                value
                    .as_str()
                    .context("automation task label values must be strings")
                    .map_err(|error| SessionStoreError::Backend(error.to_string()))?
                    .to_string(),
            ))
        })
        .collect::<Result<HashMap<_, _>, SessionStoreError>>()?;
    let artifact_refs_value: Value = row.get("artifact_refs");
    let artifact_refs = artifact_refs_value
        .as_array()
        .context("automation task artifact_refs column must be a JSON array")
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .context("automation task artifact_refs entries must be strings")
                .map(|entry| entry.to_string())
                .map_err(|error| SessionStoreError::Backend(error.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(StoredAutomationTask {
        id: row.get("id"),
        display_name: row.get("display_name"),
        executor: row.get("executor"),
        state,
        session_id: row.get("session_id"),
        session_source,
        input: row.get("input"),
        output: row.get("output"),
        error: row.get("error"),
        artifact_refs,
        labels,
        cancel_requested_at: row.get("cancel_requested_at"),
        started_at: row.get("started_at"),
        completed_at: row.get("completed_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn row_to_stored_automation_task_event(
    row: &Row,
) -> Result<StoredAutomationTaskEvent, SessionStoreError> {
    Ok(StoredAutomationTaskEvent {
        id: row.get("id"),
        task_id: row.get("task_id"),
        event_type: row.get("event_type"),
        message: row.get("message"),
        data: row.get("data"),
        created_at: row.get("created_at"),
    })
}

fn row_to_stored_automation_task_log(
    row: &Row,
) -> Result<StoredAutomationTaskLog, SessionStoreError> {
    let stream = row
        .get::<_, String>("stream")
        .parse::<AutomationTaskLogStream>()
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?;
    Ok(StoredAutomationTaskLog {
        id: row.get("id"),
        task_id: row.get("task_id"),
        stream,
        message: row.get("message"),
        created_at: row.get("created_at"),
    })
}

fn row_to_runtime_assignment(
    row: &Row,
) -> Result<PersistedSessionRuntimeAssignment, SessionStoreError> {
    let status = row
        .get::<_, String>("status")
        .parse::<SessionRuntimeAssignmentStatus>()
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?;
    Ok(PersistedSessionRuntimeAssignment {
        session_id: row.get("session_id"),
        runtime_binding: row.get("runtime_binding"),
        status,
        agent_socket_path: row.get("agent_socket_path"),
        container_name: row.get("container_name"),
        cdp_endpoint: row.get("cdp_endpoint"),
    })
}

fn row_to_recording_worker_assignment(
    row: &Row,
) -> Result<PersistedSessionRecordingWorkerAssignment, SessionStoreError> {
    let status = row
        .get::<_, String>("status")
        .parse::<SessionRecordingWorkerAssignmentStatus>()
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?;
    let process_id = row
        .get::<_, Option<i64>>("process_id")
        .map(|value| u32::try_from(value))
        .transpose()
        .map_err(|error| {
            SessionStoreError::Backend(format!(
                "recording worker process_id is out of range: {error}"
            ))
        })?;
    Ok(PersistedSessionRecordingWorkerAssignment {
        session_id: row.get("session_id"),
        recording_id: row.get("recording_id"),
        status,
        process_id,
    })
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
    fn rejects_empty_recording_source_path() {
        let error = validate_complete_recording_request(&CompleteSessionRecordingRequest {
            source_path: "   ".to_string(),
            mime_type: None,
            bytes: None,
            duration_ms: None,
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
