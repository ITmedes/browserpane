//! Typed operations and deny-by-default policy for the BrowserPane runtime broker.
//!
//! The public gateway describes BrowserPane product operations through this
//! contract. Docker-specific request models and command arguments are kept out
//! of the wire protocol and are assembled only by a trusted broker adapter.

#![forbid(unsafe_code)]

mod audit;
mod model;
mod policy;
mod secret;

pub use audit::{
    AuditOutcome, AuditResource, RuntimeBrokerAuditEvent, RuntimeBrokerAuditEventBuilder,
};
pub use model::{
    BrokerApiVersion, BrowserEgressObservationMode, BrowserEgressSelection, BrowserGeolocation,
    BrowserNetworkIdentity, BrowserProxySelection, BrowserRuntimeFeatures,
    BrowserRuntimeLaunchRequest, BrowserSessionDataSource, ContainerLifecycleAction,
    ContainerLifecycleRequest, ContractErrorCode, ContractViolation, IdempotencyKey,
    RecordingWorkerCredentials, RecordingWorkerLaunchRequest, RuntimeOperation,
    RuntimeOperationKind, RuntimeOperationRequest, RuntimeOperationResponse,
    RuntimeOperationResult, SessionDataFileTarget, StorageHelperAction, StorageHelperRequest,
    VolumeLifecycleAction, VolumeLifecycleRequest, WorkerExecutionState, WorkflowWorkerCredentials,
    WorkflowWorkerLaunchRequest,
};
pub use policy::{
    ContainerLaunchPolicy, ContainerLaunchSpec, ContainerMount, ContainerPolicyConfig,
    ContainerSecurity, LifecyclePolicy, MountSource, OwnedContainerTarget, OwnedVolumeTarget,
    PolicyConfigurationError, PolicyConfigurationErrorCode, PolicyErrorCode, PolicyViolation,
    ResourceLimits, RuntimeBrokerPolicy,
};
pub use secret::{SecretValue, SecretValueError};

/// Stable media type for the version-one broker JSON contract.
pub const RUNTIME_BROKER_V1_MEDIA_TYPE: &str = "application/vnd.browserpane.runtime-broker.v1+json";

/// Stable media type for binary storage-helper payloads.
pub const RUNTIME_BROKER_STORAGE_PAYLOAD_MEDIA_TYPE: &str = "application/octet-stream";

/// Response header carrying the broker contract version for binary payloads.
pub const RUNTIME_BROKER_API_VERSION_HEADER: &str = "x-bpane-api-version";

/// Response header correlating a binary payload with its operation request.
pub const RUNTIME_BROKER_REQUEST_ID_HEADER: &str = "x-bpane-request-id";

/// Response header declaring the exact binary payload size.
pub const RUNTIME_BROKER_PAYLOAD_BYTES_HEADER: &str = "x-bpane-payload-bytes";

/// Response header carrying the lowercase SHA-256 binary payload digest.
pub const RUNTIME_BROKER_PAYLOAD_SHA256_HEADER: &str = "x-bpane-payload-sha256";
