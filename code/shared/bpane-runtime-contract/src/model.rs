use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::SecretValue;

mod browser_features;

pub use browser_features::{
    BrowserEgressObservationMode, BrowserEgressSelection, BrowserGeolocation,
    BrowserNetworkIdentity, BrowserProxySelection, BrowserRuntimeFeatures,
    BrowserRuntimeLaunchRequest, BrowserSessionDataSource,
};

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
    /// Optional gateway service credential.
    pub gateway_bearer_token: Option<SecretValue>,
}

impl std::fmt::Debug for RecordingWorkerCredentials {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RecordingWorkerCredentials")
            .field("connect_ticket", &"[REDACTED]")
            .field("session_automation_access_token", &"[REDACTED]")
            .field("recording_worker_access_token", &"[REDACTED]")
            .field(
                "gateway_bearer_token",
                &self.gateway_bearer_token.as_ref().map(|_| "[REDACTED]"),
            )
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
    /// Delete session-owned data storage after runtime release.
    DeleteSessionData,
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

impl StorageHelperAction {
    /// Returns whether this action consumes a binary request payload.
    pub const fn accepts_input_payload(self) -> bool {
        matches!(
            self,
            Self::MaterializeSessionFiles | Self::ImportBrowserContext
        )
    }

    /// Returns whether this action produces a binary response payload.
    pub const fn produces_output_payload(self) -> bool {
        matches!(self, Self::ExportBrowserContext)
    }
}

/// Broker-derived destination for one session-data payload.
#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "purpose", rename_all = "snake_case", deny_unknown_fields)]
pub enum SessionDataFileTarget {
    /// A workspace file bound below the fixed session mounts root.
    SessionBinding {
        /// Safe relative path below the broker-owned mounts root.
        relative_path: String,
        /// Whether the browser runtime may modify the materialized file.
        writable: bool,
    },
    /// Broker-owned session binding manifest at its fixed path.
    SessionBindingManifest,
    /// Broker-owned proxy-authentication material at its fixed path.
    EgressProxyAuthentication,
    /// Broker-owned trusted CA material at its fixed path.
    EgressTrustedCa,
}

impl SessionDataFileTarget {
    fn validate(&self) -> Result<(), ContractViolation> {
        let Self::SessionBinding { relative_path, .. } = self else {
            return Ok(());
        };
        if !is_safe_relative_path(relative_path) {
            return Err(ContractErrorCode::InvalidOperationParameters.into());
        }
        Ok(())
    }
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
    /// Typed broker-derived destination for session-data writes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_target: Option<SessionDataFileTarget>,
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
                if self.session_id.is_none()
                    || self.source_context_id.is_some()
                    || self.file_target.is_some()
                {
                    return Err(ContractErrorCode::InvalidOperationParameters.into());
                }
                reject_payload(self.declared_payload_bytes)
            }
            StorageHelperAction::MaterializeSessionFiles => {
                require_storage_fields(self, true, false, false)?;
                self.file_target
                    .as_ref()
                    .ok_or(ContractErrorCode::InvalidOperationParameters)?
                    .validate()?;
                require_payload_declaration(self.declared_payload_bytes)
            }
            StorageHelperAction::DeleteSessionData => {
                require_storage_fields(self, true, false, false)?;
                reject_payload(self.declared_payload_bytes)
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
        }?;
        if self.action != StorageHelperAction::MaterializeSessionFiles && self.file_target.is_some()
        {
            return Err(ContractErrorCode::InvalidOperationParameters.into());
        }
        Ok(())
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

fn require_payload_declaration(payload: Option<u64>) -> Result<(), ContractViolation> {
    if payload.is_none() {
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

fn is_safe_relative_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 1024
        && !value.starts_with('/')
        && !value.ends_with('/')
        && !value.contains('\\')
        && !value
            .bytes()
            .any(|byte| byte == 0 || byte.is_ascii_control())
        && value
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
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

/// Sanitized execution state for a detached broker-owned worker.
#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerExecutionState {
    /// The worker container is created, starting, running, or stopping.
    Running,
    /// The worker container reached a terminal process state.
    Exited,
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
    /// A detached workflow or recording worker was inspected.
    WorkerState {
        /// Stable worker lifecycle state.
        execution_state: WorkerExecutionState,
        /// Process exit code for an exited worker when the backend provides it.
        exit_code: Option<i32>,
    },
    /// An owned resource reached its terminal state.
    Completed {
        /// Process exit code when available.
        exit_code: Option<i32>,
        /// Number of omitted output bytes after bounding.
        omitted_output_bytes: u64,
    },
    /// A storage helper measured one owned resource.
    StorageUsage {
        /// Exact storage bytes reported by the helper.
        storage_bytes: u64,
    },
    /// A storage helper produced a separately transferred binary payload.
    StoragePayload {
        /// Exact payload size carried by the response body.
        payload_bytes: u64,
        /// Lowercase SHA-256 digest of the response body.
        sha256_hex: String,
    },
}

#[cfg(test)]
mod tests;
