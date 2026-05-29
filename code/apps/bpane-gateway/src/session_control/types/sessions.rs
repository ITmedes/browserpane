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
    ProjectArchived,
}

impl ProjectAdmissionReasonCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OwnerScopeUnbounded => "owner_scope_unbounded",
            Self::ProjectQuotaAvailable => "project_quota_available",
            Self::ActiveSessionQuotaExceeded => "active_session_quota_exceeded",
            Self::ProjectArchived => "project_archived",
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
    pub state: ProjectState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectUsageResource {
    pub project_id: Uuid,
    pub active_sessions: u32,
    pub max_active_sessions: Option<u32>,
    pub active_workflow_runs: u32,
    pub max_active_workflow_runs: Option<u32>,
    pub retained_storage_bytes: u64,
    pub max_retained_storage_bytes: Option<u64>,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectResource {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub labels: HashMap<String, String>,
    pub quotas: ProjectQuotas,
    pub state: ProjectState,
    pub usage: ProjectUsageResource,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SessionProjectResource {
    pub id: Uuid,
    pub name: String,
    pub state: ProjectState,
}

#[derive(Debug, Serialize)]
pub struct ProjectListResponse {
    pub projects: Vec<ProjectResource>,
}

impl StoredProject {
    pub fn usage(&self, active_sessions: u32, observed_at: DateTime<Utc>) -> ProjectUsageResource {
        ProjectUsageResource {
            project_id: self.id,
            active_sessions,
            max_active_sessions: self.quotas.max_active_sessions,
            active_workflow_runs: 0,
            max_active_workflow_runs: self.quotas.max_active_workflow_runs,
            retained_storage_bytes: 0,
            max_retained_storage_bytes: self.quotas.max_retained_storage_bytes,
            observed_at,
        }
    }

    pub fn to_resource(&self, active_sessions: u32, observed_at: DateTime<Utc>) -> ProjectResource {
        ProjectResource {
            id: self.id,
            name: self.name.clone(),
            description: self.description.clone(),
            labels: self.labels.clone(),
            quotas: self.quotas.clone(),
            state: self.state,
            usage: self.usage(active_sessions, observed_at),
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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
