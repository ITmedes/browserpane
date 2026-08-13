use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;
use uuid::Uuid;

use crate::SecretValue;

/// Version of the runtime broker wire contract.
#[derive(Debug, Clone, Copy, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BrokerApiVersion {
    /// Initial typed runtime broker contract.
    #[default]
    V1,
}

/// Stable operation families understood by the runtime broker.
#[derive(Debug, Clone, Copy, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeOperationKind {
    /// Long-lived browser session runtime.
    BrowserRuntime,
    /// Short-lived workflow execution worker.
    WorkflowWorker,
    /// Session recording worker.
    RecordingWorker,
    /// Network-disabled storage helper.
    StorageHelper,
}

/// A bounded idempotency key supplied by the gateway.
#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct IdempotencyKey(String);

impl IdempotencyKey {
    /// Creates an idempotency key containing only safe token characters.
    ///
    /// # Errors
    ///
    /// Returns an error for empty, oversized, or unsafe values.
    pub fn new(value: impl Into<String>) -> Result<Self, IdempotencyKeyError> {
        let value = value.into();
        if value.is_empty() {
            return Err(IdempotencyKeyError::Empty);
        }
        if value.len() > 128 {
            return Err(IdempotencyKeyError::TooLarge);
        }
        if !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
        {
            return Err(IdempotencyKeyError::UnsafeCharacter);
        }
        Ok(Self(value))
    }

    /// Returns the validated key.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for IdempotencyKey {
    type Error = IdempotencyKeyError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<IdempotencyKey> for String {
    fn from(value: IdempotencyKey) -> Self {
        value.0
    }
}

/// Validation errors for [`IdempotencyKey`].
#[derive(Debug, Clone, Copy, Eq, Error, PartialEq)]
pub enum IdempotencyKeyError {
    /// Empty keys are invalid.
    #[error("idempotency key must not be empty")]
    Empty,
    /// Keys longer than 128 bytes are invalid.
    #[error("idempotency key exceeds 128 bytes")]
    TooLarge,
    /// Keys may contain only token-safe ASCII characters.
    #[error("idempotency key contains an unsafe character")]
    UnsafeCharacter,
}

/// Stable semantic-validation codes for runtime operation requests.
#[derive(Debug, Clone, Copy, Deserialize, Eq, Error, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractErrorCode {
    /// A required BrowserPane resource identifier is nil.
    #[error("runtime operation contains an invalid resource identifier")]
    InvalidResourceId,
    /// Fields do not form a valid request for the selected operation.
    #[error("runtime operation parameters are invalid")]
    InvalidOperationParameters,
    /// An inbound payload size must be declared for the selected operation.
    #[error("runtime operation requires a positive payload declaration")]
    PayloadDeclarationRequired,
    /// The selected operation does not accept an inbound payload.
    #[error("runtime operation does not accept a payload declaration")]
    PayloadDeclarationNotAllowed,
}

/// A sanitized runtime operation contract violation.
#[derive(Debug, Clone, Copy, Eq, Error, PartialEq)]
#[error("runtime operation contract rejected the request: {code}")]
pub struct ContractViolation {
    /// Stable code. Submitted values are intentionally omitted.
    pub code: ContractErrorCode,
}

impl From<ContractErrorCode> for ContractViolation {
    fn from(code: ContractErrorCode) -> Self {
        Self { code }
    }
}

/// Versioned request envelope sent from the gateway to the broker.
#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeOperationRequest {
    /// Wire contract version.
    pub api_version: BrokerApiVersion,
    /// Unique request correlation identifier.
    pub request_id: Uuid,
    /// Retry-safe operation identifier.
    pub idempotency_key: IdempotencyKey,
    /// Typed BrowserPane operation.
    pub operation: RuntimeOperation,
}

