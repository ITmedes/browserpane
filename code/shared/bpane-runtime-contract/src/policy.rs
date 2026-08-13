use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::{ContainerLifecycleAction, RuntimeOperationKind, VolumeLifecycleAction};

/// Stable policy denial codes safe to return and audit.
#[derive(Debug, Clone, Copy, Deserialize, Eq, Error, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyErrorCode {
    /// No policy exists for the requested operation family.
    #[error("operation family is not enabled")]
    OperationNotAllowed,
    /// The selected image is not approved.
    #[error("runtime image is not approved")]
    ImageNotAllowed,
    /// The container name does not match broker ownership rules.
    #[error("container name is not owned by this operation")]
    ContainerNameNotOwned,
    /// The network is not approved for the operation.
    #[error("runtime network is not approved")]
    NetworkNotAllowed,
    /// A mount source or target is not approved.
    #[error("runtime mount is not approved")]
    MountNotAllowed,
    /// An environment key is not approved.
    #[error("runtime environment key is not approved")]
    EnvironmentNotAllowed,
    /// Runtime ownership labels are absent or unexpected.
    #[error("runtime ownership labels are invalid")]
    LabelsInvalid,
    /// The entrypoint does not match the operation policy.
    #[error("runtime entrypoint is not approved")]
    EntrypointNotAllowed,
    /// Privilege, capability, device, or namespace settings are unsafe.
    #[error("runtime security settings are not approved")]
    SecuritySettingsNotAllowed,
    /// A resource or timeout limit is zero or exceeds policy.
    #[error("runtime resource limits exceed policy")]
    ResourceLimitsExceeded,
    /// A lifecycle target is not owned by BrowserPane policy.
    #[error("runtime lifecycle target is not owned")]
    LifecycleTargetNotOwned,
    /// The lifecycle action is not approved for the operation family.
    #[error("runtime lifecycle action is not approved")]
    LifecycleActionNotAllowed,
}

/// A sanitized policy violation.
#[derive(Debug, Clone, Copy, Eq, Error, PartialEq)]
#[error("runtime broker policy denied the request: {code}")]
pub struct PolicyViolation {
    /// Stable denial code. Submitted values are intentionally omitted.
    pub code: PolicyErrorCode,
}

impl From<PolicyErrorCode> for PolicyViolation {
    fn from(code: PolicyErrorCode) -> Self {
        Self { code }
    }
}

/// Maximum resources accepted for one operation family.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ResourceLimits {
    /// Maximum memory in bytes.
    pub memory_bytes: u64,
    /// Maximum CPU quota in millicores.
    pub cpu_millis: u32,
    /// Maximum process count.
    pub pids: u32,
    /// Maximum shared memory in bytes.
    pub shm_bytes: u64,
    /// Maximum operation duration in seconds.
    pub timeout_secs: u64,
    /// Maximum retained stdout or stderr bytes.
    pub output_limit_bytes: u64,
}

impl ResourceLimits {
    fn is_nonzero_and_within(&self, maximum: &Self) -> bool {
        self.memory_bytes > 0
            && self.memory_bytes <= maximum.memory_bytes
            && self.cpu_millis > 0
            && self.cpu_millis <= maximum.cpu_millis
            && self.pids > 0
            && self.pids <= maximum.pids
            && self.shm_bytes > 0
            && self.shm_bytes <= maximum.shm_bytes
            && self.timeout_secs > 0
            && self.timeout_secs <= maximum.timeout_secs
            && self.output_limit_bytes > 0
            && self.output_limit_bytes <= maximum.output_limit_bytes
    }
}

/// Mount source considered by the broker's internal adapter policy.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum MountSource {
    /// Broker-owned Docker named volume.
    NamedVolume(String),
    /// Host path. The current production policy always rejects this variant.
    HostPath(String),
}

/// Materialized container mount inspected before adapter dispatch.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ContainerMount {
    /// Candidate source.
    pub source: MountSource,
    /// Fixed container target.
    pub target: String,
    /// Whether the mount is read-only.
    pub read_only: bool,
}

/// Materialized security settings inspected before adapter dispatch.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ContainerSecurity {
    /// Docker privileged mode.
    pub privileged: bool,
    /// Host network namespace sharing.
    pub host_network: bool,
    /// Host process namespace sharing.
    pub host_pid: bool,
    /// Host IPC namespace sharing.
    pub host_ipc: bool,
    /// Host devices passed through to the container.
    pub devices: Vec<String>,
    /// Added Linux capabilities.
    pub added_capabilities: BTreeSet<String>,
    /// Whether no-new-privileges is enforced.
    pub no_new_privileges: bool,
    /// Whether the root filesystem is read-only.
    pub read_only_root_filesystem: bool,
    /// Selected seccomp profile.
    pub seccomp_profile: String,
}

