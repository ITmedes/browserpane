use std::collections::HashMap;
use std::str::FromStr;

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::extensions::{AppliedExtension, AppliedExtensionResource};
use crate::session_manager::SessionRuntimeAccess;

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
    Released,
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
            Self::Released => "released",
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
            "released" => Ok(Self::Released),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserContextState {
    Ready,
    Deleted,
}

impl FromStr for BrowserContextState {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "ready" => Ok(Self::Ready),
            "deleted" => Ok(Self::Deleted),
            _ => Err("unknown browser context state"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserContextPersistenceMode {
    Reusable,
    Ephemeral,
}

impl BrowserContextPersistenceMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Reusable => "reusable",
            Self::Ephemeral => "ephemeral",
        }
    }
}

impl FromStr for BrowserContextPersistenceMode {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "reusable" => Ok(Self::Reusable),
            "ephemeral" => Ok(Self::Ephemeral),
            _ => Err("unknown browser context persistence mode"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionBrowserContextMode {
    Fresh,
    Ephemeral,
    Reusable,
}

impl SessionBrowserContextMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::Ephemeral => "ephemeral",
            Self::Reusable => "reusable",
        }
    }
}

impl FromStr for SessionBrowserContextMode {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "fresh" => Ok(Self::Fresh),
            "ephemeral" => Ok(Self::Ephemeral),
            "reusable" => Ok(Self::Reusable),
            _ => Err("unknown session browser context mode"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionBrowserContextRequest {
    pub mode: SessionBrowserContextMode,
    #[serde(default)]
    pub context_id: Option<Uuid>,
}

impl Default for SessionBrowserContextRequest {
    fn default() -> Self {
        Self {
            mode: SessionBrowserContextMode::Fresh,
            context_id: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionBrowserContextResource {
    pub mode: SessionBrowserContextMode,
    pub context_id: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct PersistBrowserContextRequest {
    pub id: Option<Uuid>,
    pub name: String,
    pub description: Option<String>,
    pub labels: HashMap<String, String>,
    pub persistence_mode: BrowserContextPersistenceMode,
    pub retention_sec: Option<u32>,
    pub max_profile_storage_bytes: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct StoredBrowserContext {
    pub id: Uuid,
    pub owner_subject: String,
    pub owner_issuer: String,
    pub name: String,
    pub description: Option<String>,
    pub labels: HashMap<String, String>,
    pub persistence_mode: BrowserContextPersistenceMode,
    pub retention_sec: Option<u32>,
    pub max_profile_storage_bytes: Option<u64>,
    pub state: BrowserContextState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct BrowserContextRetentionCandidate {
    pub context: StoredBrowserContext,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BrowserContextResource {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub labels: HashMap<String, String>,
    pub persistence_mode: BrowserContextPersistenceMode,
    pub retention_sec: Option<u32>,
    pub retention_expires_at: Option<DateTime<Utc>>,
    pub max_profile_storage_bytes: Option<u64>,
    pub state: BrowserContextState,
    pub usage: BrowserContextUsageResource,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct BrowserContextUsageResource {
    pub visible_session_count: u32,
    pub active_runtime_session_count: u32,
    pub active_runtime_session_id: Option<Uuid>,
    pub profile_storage_bytes: Option<u64>,
    pub profile_storage_limit_exceeded: bool,
}

#[derive(Debug, Serialize)]
pub struct BrowserContextListResponse {
    pub contexts: Vec<BrowserContextResource>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionRuntimeState {
    NotStarted,
    Starting,
    Running,
    Released,
    Stopping,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionRuntimeResumeMode {
    FreshStart,
    ExactLive,
    ProfileRestart,
    Released,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionPresenceState {
    Empty,
    Connected,
    AutomationOwned,
    RecordingOnly,
    Idle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SessionConnectionCounts {
    pub interactive_clients: u32,
    pub owner_clients: u32,
    pub viewer_clients: u32,
    pub recorder_clients: u32,
    pub automation_clients: u32,
    pub total_clients: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStopBlockerKind {
    OwnerClients,
    ViewerClients,
    RecorderClients,
    AutomationOwner,
    RecordingActivity,
    AutomationTasks,
    WorkflowRuns,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SessionStopBlocker {
    pub kind: SessionStopBlockerKind,
    pub count: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SessionStopEligibility {
    pub allowed: bool,
    pub blockers: Vec<SessionStopBlocker>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SessionIdleStatus {
    pub idle_timeout_sec: Option<u32>,
    pub idle_since: Option<DateTime<Utc>>,
    pub idle_deadline: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SessionStatusSummary {
    pub runtime_state: SessionRuntimeState,
    pub runtime_resume_mode: SessionRuntimeResumeMode,
    pub presence_state: SessionPresenceState,
    pub connection_counts: SessionConnectionCounts,
    pub stop_eligibility: SessionStopEligibility,
    pub idle: SessionIdleStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SessionResource {
    pub id: Uuid,
    pub state: SessionLifecycleState,
    pub template_id: Option<String>,
    pub browser_context: SessionBrowserContextResource,
    pub owner_mode: SessionOwnerMode,
    pub viewport: SessionViewport,
    pub capabilities: SessionCapabilities,
    pub owner: SessionOwner,
    pub automation_delegate: Option<SessionAutomationDelegate>,
    pub idle_timeout_sec: Option<u32>,
    pub labels: HashMap<String, String>,
    pub integration_context: Option<Value>,
    pub extensions: Vec<AppliedExtensionResource>,
    pub recording: crate::session_control::SessionRecordingPolicy,
    pub connect: SessionConnectInfo,
    pub runtime: SessionRuntimeInfo,
    pub status: SessionStatusSummary,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub runtime_released_at: Option<DateTime<Utc>>,
    pub stopped_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateSessionRequest {
    #[serde(default)]
    pub template_id: Option<String>,
    #[serde(default)]
    pub browser_context: Option<SessionBrowserContextRequest>,
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
    pub recording: crate::session_control::SessionRecordingPolicy,
    #[serde(skip)]
    pub extensions: Vec<AppliedExtension>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SessionTemplateDefaults {
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
    pub recording: Option<crate::session_control::SessionRecordingPolicy>,
}

#[derive(Debug, Clone)]
pub struct PersistSessionTemplateRequest {
    pub name: String,
    pub description: Option<String>,
    pub labels: HashMap<String, String>,
    pub defaults: SessionTemplateDefaults,
}

#[derive(Debug, Clone)]
pub struct StoredSessionTemplate {
    pub id: Uuid,
    pub owner_subject: String,
    pub owner_issuer: String,
    pub name: String,
    pub description: Option<String>,
    pub labels: HashMap<String, String>,
    pub defaults: SessionTemplateDefaults,
    pub version: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SessionTemplateResource {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub labels: HashMap<String, String>,
    pub defaults: SessionTemplateDefaults,
    pub version: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct SessionTemplateListResponse {
    pub templates: Vec<SessionTemplateResource>,
}

impl StoredSessionTemplate {
    pub fn to_resource(&self) -> SessionTemplateResource {
        SessionTemplateResource {
            id: self.id,
            name: self.name.clone(),
            description: self.description.clone(),
            labels: self.labels.clone(),
            defaults: self.defaults.clone(),
            version: self.version,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SetAutomationDelegateRequest {
    pub client_id: String,
    #[serde(default)]
    pub issuer: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
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
    pub browser_context: SessionBrowserContextResource,
    pub owner_mode: SessionOwnerMode,
    pub viewport: SessionViewport,
    pub owner: SessionOwner,
    pub automation_delegate: Option<SessionAutomationDelegate>,
    pub idle_timeout_sec: Option<u32>,
    pub labels: HashMap<String, String>,
    pub integration_context: Option<Value>,
    pub extensions: Vec<AppliedExtension>,
    pub recording: crate::session_control::SessionRecordingPolicy,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub runtime_released_at: Option<DateTime<Utc>>,
    pub stopped_at: Option<DateTime<Utc>>,
}

impl StoredSession {
    pub fn to_resource(
        &self,
        public_gateway_url: &str,
        runtime: SessionRuntimeInfo,
        status: SessionStatusSummary,
        state_override: Option<SessionLifecycleState>,
    ) -> SessionResource {
        SessionResource {
            id: self.id,
            state: state_override.unwrap_or(self.state),
            template_id: self.template_id.clone(),
            browser_context: self.browser_context.clone(),
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
            status,
            created_at: self.created_at,
            updated_at: self.updated_at,
            runtime_released_at: self.runtime_released_at,
            stopped_at: self.stopped_at,
        }
    }
}

impl StoredBrowserContext {
    pub fn to_resource(&self) -> BrowserContextResource {
        BrowserContextResource {
            id: self.id,
            name: self.name.clone(),
            description: self.description.clone(),
            labels: self.labels.clone(),
            persistence_mode: self.persistence_mode,
            retention_sec: self.retention_sec,
            retention_expires_at: self.retention_expires_at(),
            max_profile_storage_bytes: self.max_profile_storage_bytes,
            state: self.state,
            usage: BrowserContextUsageResource::default(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            last_used_at: self.last_used_at,
            deleted_at: self.deleted_at,
        }
    }

    pub fn retention_expires_at(&self) -> Option<DateTime<Utc>> {
        self.retention_sec.map(|retention_sec| {
            let base = self.last_used_at.unwrap_or(self.created_at);
            base + ChronoDuration::seconds(i64::from(retention_sec))
        })
    }
}
