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
    RuntimeOperationResult, StorageHelperAction, StorageHelperRequest, VolumeLifecycleAction,
    VolumeLifecycleRequest, WorkflowWorkerCredentials, WorkflowWorkerLaunchRequest,
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
