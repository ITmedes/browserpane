use super::*;

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