/// Fully materialized internal launch specification validated before Docker use.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ContainerLaunchSpec {
    /// Product operation family.
    pub operation_kind: RuntimeOperationKind,
    /// Primary BrowserPane resource identifier.
    pub resource_id: Uuid,
    /// Candidate immutable image reference.
    pub image: String,
    /// Candidate container name.
    pub container_name: String,
    /// Candidate network.
    pub network: Option<String>,
    /// Candidate mounts.
    pub mounts: Vec<ContainerMount>,
    /// Candidate environment keys. Values remain outside policy diagnostics.
    pub environment_keys: BTreeSet<String>,
    /// Candidate ownership labels.
    pub labels: BTreeMap<String, String>,
    /// Candidate fixed entrypoint and command.
    pub entrypoint: Vec<String>,
    /// Candidate security settings.
    pub security: ContainerSecurity,
    /// Candidate resource bounds.
    pub resources: ResourceLimits,
}

/// Broker-owned launch policy for one operation family.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ContainerLaunchPolicy {
    /// Exact approved image reference.
    pub image: String,
    /// Fixed owned name prefix.
    pub container_name_prefix: String,
    /// Exact approved network.
    pub network: Option<String>,
    /// Allowed named-volume prefixes.
    pub volume_prefixes: Vec<String>,
    /// Fixed approved container mount targets.
    pub mount_targets: BTreeSet<String>,
    /// Whether every approved mount must be read-only.
    pub require_read_only_mounts: bool,
    /// Exact environment key allowlist.
    pub environment_keys: BTreeSet<String>,
    /// Static labels in addition to broker-derived ownership labels.
    pub static_labels: BTreeMap<String, String>,
    /// Exact approved entrypoint and command.
    pub entrypoint: Vec<String>,
    /// Approved additional capabilities, normally empty.
    pub added_capabilities: BTreeSet<String>,
    /// Approved seccomp profiles.
    pub seccomp_profiles: BTreeSet<String>,
    /// Whether a read-only root filesystem is required.
    pub require_read_only_root_filesystem: bool,
    /// Maximum resource bounds.
    pub maximum_resources: ResourceLimits,
}

/// Lifecycle ownership policy for one operation family.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LifecyclePolicy {
    /// Fixed container-name prefix.
    pub container_name_prefix: String,
    /// Fixed volume-name prefixes.
    pub volume_name_prefixes: Vec<String>,
    /// Approved container actions.
    pub container_actions: BTreeSet<ContainerLifecycleAction>,
    /// Approved volume actions.
    pub volume_actions: BTreeSet<VolumeLifecycleAction>,
}

/// Policy configuration installed locally in the trusted broker.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ContainerPolicyConfig {
    /// Launch policies keyed by product operation family.
    pub launch: BTreeMap<RuntimeOperationKind, ContainerLaunchPolicy>,
    /// Lifecycle policies keyed by product operation family.
    pub lifecycle: BTreeMap<RuntimeOperationKind, LifecyclePolicy>,
}

/// Owned container target inspected before lifecycle dispatch.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct OwnedContainerTarget {
    /// Product operation family.
    pub operation_kind: RuntimeOperationKind,
    /// Primary BrowserPane resource identifier.
    pub resource_id: Uuid,
    /// Candidate container name.
    pub container_name: String,
    /// Requested action.
    pub action: ContainerLifecycleAction,
}

/// Owned volume target inspected before lifecycle dispatch.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct OwnedVolumeTarget {
    /// Product operation family.
    pub operation_kind: RuntimeOperationKind,
    /// Primary BrowserPane resource identifier.
    pub resource_id: Uuid,
    /// Candidate volume name.
    pub volume_name: String,
    /// Requested action.
    pub action: VolumeLifecycleAction,
}

/// Deny-by-default runtime broker policy evaluator.
#[derive(Debug, Clone)]
pub struct RuntimeBrokerPolicy {
    config: ContainerPolicyConfig,
}

impl RuntimeBrokerPolicy {
    /// Creates a policy from trusted local configuration.
    pub fn new(config: ContainerPolicyConfig) -> Self {
        Self { config }
    }

    /// Validates a fully materialized container launch specification.
    ///
    /// # Errors
    ///
    /// Returns a sanitized policy violation for the first failed invariant.
    pub fn authorize_launch(&self, spec: &ContainerLaunchSpec) -> Result<(), PolicyViolation> {
        let policy = self
            .config
            .launch
            .get(&spec.operation_kind)
            .ok_or(PolicyErrorCode::OperationNotAllowed)?;
        if spec.image != policy.image {
            return Err(PolicyErrorCode::ImageNotAllowed.into());
        }
        if spec.container_name != owned_name(&policy.container_name_prefix, spec.resource_id) {
            return Err(PolicyErrorCode::ContainerNameNotOwned.into());
        }
        if spec.network != policy.network {
            return Err(PolicyErrorCode::NetworkNotAllowed.into());
        }
        validate_mounts(spec, policy)?;
        if !spec.environment_keys.is_subset(&policy.environment_keys) {
            return Err(PolicyErrorCode::EnvironmentNotAllowed.into());
        }
        if spec.labels != expected_labels(spec, policy) {
            return Err(PolicyErrorCode::LabelsInvalid.into());
        }
        if spec.entrypoint != policy.entrypoint {
            return Err(PolicyErrorCode::EntrypointNotAllowed.into());
        }
        validate_security(spec, policy)?;
        if !spec
            .resources
            .is_nonzero_and_within(&policy.maximum_resources)
        {
            return Err(PolicyErrorCode::ResourceLimitsExceeded.into());
        }
        Ok(())
    }