impl RuntimeOperationRequest {
    /// Validates semantic invariants that cannot be expressed by JSON shape.
    ///
    /// # Errors
    ///
    /// Returns a sanitized violation for nil identifiers, invalid field
    /// combinations, or missing and unexpected payload declarations.
    pub fn validate(&self) -> Result<(), ContractViolation> {
        if self.request_id.is_nil() {
            return Err(ContractErrorCode::InvalidResourceId.into());
        }
        self.operation.validate()
    }
}

/// Typed operation accepted by the broker.
#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "parameters", rename_all = "snake_case")]
pub enum RuntimeOperation {
    /// Launch a browser runtime.
    LaunchBrowser(BrowserRuntimeLaunchRequest),
    /// Launch a workflow worker.
    LaunchWorkflow(WorkflowWorkerLaunchRequest),
    /// Launch a recording worker.
    LaunchRecording(RecordingWorkerLaunchRequest),
    /// Run an approved storage helper operation.
    RunStorageHelper(StorageHelperRequest),
    /// Inspect, stop, or remove an owned container.
    ContainerLifecycle(ContainerLifecycleRequest),
    /// Inspect or remove an owned volume.
    VolumeLifecycle(VolumeLifecycleRequest),
}

impl RuntimeOperation {
    /// Returns the policy family for this operation.
    pub fn kind(&self) -> RuntimeOperationKind {
        match self {
            Self::LaunchBrowser(_) => RuntimeOperationKind::BrowserRuntime,
            Self::LaunchWorkflow(_) => RuntimeOperationKind::WorkflowWorker,
            Self::LaunchRecording(_) => RuntimeOperationKind::RecordingWorker,
            Self::RunStorageHelper(_) => RuntimeOperationKind::StorageHelper,
            Self::ContainerLifecycle(request) => request.operation_kind,
            Self::VolumeLifecycle(request) => request.operation_kind,
        }
    }

    /// Returns the primary BrowserPane resource correlated with this operation.
    pub fn resource_id(&self) -> Uuid {
        match self {
            Self::LaunchBrowser(request) => request.session_id,
            Self::LaunchWorkflow(request) => request.workflow_run_id,
            Self::LaunchRecording(request) => request.recording_id,
            Self::RunStorageHelper(request) => request
                .session_id
                .or(request.target_context_id)
                .or(request.source_context_id)
                .unwrap_or(Uuid::nil()),
            Self::ContainerLifecycle(request) => request.resource_id,
            Self::VolumeLifecycle(request) => request.resource_id,
        }
    }

    fn validate(&self) -> Result<(), ContractViolation> {
        match self {
            Self::LaunchBrowser(request) => {
                require_ids([request.session_id])?;
                require_optional_ids([request.browser_context_id])?;
                request.features.validate()
            }
            Self::LaunchWorkflow(request) => require_ids([
                request.workflow_run_id,
                request.session_id,
                request.automation_task_id,
            ]),
            Self::LaunchRecording(request) => {
                require_ids([request.session_id, request.recording_id])
            }
            Self::RunStorageHelper(request) => request.validate(),
            Self::ContainerLifecycle(request) => require_ids([request.resource_id]),
            Self::VolumeLifecycle(request) => require_ids([request.resource_id]),
        }
    }
}

/// Browser runtime launch intent. Docker-sensitive fields are broker-owned.
#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserRuntimeLaunchRequest {
    /// Session that owns the runtime.
    pub session_id: Uuid,
    /// Optional reusable browser context mounted by broker policy.
    pub browser_context_id: Option<Uuid>,
    /// Approved browser features materialized by broker policy.
    #[serde(default, skip_serializing_if = "BrowserRuntimeFeatures::is_empty")]
    pub features: BrowserRuntimeFeatures,
}

