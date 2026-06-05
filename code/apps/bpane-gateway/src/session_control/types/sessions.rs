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
    Queued,
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
            Self::Queued => "queued",
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
            "queued" => Ok(Self::Queued),
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
pub enum EgressProfileState {
    Ready,
    Disabled,
}

impl EgressProfileState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Disabled => "disabled",
        }
    }
}

impl FromStr for EgressProfileState {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "ready" => Ok(Self::Ready),
            "disabled" => Ok(Self::Disabled),
            _ => Err("unknown egress profile state"),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EgressTrafficObservationMode {
    #[default]
    MetadataOnly,
    TlsIntercept,
}

impl EgressTrafficObservationMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MetadataOnly => "metadata_only",
            Self::TlsIntercept => "tls_intercept",
        }
    }
}

impl FromStr for EgressTrafficObservationMode {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "metadata_only" => Ok(Self::MetadataOnly),
            "tls_intercept" => Ok(Self::TlsIntercept),
            _ => Err("unknown egress traffic observation mode"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionGeolocation {
    pub latitude: f64,
    pub longitude: f64,
    #[serde(default)]
    pub accuracy_meters: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SessionNetworkIdentity {
    #[serde(default)]
    pub locale: Option<String>,
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub timezone: Option<String>,
    #[serde(default)]
    pub geolocation: Option<SessionGeolocation>,
    #[serde(default)]
    pub user_agent: Option<String>,
    #[serde(default)]
    pub browser_identity: Option<String>,
    #[serde(default)]
    pub egress_profile_id: Option<Uuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectState {
    Active,
    Archived,
}

impl ProjectState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Archived => "archived",
        }
    }
}

impl FromStr for ProjectState {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "active" => Ok(Self::Active),
            "archived" => Ok(Self::Archived),
            _ => Err("unknown project state"),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectQuotas {
    #[serde(default)]
    pub max_active_sessions: Option<u32>,
    #[serde(default)]
    pub max_active_workflow_runs: Option<u32>,
    #[serde(default)]
    pub max_retained_storage_bytes: Option<u64>,
    #[serde(default)]
    pub max_session_creations: Option<u32>,
    #[serde(default)]
    pub max_session_creations_per_window: Option<u32>,
    #[serde(default)]
    pub session_creation_window_sec: Option<u32>,
    #[serde(default)]
    pub max_runtime_usage_ms: Option<u64>,
    #[serde(default)]
    pub max_egress_total_bytes: Option<u64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectUsageBudgetEnforcement {
    #[default]
    WarningOnly,
    BlockSessionCreation,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectPolicy {
    #[serde(default)]
    pub allowed_session_template_ids: Vec<String>,
    #[serde(default)]
    pub allowed_egress_profile_ids: Vec<Uuid>,
    #[serde(default)]
    pub usage_budget_enforcement: ProjectUsageBudgetEnforcement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectAdmissionState {
    Allowed,
    Queued,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectAdmissionReasonCode {
    OwnerScopeUnbounded,
    ProjectQuotaAvailable,
    ActiveSessionQuotaExceeded,
    SessionCreationBudgetExceeded,
    SessionCreationRateExceeded,
    RuntimeUsageBudgetExceeded,
    ActiveWorkflowRunQuotaExceeded,
    ProjectArchived,
    SessionTemplateNotAllowed,
    EgressProfileNotAllowed,
}

impl ProjectAdmissionReasonCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OwnerScopeUnbounded => "owner_scope_unbounded",
            Self::ProjectQuotaAvailable => "project_quota_available",
            Self::ActiveSessionQuotaExceeded => "active_session_quota_exceeded",
            Self::SessionCreationBudgetExceeded => "session_creation_budget_exceeded",
            Self::SessionCreationRateExceeded => "session_creation_rate_exceeded",
            Self::RuntimeUsageBudgetExceeded => "runtime_usage_budget_exceeded",
            Self::ActiveWorkflowRunQuotaExceeded => "active_workflow_run_quota_exceeded",
            Self::ProjectArchived => "project_archived",
            Self::SessionTemplateNotAllowed => "session_template_not_allowed",
            Self::EgressProfileNotAllowed => "egress_profile_not_allowed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectAdmissionDecision {
    pub state: ProjectAdmissionState,
    pub reason_code: ProjectAdmissionReasonCode,
    pub message: String,
    #[serde(default)]
    pub project_id: Option<Uuid>,
    #[serde(default)]
    pub active_sessions: Option<u32>,
    #[serde(default)]
    pub max_active_sessions: Option<u32>,
    #[serde(default)]
    pub active_workflow_runs: Option<u32>,
    #[serde(default)]
    pub max_active_workflow_runs: Option<u32>,
    #[serde(default)]
    pub session_creations: Option<u32>,
    #[serde(default)]
    pub max_session_creations: Option<u32>,
    #[serde(default)]
    pub session_creations_in_window: Option<u32>,
    #[serde(default)]
    pub max_session_creations_per_window: Option<u32>,
    #[serde(default)]
    pub session_creation_window_sec: Option<u32>,
    #[serde(default)]
    pub runtime_usage_ms: Option<u64>,
    #[serde(default)]
    pub max_runtime_usage_ms: Option<u64>,
    pub checked_at: DateTime<Utc>,
}

impl ProjectAdmissionDecision {
    pub fn owner_scope_unbounded(checked_at: DateTime<Utc>) -> Self {
        Self {
            state: ProjectAdmissionState::Allowed,
            reason_code: ProjectAdmissionReasonCode::OwnerScopeUnbounded,
            message: "No project was selected; owner-scoped admission is unbounded.".to_string(),
            project_id: None,
            active_sessions: None,
            max_active_sessions: None,
            active_workflow_runs: None,
            max_active_workflow_runs: None,
            session_creations: None,
            max_session_creations: None,
            session_creations_in_window: None,
            max_session_creations_per_window: None,
            session_creation_window_sec: None,
            runtime_usage_ms: None,
            max_runtime_usage_ms: None,
            checked_at,
        }
    }

    pub fn project_quota_available(
        project_id: Uuid,
        active_sessions: u32,
        max_active_sessions: Option<u32>,
        checked_at: DateTime<Utc>,
    ) -> Self {
        Self {
            state: ProjectAdmissionState::Allowed,
            reason_code: ProjectAdmissionReasonCode::ProjectQuotaAvailable,
            message: "Project admission allowed.".to_string(),
            project_id: Some(project_id),
            active_sessions: Some(active_sessions),
            max_active_sessions,
            active_workflow_runs: None,
            max_active_workflow_runs: None,
            session_creations: None,
            max_session_creations: None,
            session_creations_in_window: None,
            max_session_creations_per_window: None,
            session_creation_window_sec: None,
            runtime_usage_ms: None,
            max_runtime_usage_ms: None,
            checked_at,
        }
    }

    pub fn workflow_quota_available(
        project_id: Uuid,
        active_workflow_runs: u32,
        max_active_workflow_runs: Option<u32>,
        checked_at: DateTime<Utc>,
    ) -> Self {
        Self {
            state: ProjectAdmissionState::Allowed,
            reason_code: ProjectAdmissionReasonCode::ProjectQuotaAvailable,
            message: "Project workflow admission allowed.".to_string(),
            project_id: Some(project_id),
            active_sessions: None,
            max_active_sessions: None,
            active_workflow_runs: Some(active_workflow_runs),
            max_active_workflow_runs,
            session_creations: None,
            max_session_creations: None,
            session_creations_in_window: None,
            max_session_creations_per_window: None,
            session_creation_window_sec: None,
            runtime_usage_ms: None,
            max_runtime_usage_ms: None,
            checked_at,
        }
    }

    pub fn rejected(
        project_id: Uuid,
        reason_code: ProjectAdmissionReasonCode,
        message: String,
        active_sessions: u32,
        max_active_sessions: Option<u32>,
        checked_at: DateTime<Utc>,
    ) -> Self {
        Self {
            state: ProjectAdmissionState::Rejected,
            reason_code,
            message,
            project_id: Some(project_id),
            active_sessions: Some(active_sessions),
            max_active_sessions,
            active_workflow_runs: None,
            max_active_workflow_runs: None,
            session_creations: None,
            max_session_creations: None,
            session_creations_in_window: None,
            max_session_creations_per_window: None,
            session_creation_window_sec: None,
            runtime_usage_ms: None,
            max_runtime_usage_ms: None,
            checked_at,
        }
    }

    pub fn session_creation_budget_rejected(
        project_id: Uuid,
        session_creations: u32,
        max_session_creations: u32,
        checked_at: DateTime<Utc>,
    ) -> Self {
        Self {
            state: ProjectAdmissionState::Rejected,
            reason_code: ProjectAdmissionReasonCode::SessionCreationBudgetExceeded,
            message: format!(
                "Project session creation budget is exhausted ({session_creations}/{max_session_creations})."
            ),
            project_id: Some(project_id),
            active_sessions: None,
            max_active_sessions: None,
            active_workflow_runs: None,
            max_active_workflow_runs: None,
            session_creations: Some(session_creations),
            max_session_creations: Some(max_session_creations),
            session_creations_in_window: None,
            max_session_creations_per_window: None,
            session_creation_window_sec: None,
            runtime_usage_ms: None,
            max_runtime_usage_ms: None,
            checked_at,
        }
    }

    pub fn session_creation_rate_rejected(
        project_id: Uuid,
        session_creations_in_window: u32,
        max_session_creations_per_window: u32,
        session_creation_window_sec: u32,
        checked_at: DateTime<Utc>,
    ) -> Self {
        Self {
            state: ProjectAdmissionState::Rejected,
            reason_code: ProjectAdmissionReasonCode::SessionCreationRateExceeded,
            message: format!(
                "Project session creation rate limit is exhausted ({session_creations_in_window}/{max_session_creations_per_window} in {session_creation_window_sec}s)."
            ),
            project_id: Some(project_id),
            active_sessions: None,
            max_active_sessions: None,
            active_workflow_runs: None,
            max_active_workflow_runs: None,
            session_creations: None,
            max_session_creations: None,
            session_creations_in_window: Some(session_creations_in_window),
            max_session_creations_per_window: Some(max_session_creations_per_window),
            session_creation_window_sec: Some(session_creation_window_sec),
            runtime_usage_ms: None,
            max_runtime_usage_ms: None,
            checked_at,
        }
    }

    pub fn runtime_usage_budget_rejected(
        project_id: Uuid,
        runtime_usage_ms: u64,
        max_runtime_usage_ms: u64,
        checked_at: DateTime<Utc>,
    ) -> Self {
        Self {
            state: ProjectAdmissionState::Rejected,
            reason_code: ProjectAdmissionReasonCode::RuntimeUsageBudgetExceeded,
            message: format!(
                "Project browser runtime budget is exhausted ({runtime_usage_ms}/{max_runtime_usage_ms} ms)."
            ),
            project_id: Some(project_id),
            active_sessions: None,
            max_active_sessions: None,
            active_workflow_runs: None,
            max_active_workflow_runs: None,
            session_creations: None,
            max_session_creations: None,
            session_creations_in_window: None,
            max_session_creations_per_window: None,
            session_creation_window_sec: None,
            runtime_usage_ms: Some(runtime_usage_ms),
            max_runtime_usage_ms: Some(max_runtime_usage_ms),
            checked_at,
        }
    }

    pub fn workflow_queued(
        project_id: Uuid,
        reason_code: ProjectAdmissionReasonCode,
        message: String,
        active_workflow_runs: u32,
        max_active_workflow_runs: Option<u32>,
        checked_at: DateTime<Utc>,
    ) -> Self {
        Self {
            state: ProjectAdmissionState::Queued,
            reason_code,
            message,
            project_id: Some(project_id),
            active_sessions: None,
            max_active_sessions: None,
            active_workflow_runs: Some(active_workflow_runs),
            max_active_workflow_runs,
            session_creations: None,
            max_session_creations: None,
            session_creations_in_window: None,
            max_session_creations_per_window: None,
            session_creation_window_sec: None,
            runtime_usage_ms: None,
            max_runtime_usage_ms: None,
            checked_at,
        }
    }

    pub fn session_queued(
        project_id: Uuid,
        reason_code: ProjectAdmissionReasonCode,
        message: String,
        active_sessions: u32,
        max_active_sessions: Option<u32>,
        checked_at: DateTime<Utc>,
    ) -> Self {
        Self {
            state: ProjectAdmissionState::Queued,
            reason_code,
            message,
            project_id: Some(project_id),
            active_sessions: Some(active_sessions),
            max_active_sessions,
            active_workflow_runs: None,
            max_active_workflow_runs: None,
            session_creations: None,
            max_session_creations: None,
            session_creations_in_window: None,
            max_session_creations_per_window: None,
            session_creation_window_sec: None,
            runtime_usage_ms: None,
            max_runtime_usage_ms: None,
            checked_at,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PersistProjectRequest {
    pub name: String,
    pub description: Option<String>,
    pub labels: HashMap<String, String>,
    pub quotas: ProjectQuotas,
    pub policy: ProjectPolicy,
    pub state: ProjectState,
}

#[derive(Debug, Clone)]
pub struct StoredProject {
    pub id: Uuid,
    pub owner_subject: String,
    pub owner_issuer: String,
    pub name: String,
    pub description: Option<String>,
    pub labels: HashMap<String, String>,
    pub quotas: ProjectQuotas,
    pub policy: ProjectPolicy,
    pub state: ProjectState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectUsageResource {
    pub project_id: Uuid,
    pub active_sessions: u32,
    pub queued_sessions: u32,
    pub session_creations: u32,
    pub max_session_creations: Option<u32>,
    pub max_active_sessions: Option<u32>,
    pub active_workflow_runs: u32,
    pub max_active_workflow_runs: Option<u32>,
    pub runtime_usage_ms: u64,
    pub max_runtime_usage_ms: Option<u64>,
    pub egress_rx_bytes: u64,
    pub egress_tx_bytes: u64,
    pub egress_total_bytes: u64,
    pub max_egress_total_bytes: Option<u64>,
    pub retained_storage_bytes: u64,
    pub max_retained_storage_bytes: Option<u64>,
    pub alerts: Vec<ProjectUsageAlertResource>,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectUsageAlertMetric {
    SessionCreations,
    RuntimeUsageMs,
    EgressTotalBytes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectUsageAlertState {
    ApproachingLimit,
    Exceeded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectUsageAlertResource {
    pub metric: ProjectUsageAlertMetric,
    pub state: ProjectUsageAlertState,
    pub current_value: u64,
    pub limit_value: u64,
    pub threshold_percent: u8,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectResource {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub labels: HashMap<String, String>,
    pub quotas: ProjectQuotas,
    pub policy: ProjectPolicy,
    pub state: ProjectState,
    pub usage: ProjectUsageResource,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionProjectResource {
    pub id: Uuid,
    pub name: String,
    pub state: ProjectState,
}

#[derive(Debug, Serialize)]
pub struct ProjectListResponse {
    pub projects: Vec<ProjectResource>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServicePrincipalState {
    Active,
    Disabled,
}

impl ServicePrincipalState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Disabled => "disabled",
        }
    }
}

impl FromStr for ServicePrincipalState {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "active" => Ok(Self::Active),
            "disabled" => Ok(Self::Disabled),
            _ => Err("unknown service principal state"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PersistServicePrincipalRequest {
    pub name: String,
    pub description: Option<String>,
    pub client_id: String,
    pub issuer: String,
    pub labels: HashMap<String, String>,
    pub scopes: Vec<String>,
    pub allowed_project_ids: Vec<Uuid>,
    pub state: ServicePrincipalState,
}

#[derive(Debug, Clone)]
pub struct StoredServicePrincipal {
    pub id: Uuid,
    pub owner_subject: String,
    pub owner_issuer: String,
    pub name: String,
    pub description: Option<String>,
    pub client_id: String,
    pub issuer: String,
    pub labels: HashMap<String, String>,
    pub scopes: Vec<String>,
    pub allowed_project_ids: Vec<Uuid>,
    pub state: ServicePrincipalState,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub last_delegated_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ServicePrincipalResource {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub client_id: String,
    pub issuer: String,
    pub labels: HashMap<String, String>,
    pub scopes: Vec<String>,
    pub allowed_project_ids: Vec<Uuid>,
    pub state: ServicePrincipalState,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub last_delegated_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ServicePrincipalListResponse {
    pub service_principals: Vec<ServicePrincipalResource>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityMappingKind {
    User,
    Group,
    Claim,
    ServicePrincipal,
}

impl IdentityMappingKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Group => "group",
            Self::Claim => "claim",
            Self::ServicePrincipal => "service_principal",
        }
    }
}

impl FromStr for IdentityMappingKind {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "user" => Ok(Self::User),
            "group" => Ok(Self::Group),
            "claim" => Ok(Self::Claim),
            "service_principal" => Ok(Self::ServicePrincipal),
            _ => Err("unknown identity mapping kind"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityMappingState {
    Active,
    Disabled,
}

impl IdentityMappingState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Disabled => "disabled",
        }
    }
}

impl FromStr for IdentityMappingState {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "active" => Ok(Self::Active),
            "disabled" => Ok(Self::Disabled),
            _ => Err("unknown identity mapping state"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PersistIdentityMappingRequest {
    pub name: String,
    pub description: Option<String>,
    pub kind: IdentityMappingKind,
    pub issuer: String,
    pub external_id: String,
    pub claim_name: Option<String>,
    pub service_principal_id: Option<Uuid>,
    pub project_id: Uuid,
    pub labels: HashMap<String, String>,
    pub scopes: Vec<String>,
    pub state: IdentityMappingState,
}

#[derive(Debug, Clone)]
pub struct StoredIdentityMapping {
    pub id: Uuid,
    pub owner_subject: String,
    pub owner_issuer: String,
    pub name: String,
    pub description: Option<String>,
    pub kind: IdentityMappingKind,
    pub issuer: String,
    pub external_id: String,
    pub claim_name: Option<String>,
    pub service_principal_id: Option<Uuid>,
    pub project_id: Uuid,
    pub labels: HashMap<String, String>,
    pub scopes: Vec<String>,
    pub state: IdentityMappingState,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IdentityMappingResource {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub kind: IdentityMappingKind,
    pub issuer: String,
    pub external_id: String,
    pub claim_name: Option<String>,
    pub service_principal_id: Option<Uuid>,
    pub project_id: Uuid,
    pub labels: HashMap<String, String>,
    pub scopes: Vec<String>,
    pub state: IdentityMappingState,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct IdentityMappingListResponse {
    pub identity_mappings: Vec<IdentityMappingResource>,
}

impl StoredIdentityMapping {
    pub fn to_resource(&self) -> IdentityMappingResource {
        IdentityMappingResource {
            id: self.id,
            name: self.name.clone(),
            description: self.description.clone(),
            kind: self.kind,
            issuer: self.issuer.clone(),
            external_id: self.external_id.clone(),
            claim_name: self.claim_name.clone(),
            service_principal_id: self.service_principal_id,
            project_id: self.project_id,
            labels: self.labels.clone(),
            scopes: self.scopes.clone(),
            state: self.state,
            last_seen_at: self.last_seen_at,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

impl StoredServicePrincipal {
    pub fn to_resource(&self) -> ServicePrincipalResource {
        ServicePrincipalResource {
            id: self.id,
            name: self.name.clone(),
            description: self.description.clone(),
            client_id: self.client_id.clone(),
            issuer: self.issuer.clone(),
            labels: self.labels.clone(),
            scopes: self.scopes.clone(),
            allowed_project_ids: self.allowed_project_ids.clone(),
            state: self.state,
            last_seen_at: self.last_seen_at,
            last_delegated_at: self.last_delegated_at,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

impl StoredProject {
    const USAGE_ALERT_THRESHOLD_PERCENT: u8 = 80;

    pub fn usage(
        &self,
        active_sessions: u32,
        queued_sessions: u32,
        session_creations: u32,
        active_workflow_runs: u32,
        runtime_usage_ms: u64,
        egress_rx_bytes: u64,
        egress_tx_bytes: u64,
        retained_storage_bytes: u64,
        observed_at: DateTime<Utc>,
    ) -> ProjectUsageResource {
        let egress_total_bytes = egress_rx_bytes.saturating_add(egress_tx_bytes);
        let alerts = self.usage_alerts(session_creations, runtime_usage_ms, egress_total_bytes);
        ProjectUsageResource {
            project_id: self.id,
            active_sessions,
            queued_sessions,
            session_creations,
            max_session_creations: self.quotas.max_session_creations,
            max_active_sessions: self.quotas.max_active_sessions,
            active_workflow_runs,
            max_active_workflow_runs: self.quotas.max_active_workflow_runs,
            runtime_usage_ms,
            max_runtime_usage_ms: self.quotas.max_runtime_usage_ms,
            egress_rx_bytes,
            egress_tx_bytes,
            egress_total_bytes,
            max_egress_total_bytes: self.quotas.max_egress_total_bytes,
            retained_storage_bytes,
            max_retained_storage_bytes: self.quotas.max_retained_storage_bytes,
            alerts,
            observed_at,
        }
    }

    fn usage_alerts(
        &self,
        session_creations: u32,
        runtime_usage_ms: u64,
        egress_total_bytes: u64,
    ) -> Vec<ProjectUsageAlertResource> {
        let mut alerts = Vec::new();
        push_usage_alert(
            &mut alerts,
            ProjectUsageAlertMetric::SessionCreations,
            u64::from(session_creations),
            self.quotas.max_session_creations.map(u64::from),
            "Project session creation count",
        );
        push_usage_alert(
            &mut alerts,
            ProjectUsageAlertMetric::RuntimeUsageMs,
            runtime_usage_ms,
            self.quotas.max_runtime_usage_ms,
            "Project browser runtime usage",
        );
        push_usage_alert(
            &mut alerts,
            ProjectUsageAlertMetric::EgressTotalBytes,
            egress_total_bytes,
            self.quotas.max_egress_total_bytes,
            "Project metadata-only egress byte counter",
        );
        alerts
    }

    pub fn to_resource(
        &self,
        active_sessions: u32,
        queued_sessions: u32,
        session_creations: u32,
        active_workflow_runs: u32,
        runtime_usage_ms: u64,
        egress_rx_bytes: u64,
        egress_tx_bytes: u64,
        retained_storage_bytes: u64,
        observed_at: DateTime<Utc>,
    ) -> ProjectResource {
        ProjectResource {
            id: self.id,
            name: self.name.clone(),
            description: self.description.clone(),
            labels: self.labels.clone(),
            quotas: self.quotas.clone(),
            policy: self.policy.clone(),
            state: self.state,
            usage: self.usage(
                active_sessions,
                queued_sessions,
                session_creations,
                active_workflow_runs,
                runtime_usage_ms,
                egress_rx_bytes,
                egress_tx_bytes,
                retained_storage_bytes,
                observed_at,
            ),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }

    pub fn to_session_project_resource(&self) -> SessionProjectResource {
        SessionProjectResource {
            id: self.id,
            name: self.name.clone(),
            state: self.state,
        }
    }
}

fn push_usage_alert(
    alerts: &mut Vec<ProjectUsageAlertResource>,
    metric: ProjectUsageAlertMetric,
    current_value: u64,
    limit_value: Option<u64>,
    label: &str,
) {
    let Some(limit_value) = limit_value else {
        return;
    };
    if limit_value == 0 {
        return;
    }
    let threshold = u64::from(StoredProject::USAGE_ALERT_THRESHOLD_PERCENT);
    let state = if current_value >= limit_value {
        ProjectUsageAlertState::Exceeded
    } else if u128::from(current_value) * 100 >= u128::from(limit_value) * u128::from(threshold) {
        ProjectUsageAlertState::ApproachingLimit
    } else {
        return;
    };
    let message = match state {
        ProjectUsageAlertState::ApproachingLimit => {
            format!("{label} has reached at least {threshold}% of the configured soft budget.")
        }
        ProjectUsageAlertState::Exceeded => {
            format!("{label} exceeded the configured soft budget.")
        }
    };
    alerts.push(ProjectUsageAlertResource {
        metric,
        state,
        current_value,
        limit_value,
        threshold_percent: match state {
            ProjectUsageAlertState::ApproachingLimit => {
                StoredProject::USAGE_ALERT_THRESHOLD_PERCENT
            }
            ProjectUsageAlertState::Exceeded => 100,
        },
        message,
    });
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EgressProxyConfig {
    pub url: String,
    #[serde(default)]
    pub credential_binding_id: Option<Uuid>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EgressCustomCaConfig {
    pub certificate_ref: String,
    #[serde(default)]
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EgressTrafficObservationConfig {
    #[serde(default)]
    pub mode: EgressTrafficObservationMode,
    #[serde(default)]
    pub sensitive_log_sink_ref: Option<String>,
    #[serde(default)]
    pub sensitive_log_sink_display_name: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EgressProfileEffectiveStatus {
    pub proxy_configured: bool,
    pub proxy_auth_configured: bool,
    pub bypass_rule_count: u32,
    pub custom_ca_configured: bool,
    pub observation_mode: EgressTrafficObservationMode,
    pub tls_interception_enabled: bool,
    pub sensitive_log_sink_configured: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EgressDiagnosticsHealth {
    Ready,
    Unknown,
    Attention,
    Blocked,
    Missing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EgressDiagnosticsProofLevel {
    None,
    Configuration,
    RuntimeLaunchMetadata,
    ActiveProbe,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EgressDiagnosticsProof {
    pub profile_resolved: bool,
    pub profile_ready: bool,
    pub profile_reachability_collected: bool,
    pub profile_reachability_healthy: bool,
    pub profile_reachability_observed_at: Option<DateTime<Utc>>,
    pub profile_reachability_failure: Option<String>,
    pub proxy_launch_config_expected: bool,
    pub bypass_rules_expected: u32,
    pub custom_ca_launch_config_expected: bool,
    pub tls_interception_expected: bool,
    pub sensitive_log_sink_declared: bool,
    pub runtime_launch_observed: bool,
    pub active_probe_collected: bool,
    pub observed_public_ip: Option<String>,
    pub observed_tls_issuer: Option<String>,
    pub last_failure_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EgressDiagnosticsResource {
    pub profile_id: Option<Uuid>,
    pub profile_name: Option<String>,
    pub profile_state: Option<EgressProfileState>,
    pub health: EgressDiagnosticsHealth,
    pub observation_mode: EgressTrafficObservationMode,
    pub proof_level: EgressDiagnosticsProofLevel,
    pub runtime_binding: Option<String>,
    pub runtime_assignment: Option<String>,
    pub proxy_configured: bool,
    pub proxy_auth_configured: bool,
    pub bypass_rule_count: u32,
    pub custom_ca_configured: bool,
    pub tls_interception_enabled: bool,
    pub sensitive_log_sink_configured: bool,
    pub proof: EgressDiagnosticsProof,
    pub warnings: Vec<String>,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistEgressDiagnosticsProbeResult {
    pub session_id: Uuid,
    pub profile_id: Option<Uuid>,
    pub active_probe_collected: bool,
    pub observed_public_ip: Option<String>,
    pub observed_tls_issuer: Option<String>,
    pub last_failure_reason: Option<String>,
    pub observed_at: DateTime<Utc>,
}

pub type StoredEgressDiagnosticsProbeResult = PersistEgressDiagnosticsProbeResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistEgressProfileReachabilityProbeResult {
    pub profile_id: Uuid,
    pub reachability_collected: bool,
    pub reachability_healthy: bool,
    pub last_failure_reason: Option<String>,
    pub observed_at: DateTime<Utc>,
}

pub type StoredEgressProfileReachabilityProbeResult = PersistEgressProfileReachabilityProbeResult;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SessionEffectiveEgress {
    pub profile_id: Option<Uuid>,
    pub profile_name: Option<String>,
    pub profile_state: Option<EgressProfileState>,
    pub proxy_configured: bool,
    pub proxy_auth_configured: bool,
    pub bypass_rule_count: u32,
    pub custom_ca_configured: bool,
    pub observation_mode: EgressTrafficObservationMode,
    pub tls_interception_enabled: bool,
    pub sensitive_log_sink_configured: bool,
}

#[derive(Debug, Clone)]
pub struct PersistEgressProfileRequest {
    pub name: String,
    pub description: Option<String>,
    pub labels: HashMap<String, String>,
    pub proxy: Option<EgressProxyConfig>,
    pub bypass_rules: Vec<String>,
    pub custom_ca: Option<EgressCustomCaConfig>,
    pub traffic_observation: EgressTrafficObservationConfig,
    pub state: EgressProfileState,
}

#[derive(Debug, Clone)]
pub struct StoredEgressProfile {
    pub id: Uuid,
    pub owner_subject: String,
    pub owner_issuer: String,
    pub name: String,
    pub description: Option<String>,
    pub labels: HashMap<String, String>,
    pub proxy: Option<EgressProxyConfig>,
    pub bypass_rules: Vec<String>,
    pub custom_ca: Option<EgressCustomCaConfig>,
    pub traffic_observation: EgressTrafficObservationConfig,
    pub state: EgressProfileState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EgressProfileResource {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub labels: HashMap<String, String>,
    pub proxy: Option<EgressProxyConfig>,
    pub bypass_rules: Vec<String>,
    pub custom_ca: Option<EgressCustomCaConfig>,
    pub traffic_observation: EgressTrafficObservationConfig,
    pub state: EgressProfileState,
    pub effective: EgressProfileEffectiveStatus,
    pub diagnostics: EgressDiagnosticsResource,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct EgressProfileListResponse {
    pub profiles: Vec<EgressProfileResource>,
}

impl StoredEgressProfile {
    pub fn effective_status(&self) -> EgressProfileEffectiveStatus {
        EgressProfileEffectiveStatus {
            proxy_configured: self.proxy.is_some(),
            proxy_auth_configured: self
                .proxy
                .as_ref()
                .and_then(|proxy| proxy.credential_binding_id)
                .is_some(),
            bypass_rule_count: self.bypass_rules.len() as u32,
            custom_ca_configured: self.custom_ca.is_some(),
            observation_mode: self.traffic_observation.mode,
            tls_interception_enabled: self.traffic_observation.mode
                == EgressTrafficObservationMode::TlsIntercept,
            sensitive_log_sink_configured: self
                .traffic_observation
                .sensitive_log_sink_ref
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty()),
        }
    }

    pub fn to_resource(&self) -> EgressProfileResource {
        EgressProfileResource {
            id: self.id,
            name: self.name.clone(),
            description: self.description.clone(),
            labels: self.labels.clone(),
            proxy: self.proxy.clone(),
            bypass_rules: self.bypass_rules.clone(),
            custom_ca: self.custom_ca.clone(),
            traffic_observation: self.traffic_observation.clone(),
            state: self.state,
            effective: self.effective_status(),
            diagnostics: self.to_diagnostics(None, None, Utc::now()),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }

    pub fn to_resource_with_reachability(
        &self,
        reachability: Option<&StoredEgressProfileReachabilityProbeResult>,
    ) -> EgressProfileResource {
        let mut resource = self.to_resource();
        resource.diagnostics = resource
            .diagnostics
            .with_profile_reachability_result(reachability);
        resource
    }

    pub fn to_session_effective_egress(&self) -> SessionEffectiveEgress {
        let effective = self.effective_status();
        SessionEffectiveEgress {
            profile_id: Some(self.id),
            profile_name: Some(self.name.clone()),
            profile_state: Some(self.state),
            proxy_configured: effective.proxy_configured,
            proxy_auth_configured: effective.proxy_auth_configured,
            bypass_rule_count: effective.bypass_rule_count,
            custom_ca_configured: effective.custom_ca_configured,
            observation_mode: effective.observation_mode,
            tls_interception_enabled: effective.tls_interception_enabled,
            sensitive_log_sink_configured: effective.sensitive_log_sink_configured,
        }
    }

    pub fn to_diagnostics(
        &self,
        runtime_binding: Option<String>,
        runtime_assignment: Option<String>,
        observed_at: DateTime<Utc>,
    ) -> EgressDiagnosticsResource {
        let effective = self.effective_status();
        let mut warnings = Vec::new();
        let session_scoped = runtime_binding.is_some() || runtime_assignment.is_some();
        let runtime_ready = runtime_assignment.as_deref() == Some("ready");
        let runtime_starting = runtime_assignment.as_deref() == Some("starting");

        if self.state == EgressProfileState::Disabled {
            warnings.push(
                "Egress profile is disabled and cannot be used as a healthy launch choice."
                    .to_string(),
            );
        }
        if effective.tls_interception_enabled {
            if !effective.proxy_configured {
                warnings.push("TLS interception requires a configured proxy.".to_string());
            }
            if !effective.custom_ca_configured {
                warnings.push(
                    "TLS interception requires a configured custom CA reference.".to_string(),
                );
            }
            if !effective.sensitive_log_sink_configured {
                warnings.push(
                    "TLS interception requires an approved sensitive log-sink reference."
                        .to_string(),
                );
            }
        }
        if session_scoped && self.state == EgressProfileState::Ready && !runtime_ready {
            if runtime_starting {
                warnings.push(
                    "Runtime launch is still starting; egress launch metadata is pending."
                        .to_string(),
                );
            } else {
                warnings.push(
                    "No active runtime launch metadata has been observed for this session yet."
                        .to_string(),
                );
            }
        }

        let has_configuration_gap = effective.tls_interception_enabled
            && (!effective.proxy_configured
                || !effective.custom_ca_configured
                || !effective.sensitive_log_sink_configured);
        let health = if self.state == EgressProfileState::Disabled {
            EgressDiagnosticsHealth::Blocked
        } else if has_configuration_gap {
            EgressDiagnosticsHealth::Attention
        } else if session_scoped && !runtime_ready {
            EgressDiagnosticsHealth::Unknown
        } else {
            EgressDiagnosticsHealth::Ready
        };
        let proof_level = if runtime_ready {
            EgressDiagnosticsProofLevel::RuntimeLaunchMetadata
        } else {
            EgressDiagnosticsProofLevel::Configuration
        };

        EgressDiagnosticsResource {
            profile_id: Some(self.id),
            profile_name: Some(self.name.clone()),
            profile_state: Some(self.state),
            health,
            observation_mode: effective.observation_mode,
            proof_level,
            runtime_binding,
            runtime_assignment,
            proxy_configured: effective.proxy_configured,
            proxy_auth_configured: effective.proxy_auth_configured,
            bypass_rule_count: effective.bypass_rule_count,
            custom_ca_configured: effective.custom_ca_configured,
            tls_interception_enabled: effective.tls_interception_enabled,
            sensitive_log_sink_configured: effective.sensitive_log_sink_configured,
            proof: EgressDiagnosticsProof {
                profile_resolved: true,
                profile_ready: self.state == EgressProfileState::Ready,
                profile_reachability_collected: false,
                profile_reachability_healthy: false,
                profile_reachability_observed_at: None,
                profile_reachability_failure: None,
                proxy_launch_config_expected: effective.proxy_configured,
                bypass_rules_expected: effective.bypass_rule_count,
                custom_ca_launch_config_expected: effective.custom_ca_configured
                    && effective.tls_interception_enabled,
                tls_interception_expected: effective.tls_interception_enabled,
                sensitive_log_sink_declared: effective.sensitive_log_sink_configured,
                runtime_launch_observed: runtime_ready,
                active_probe_collected: false,
                observed_public_ip: None,
                observed_tls_issuer: None,
                last_failure_reason: None,
            },
            warnings,
            observed_at,
        }
    }
}

impl EgressDiagnosticsResource {
    pub fn with_profile_reachability_result(
        mut self,
        result: Option<&StoredEgressProfileReachabilityProbeResult>,
    ) -> Self {
        let Some(result) = result else {
            return self;
        };
        if self.profile_id != Some(result.profile_id) {
            return self;
        }

        self.proof.profile_reachability_collected = result.reachability_collected;
        self.proof.profile_reachability_healthy = result.reachability_healthy;
        self.proof.profile_reachability_observed_at = Some(result.observed_at);
        self.proof.profile_reachability_failure = result.last_failure_reason.clone();
        if result.reachability_collected && result.reachability_healthy {
            if self.proof_level == EgressDiagnosticsProofLevel::Configuration {
                self.proof_level = EgressDiagnosticsProofLevel::ActiveProbe;
            }
            if !matches!(
                self.health,
                EgressDiagnosticsHealth::Blocked | EgressDiagnosticsHealth::Missing
            ) {
                self.health = EgressDiagnosticsHealth::Ready;
            }
        } else if let Some(reason) = result.last_failure_reason.as_deref() {
            if !matches!(
                self.health,
                EgressDiagnosticsHealth::Blocked | EgressDiagnosticsHealth::Missing
            ) {
                self.health = EgressDiagnosticsHealth::Attention;
            }
            self.warnings
                .push(format!("Last profile reachability probe failed: {reason}"));
        }
        self
    }

    pub fn with_probe_result(mut self, probe: Option<&StoredEgressDiagnosticsProbeResult>) -> Self {
        let Some(probe) = probe else {
            return self;
        };
        if probe.profile_id != self.profile_id {
            return self;
        }

        self.proof.active_probe_collected = probe.active_probe_collected;
        self.proof.observed_public_ip = probe.observed_public_ip.clone();
        self.proof.observed_tls_issuer = probe.observed_tls_issuer.clone();
        self.proof.last_failure_reason = probe.last_failure_reason.clone();
        if probe.active_probe_collected {
            self.proof_level = EgressDiagnosticsProofLevel::ActiveProbe;
            if !matches!(
                self.health,
                EgressDiagnosticsHealth::Blocked | EgressDiagnosticsHealth::Missing
            ) {
                self.health = EgressDiagnosticsHealth::Ready;
            }
        } else if let Some(reason) = probe.last_failure_reason.as_deref() {
            if !matches!(
                self.health,
                EgressDiagnosticsHealth::Blocked | EgressDiagnosticsHealth::Missing
            ) {
                self.health = EgressDiagnosticsHealth::Attention;
            }
            self.warnings
                .push(format!("Last active egress probe failed: {reason}"));
        }
        self
    }

    pub fn direct(
        runtime_binding: Option<String>,
        runtime_assignment: Option<String>,
        observed_at: DateTime<Utc>,
    ) -> Self {
        let runtime_ready = runtime_assignment.as_deref() == Some("ready");
        Self {
            profile_id: None,
            profile_name: None,
            profile_state: None,
            health: EgressDiagnosticsHealth::Ready,
            observation_mode: EgressTrafficObservationMode::MetadataOnly,
            proof_level: if runtime_ready {
                EgressDiagnosticsProofLevel::RuntimeLaunchMetadata
            } else {
                EgressDiagnosticsProofLevel::Configuration
            },
            runtime_binding,
            runtime_assignment,
            proxy_configured: false,
            proxy_auth_configured: false,
            bypass_rule_count: 0,
            custom_ca_configured: false,
            tls_interception_enabled: false,
            sensitive_log_sink_configured: false,
            proof: EgressDiagnosticsProof {
                profile_resolved: true,
                profile_ready: true,
                profile_reachability_collected: false,
                profile_reachability_healthy: false,
                profile_reachability_observed_at: None,
                profile_reachability_failure: None,
                proxy_launch_config_expected: false,
                bypass_rules_expected: 0,
                custom_ca_launch_config_expected: false,
                tls_interception_expected: false,
                sensitive_log_sink_declared: false,
                runtime_launch_observed: runtime_ready,
                active_probe_collected: false,
                observed_public_ip: None,
                observed_tls_issuer: None,
                last_failure_reason: None,
            },
            warnings: Vec::new(),
            observed_at,
        }
    }

    pub fn missing_profile(
        profile_id: Uuid,
        runtime_binding: Option<String>,
        runtime_assignment: Option<String>,
        observed_at: DateTime<Utc>,
    ) -> Self {
        Self {
            profile_id: Some(profile_id),
            profile_name: None,
            profile_state: None,
            health: EgressDiagnosticsHealth::Missing,
            observation_mode: EgressTrafficObservationMode::MetadataOnly,
            proof_level: EgressDiagnosticsProofLevel::None,
            runtime_binding,
            runtime_assignment,
            proxy_configured: false,
            proxy_auth_configured: false,
            bypass_rule_count: 0,
            custom_ca_configured: false,
            tls_interception_enabled: false,
            sensitive_log_sink_configured: false,
            proof: EgressDiagnosticsProof {
                profile_resolved: false,
                profile_ready: false,
                profile_reachability_collected: false,
                profile_reachability_healthy: false,
                profile_reachability_observed_at: None,
                profile_reachability_failure: None,
                proxy_launch_config_expected: false,
                bypass_rules_expected: 0,
                custom_ca_launch_config_expected: false,
                tls_interception_expected: false,
                sensitive_log_sink_declared: false,
                runtime_launch_observed: false,
                active_probe_collected: false,
                observed_public_ip: None,
                observed_tls_issuer: None,
                last_failure_reason: None,
            },
            warnings: vec!["Selected egress profile was not found for this owner.".to_string()],
            observed_at,
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
    pub project_id: Option<Uuid>,
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
    pub project_id: Option<Uuid>,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrowserContextResource {
    pub id: Uuid,
    pub project_id: Option<Uuid>,
    pub project: Option<SessionProjectResource>,
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

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SessionQueueInfo {
    pub queued_at: DateTime<Utc>,
    pub queued_for_ms: u64,
    pub position: u32,
    pub active_sessions: u32,
    pub queued_sessions: u32,
    pub max_active_sessions: Option<u32>,
    pub dispatch_blocker: String,
    pub cancellable: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SessionResource {
    pub id: Uuid,
    pub state: SessionLifecycleState,
    pub project_id: Option<Uuid>,
    pub project: Option<SessionProjectResource>,
    pub admission: ProjectAdmissionDecision,
    pub template_id: Option<String>,
    pub browser_context: SessionBrowserContextResource,
    pub network_identity: SessionNetworkIdentity,
    pub effective_egress: SessionEffectiveEgress,
    pub egress_diagnostics: EgressDiagnosticsResource,
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
    pub queue: Option<SessionQueueInfo>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub queued_at: Option<DateTime<Utc>>,
    pub runtime_released_at: Option<DateTime<Utc>>,
    pub stopped_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateSessionRequest {
    #[serde(default)]
    pub project_id: Option<Uuid>,
    #[serde(default)]
    pub template_id: Option<String>,
    #[serde(default)]
    pub browser_context: Option<SessionBrowserContextRequest>,
    #[serde(default)]
    pub network_identity: Option<SessionNetworkIdentity>,
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
    pub project_id: Option<Uuid>,
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
    pub network_identity: Option<SessionNetworkIdentity>,
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

#[derive(Debug, Clone, Deserialize)]
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
    pub project_id: Option<Uuid>,
    pub admission: ProjectAdmissionDecision,
    pub template_id: Option<String>,
    pub browser_context: SessionBrowserContextResource,
    pub network_identity: SessionNetworkIdentity,
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
    pub queued_at: Option<DateTime<Utc>>,
    pub runtime_started_at: Option<DateTime<Utc>>,
    pub runtime_usage_ms: u64,
    pub egress_rx_bytes: u64,
    pub egress_tx_bytes: u64,
    pub runtime_released_at: Option<DateTime<Utc>>,
    pub stopped_at: Option<DateTime<Utc>>,
}

impl StoredSession {
    pub fn to_resource(
        &self,
        public_gateway_url: &str,
        project: Option<SessionProjectResource>,
        runtime: SessionRuntimeInfo,
        status: SessionStatusSummary,
        queue: Option<SessionQueueInfo>,
        state_override: Option<SessionLifecycleState>,
        effective_egress: SessionEffectiveEgress,
        egress_diagnostics: EgressDiagnosticsResource,
    ) -> SessionResource {
        SessionResource {
            id: self.id,
            state: state_override.unwrap_or(self.state),
            project_id: self.project_id,
            project,
            admission: self.admission.clone(),
            template_id: self.template_id.clone(),
            browser_context: self.browser_context.clone(),
            network_identity: self.network_identity.clone(),
            effective_egress,
            egress_diagnostics,
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
            queue,
            created_at: self.created_at,
            updated_at: self.updated_at,
            queued_at: self.queued_at,
            runtime_released_at: self.runtime_released_at,
            stopped_at: self.stopped_at,
        }
    }
}

impl StoredBrowserContext {
    pub fn to_resource(&self) -> BrowserContextResource {
        BrowserContextResource {
            id: self.id,
            project_id: self.project_id,
            project: None,
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