    /// Validates an owned-container lifecycle target.
    ///
    /// # Errors
    ///
    /// Returns a sanitized violation when ownership or action policy fails.
    pub fn authorize_container_lifecycle(
        &self,
        target: &OwnedContainerTarget,
    ) -> Result<(), PolicyViolation> {
        let policy = self
            .config
            .lifecycle
            .get(&target.operation_kind)
            .ok_or(PolicyErrorCode::OperationNotAllowed)?;
        if target.container_name != owned_name(&policy.container_name_prefix, target.resource_id) {
            return Err(PolicyErrorCode::LifecycleTargetNotOwned.into());
        }
        if !policy.container_actions.contains(&target.action) {
            return Err(PolicyErrorCode::LifecycleActionNotAllowed.into());
        }
        Ok(())
    }

    /// Validates an owned-volume lifecycle target.
    ///
    /// # Errors
    ///
    /// Returns a sanitized violation when ownership or action policy fails.
    pub fn authorize_volume_lifecycle(
        &self,
        target: &OwnedVolumeTarget,
    ) -> Result<(), PolicyViolation> {
        let policy = self
            .config
            .lifecycle
            .get(&target.operation_kind)
            .ok_or(PolicyErrorCode::OperationNotAllowed)?;
        let owned = policy
            .volume_name_prefixes
            .iter()
            .any(|prefix| target.volume_name == owned_name(prefix, target.resource_id));
        if !owned {
            return Err(PolicyErrorCode::LifecycleTargetNotOwned.into());
        }
        if !policy.volume_actions.contains(&target.action) {
            return Err(PolicyErrorCode::LifecycleActionNotAllowed.into());
        }
        Ok(())
    }
}

fn owned_name(prefix: &str, resource_id: Uuid) -> String {
    format!("{prefix}-{}", resource_id.simple())
}

fn validate_mounts(
    spec: &ContainerLaunchSpec,
    policy: &ContainerLaunchPolicy,
) -> Result<(), PolicyViolation> {
    let mut targets = BTreeSet::new();
    for mount in &spec.mounts {
        let MountSource::NamedVolume(volume) = &mount.source else {
            return Err(PolicyErrorCode::MountNotAllowed.into());
        };
        if !policy
            .volume_prefixes
            .iter()
            .any(|prefix| has_owned_prefix(volume, prefix))
            || !policy.mount_targets.contains(&mount.target)
            || !targets.insert(&mount.target)
            || (policy.require_read_only_mounts && !mount.read_only)
        {
            return Err(PolicyErrorCode::MountNotAllowed.into());
        }
    }
    Ok(())
}

fn has_owned_prefix(value: &str, prefix: &str) -> bool {
    value == prefix
        || value
            .strip_prefix(prefix)
            .is_some_and(|suffix| suffix.starts_with('-'))
}

fn expected_labels(
    spec: &ContainerLaunchSpec,
    policy: &ContainerLaunchPolicy,
) -> BTreeMap<String, String> {
    let mut labels = policy.static_labels.clone();
    labels.insert(
        "browserpane.runtime.operation".to_string(),
        operation_kind_name(spec.operation_kind).to_string(),
    );
    labels.insert(
        "browserpane.runtime.resource_id".to_string(),
        spec.resource_id.to_string(),
    );
    labels
}

fn operation_kind_name(kind: RuntimeOperationKind) -> &'static str {
    match kind {
        RuntimeOperationKind::BrowserRuntime => "browser_runtime",
        RuntimeOperationKind::WorkflowWorker => "workflow_worker",
        RuntimeOperationKind::RecordingWorker => "recording_worker",
        RuntimeOperationKind::StorageHelper => "storage_helper",
    }
}

fn validate_security(
    spec: &ContainerLaunchSpec,
    policy: &ContainerLaunchPolicy,
) -> Result<(), PolicyViolation> {
    let security = &spec.security;
    if security.privileged
        || security.host_network
        || security.host_pid
        || security.host_ipc
        || !security.devices.is_empty()
        || !security
            .added_capabilities
            .is_subset(&policy.added_capabilities)
        || !security.no_new_privileges
        || (policy.require_read_only_root_filesystem && !security.read_only_root_filesystem)
        || !policy.seccomp_profiles.contains(&security.seccomp_profile)
    {
        return Err(PolicyErrorCode::SecuritySettingsNotAllowed.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