/// Bounded browser feature selections. Runtime paths and Docker fields are
/// intentionally absent.
#[derive(Debug, Clone, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserRuntimeFeatures {
    /// Browser-visible locale, timezone, geolocation, and identity settings.
    #[serde(default, skip_serializing_if = "BrowserNetworkIdentity::is_empty")]
    pub network_identity: BrowserNetworkIdentity,
    /// Optional approved egress behavior.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub egress: Option<BrowserEgressSelection>,
    /// Approved extension versions resolved by trusted broker configuration.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extension_version_ids: Vec<Uuid>,
    /// Whether a session-file manifest has been prepared in session data.
    #[serde(default, skip_serializing_if = "is_false")]
    pub session_file_bindings: bool,
}

impl BrowserRuntimeFeatures {
    fn is_empty(&self) -> bool {
        self.network_identity.is_empty()
            && self.egress.is_none()
            && self.extension_version_ids.is_empty()
            && !self.session_file_bindings
    }

    fn validate(&self) -> Result<(), ContractViolation> {
        self.network_identity.validate()?;
        if let Some(egress) = &self.egress {
            egress.validate()?;
        }
        if self.extension_version_ids.len() > 32 {
            return Err(ContractErrorCode::InvalidOperationParameters.into());
        }
        let mut unique_ids = BTreeSet::new();
        for extension_id in &self.extension_version_ids {
            if extension_id.is_nil() || !unique_ids.insert(*extension_id) {
                return Err(ContractErrorCode::InvalidOperationParameters.into());
            }
        }
        Ok(())
    }
}

/// Browser-visible network identity values.
#[derive(Debug, Clone, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserNetworkIdentity {
    /// BCP 47-like locale used by Chromium and the POSIX environment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    /// Ordered browser language preferences.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub languages: Vec<String>,
    /// IANA timezone identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    /// Optional fixed-point browser geolocation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geolocation: Option<BrowserGeolocation>,
    /// Optional Chromium user agent override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
    /// Optional application-defined browser identity label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browser_identity: Option<String>,
}

impl BrowserNetworkIdentity {
    fn is_empty(&self) -> bool {
        self.locale.is_none()
            && self.languages.is_empty()
            && self.timezone.is_none()
            && self.geolocation.is_none()
            && self.user_agent.is_none()
            && self.browser_identity.is_none()
    }

    fn validate(&self) -> Result<(), ContractViolation> {
        if self
            .locale
            .as_deref()
            .is_some_and(|value| !is_locale(value))
            || self
                .timezone
                .as_deref()
                .is_some_and(|value| !is_timezone(value))
        {
            return Err(ContractErrorCode::InvalidOperationParameters.into());
        }
        validate_optional_text(&self.user_agent, 1_024)?;
        validate_optional_text(&self.browser_identity, 128)?;
        if self.languages.len() > 16 {
            return Err(ContractErrorCode::InvalidOperationParameters.into());
        }
        for language in &self.languages {
            if !is_locale(language) {
                return Err(ContractErrorCode::InvalidOperationParameters.into());
            }
        }
        if let Some(geolocation) = &self.geolocation {
            geolocation.validate()?;
        }
        Ok(())
    }
}

/// Fixed-point browser geolocation, avoiding floating-point ambiguity in
/// idempotency comparisons.
#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserGeolocation {
    /// Latitude multiplied by 10,000,000.
    pub latitude_e7: i32,
    /// Longitude multiplied by 10,000,000.
    pub longitude_e7: i32,
    /// Optional accuracy in millimeters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accuracy_mm: Option<u32>,
}

impl BrowserGeolocation {
    fn validate(&self) -> Result<(), ContractViolation> {
        if !(-900_000_000..=900_000_000).contains(&self.latitude_e7)
            || !(-1_800_000_000..=1_800_000_000).contains(&self.longitude_e7)
            || self
                .accuracy_mm
                .is_some_and(|value| value == 0 || value > 100_000_000)
        {
            return Err(ContractErrorCode::InvalidOperationParameters.into());
        }
        Ok(())
    }
}

/// Egress traffic observation mode selected for a browser runtime.
#[derive(Debug, Clone, Copy, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserEgressObservationMode {
    /// Proxy metadata may be observed without decrypting HTTPS traffic.
    #[default]
    MetadataOnly,
    /// HTTPS traffic is intercepted by an explicitly configured proxy.
    TlsIntercept,
}

impl BrowserEgressObservationMode {
    /// Stable value used in broker-derived environment and labels.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MetadataOnly => "metadata_only",
            Self::TlsIntercept => "tls_intercept",
        }
    }
}

/// Marker for sensitive material prepared at a broker-owned session-data path.
#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserSessionDataSource {
    /// The later typed storage operation prepares the material in session data.
    SessionData,
}

/// Approved proxy selection without embedded credentials.
#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserProxySelection {
    /// Proxy URL without user information.
    pub url: String,
    /// Optional fixed-path proxy-auth material prerequisite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authentication: Option<BrowserSessionDataSource>,
}

/// Approved browser egress behavior.
#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserEgressSelection {
    /// Control-plane egress profile correlated with the runtime.
    pub profile_id: Uuid,
    /// Optional proxy used by Chromium.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy: Option<BrowserProxySelection>,
    /// Chromium proxy bypass rules.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bypass_rules: Vec<String>,
    /// Traffic observation mode.
    #[serde(default)]
    pub observation_mode: BrowserEgressObservationMode,
    /// Optional fixed-path trusted CA prerequisite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_ca: Option<BrowserSessionDataSource>,
    /// Whether an approved sensitive-log sink exists outside BrowserPane.
    #[serde(default, skip_serializing_if = "is_false")]
    pub sensitive_log_sink_configured: bool,
}

impl BrowserEgressSelection {
    fn validate(&self) -> Result<(), ContractViolation> {
        if self.profile_id.is_nil() || self.bypass_rules.len() > 128 {
            return Err(ContractErrorCode::InvalidOperationParameters.into());
        }
        for rule in &self.bypass_rules {
            validate_text(rule, 512)?;
            if rule.contains(';') {
                return Err(ContractErrorCode::InvalidOperationParameters.into());
            }
        }
        if let Some(proxy) = &self.proxy {
            validate_proxy_url(&proxy.url)?;
        }
        let intercepting = self.observation_mode == BrowserEgressObservationMode::TlsIntercept;
        if (intercepting
            && (self.proxy.is_none()
                || self.custom_ca.is_none()
                || !self.sensitive_log_sink_configured))
            || (!intercepting && self.custom_ca.is_some())
        {
            return Err(ContractErrorCode::InvalidOperationParameters.into());
        }
        Ok(())
    }
}

/// Workflow worker launch intent.
#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowWorkerLaunchRequest {
    /// Workflow run executed by the worker.
    pub workflow_run_id: Uuid,
    /// Session controlled by the workflow.
    pub session_id: Uuid,
    /// Automation task correlated with the run.
    pub automation_task_id: Uuid,
    /// Purpose-scoped worker credentials.
    pub credentials: WorkflowWorkerCredentials,
}

/// Purpose-scoped workflow worker secrets.
#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowWorkerCredentials {
    /// Session automation credential.
    pub session_automation_access_token: SecretValue,
    /// Optional gateway service credential.
    pub gateway_bearer_token: Option<SecretValue>,
}

impl std::fmt::Debug for WorkflowWorkerCredentials {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WorkflowWorkerCredentials")
            .field("session_automation_access_token", &"[REDACTED]")
            .field(
                "gateway_bearer_token",
                &self.gateway_bearer_token.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

/// Recording worker launch intent.
#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecordingWorkerLaunchRequest {
    /// Session being recorded.
    pub session_id: Uuid,
    /// Recording segment produced by the worker.
    pub recording_id: Uuid,
    /// Purpose-scoped worker credentials.
    pub credentials: RecordingWorkerCredentials,
}

/// Purpose-scoped recording worker secrets.
#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecordingWorkerCredentials {
    /// Short-lived WebTransport connect ticket.
    pub connect_ticket: SecretValue,
    /// Session automation credential.
    pub session_automation_access_token: SecretValue,
    /// Recording completion credential.
    pub recording_worker_access_token: SecretValue,
}

impl std::fmt::Debug for RecordingWorkerCredentials {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RecordingWorkerCredentials")
            .field("connect_ticket", &"[REDACTED]")
            .field("session_automation_access_token", &"[REDACTED]")
            .field("recording_worker_access_token", &"[REDACTED]")
            .finish()
    }
}

/// Approved storage helper actions.
#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageHelperAction {
    /// Initialize session-owned data storage.
    InitializeSessionData,
    /// Materialize approved session files.
    MaterializeSessionFiles,
    /// Clone one inactive browser context into another.
    CloneBrowserContext,
    /// Export an inactive browser context.
    ExportBrowserContext,
    /// Import a browser context archive.
    ImportBrowserContext,
    /// Measure browser context storage usage.
    MeasureBrowserContext,
    /// Delete inactive browser context data.
    DeleteBrowserContext,
}

/// Storage helper intent. Payload bytes are carried by a separately bounded stream.
#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StorageHelperRequest {
    /// Approved helper action.
    pub action: StorageHelperAction,
    /// Optional owning session.
    pub session_id: Option<Uuid>,
    /// Optional source browser context.
    pub source_context_id: Option<Uuid>,
    /// Optional target browser context.
    pub target_context_id: Option<Uuid>,
    /// Declared stream size for bounded transfer admission.
    pub declared_payload_bytes: Option<u64>,
}

impl StorageHelperRequest {
    fn validate(&self) -> Result<(), ContractViolation> {
        require_optional_ids([
            self.session_id,
            self.source_context_id,
            self.target_context_id,
        ])?;
        match self.action {
            StorageHelperAction::InitializeSessionData => {
                require_storage_fields(self, true, false, false)?;
                reject_payload(self.declared_payload_bytes)
            }
            StorageHelperAction::MaterializeSessionFiles => {
                require_storage_fields(self, true, false, false)?;
                require_payload(self.declared_payload_bytes)
            }
            StorageHelperAction::CloneBrowserContext => {
                require_storage_fields(self, false, true, true)?;
                if self.source_context_id == self.target_context_id {
                    return Err(ContractErrorCode::InvalidOperationParameters.into());
                }
                reject_payload(self.declared_payload_bytes)
            }
            StorageHelperAction::ExportBrowserContext
            | StorageHelperAction::MeasureBrowserContext
            | StorageHelperAction::DeleteBrowserContext => {
                require_storage_fields(self, false, true, false)?;
                reject_payload(self.declared_payload_bytes)
            }
            StorageHelperAction::ImportBrowserContext => {
                require_storage_fields(self, false, false, true)?;
                require_payload(self.declared_payload_bytes)
            }
        }
    }
}

fn require_ids<const N: usize>(ids: [Uuid; N]) -> Result<(), ContractViolation> {
    if ids.into_iter().any(|id| id.is_nil()) {
        return Err(ContractErrorCode::InvalidResourceId.into());
    }
    Ok(())
}

fn require_optional_ids<const N: usize>(ids: [Option<Uuid>; N]) -> Result<(), ContractViolation> {
    if ids.into_iter().flatten().any(|id| id.is_nil()) {
        return Err(ContractErrorCode::InvalidResourceId.into());
    }
    Ok(())
}

fn require_storage_fields(
    request: &StorageHelperRequest,
    session: bool,
    source: bool,
    target: bool,
) -> Result<(), ContractViolation> {
    let valid = request.session_id.is_some() == session
        && request.source_context_id.is_some() == source
        && request.target_context_id.is_some() == target;
    if !valid {
        return Err(ContractErrorCode::InvalidOperationParameters.into());
    }
    Ok(())
}

fn require_payload(payload: Option<u64>) -> Result<(), ContractViolation> {
    if payload.is_none_or(|bytes| bytes == 0) {
        return Err(ContractErrorCode::PayloadDeclarationRequired.into());
    }
    Ok(())
}

fn reject_payload(payload: Option<u64>) -> Result<(), ContractViolation> {
    if payload.is_some() {
        return Err(ContractErrorCode::PayloadDeclarationNotAllowed.into());
    }
    Ok(())
}

fn validate_optional_text(
    value: &Option<String>,
    maximum_bytes: usize,
) -> Result<(), ContractViolation> {
    if let Some(value) = value {
        validate_text(value, maximum_bytes)?;
    }
    Ok(())
}

fn validate_text(value: &str, maximum_bytes: usize) -> Result<(), ContractViolation> {
    if value.is_empty()
        || value.len() > maximum_bytes
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(ContractErrorCode::InvalidOperationParameters.into());
    }
    Ok(())
}

fn validate_proxy_url(value: &str) -> Result<(), ContractViolation> {
    validate_text(value, 2_048)?;
    if value.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return Err(ContractErrorCode::InvalidOperationParameters.into());
    }
    let parsed = Url::parse(value)
        .map_err(|_| ContractViolation::from(ContractErrorCode::InvalidOperationParameters))?;
    let valid_scheme = matches!(parsed.scheme(), "http" | "https");
    let valid_path = parsed.path().is_empty() || parsed.path() == "/";
    if !valid_scheme
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || !valid_path
    {
        return Err(ContractErrorCode::InvalidOperationParameters.into());
    }
    Ok(())
}

fn is_false(value: &bool) -> bool {
    !value
}

fn is_locale(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .split('-')
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_alphanumeric()))
}

fn is_timezone(value: &str) -> bool {
    value == "UTC"
        || (!value.is_empty()
            && value.len() <= 128
            && !value.starts_with('/')
            && !value.ends_with('/')
            && !value.contains("..")
            && value.contains('/')
            && value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'_' | b'-' | b'+')
            }))
}

/// Owned-container lifecycle actions.
#[derive(Debug, Clone, Copy, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContainerLifecycleAction {
    /// Inspect existence/readiness metadata.
    Inspect,
    /// Request bounded graceful stop.
    Stop,
    /// Force-remove after policy and ownership validation.
    Remove,
}

/// Lifecycle request for one BrowserPane-owned container.
#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContainerLifecycleRequest {
    /// Product family that owns the container.
    pub operation_kind: RuntimeOperationKind,
    /// BrowserPane resource identifier encoded in the owned name.
    pub resource_id: Uuid,
    /// Requested lifecycle action.
    pub action: ContainerLifecycleAction,
}

/// Owned-volume lifecycle actions.
#[derive(Debug, Clone, Copy, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VolumeLifecycleAction {
    /// Inspect owned volume metadata.
    Inspect,
    /// Remove an owned inactive volume.
    Remove,
}

/// Lifecycle request for one BrowserPane-owned volume.
#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VolumeLifecycleRequest {
    /// Product family that owns the volume.
    pub operation_kind: RuntimeOperationKind,
    /// BrowserPane resource identifier encoded in the owned name.
    pub resource_id: Uuid,
    /// Requested lifecycle action.
    pub action: VolumeLifecycleAction,
}

/// Versioned response envelope returned by the broker.
#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeOperationResponse {
    /// Wire contract version.
    pub api_version: BrokerApiVersion,
    /// Correlated request identifier.
    pub request_id: Uuid,
    /// Sanitized operation result.
    pub result: RuntimeOperationResult,
}

/// Sanitized operation results. Docker response models are never exposed.
#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum RuntimeOperationResult {
    /// An owned runtime or worker was accepted.
    Accepted,
    /// An owned resource exists.
    Exists,
    /// An owned resource does not exist.
    Absent,
    /// An owned resource reached its terminal state.
    Completed {
        /// Process exit code when available.
        exit_code: Option<i32>,
        /// Number of omitted output bytes after bounding.
        omitted_output_bytes: u64,
    },
}

#[cfg(test)]
mod tests;
