use std::collections::BTreeMap;

use thiserror::Error;

use super::{ContainerLaunchPolicy, ContainerPolicyConfig, LifecyclePolicy};

/// Stable errors for invalid trusted broker policy configuration.
#[derive(Debug, Clone, Copy, Eq, Error, PartialEq)]
pub enum PolicyConfigurationErrorCode {
    /// An image is not pinned to a valid SHA-256 digest.
    #[error("runtime policy image must use an immutable SHA-256 digest")]
    InvalidImage,
    /// A container ownership prefix is unsafe.
    #[error("runtime policy container-name prefix is invalid")]
    InvalidContainerNamePrefix,
    /// A configured network is unsafe or unsupported.
    #[error("runtime policy network is invalid")]
    InvalidNetwork,
    /// A named-volume prefix or fixed volume is unsafe.
    #[error("runtime policy volume ownership is invalid")]
    InvalidVolumePolicy,
    /// A mount target is not a safe absolute container path.
    #[error("runtime policy mount target is invalid")]
    InvalidMountTarget,
    /// An environment allowlist key is malformed.
    #[error("runtime policy environment key is invalid")]
    InvalidEnvironmentKey,
    /// Static labels are unsafe or overwrite broker ownership labels.
    #[error("runtime policy static labels are invalid")]
    InvalidLabels,
    /// The fixed entrypoint is absent or unsafe.
    #[error("runtime policy entrypoint is invalid")]
    InvalidEntrypoint,
    /// A seccomp profile selection is unsafe.
    #[error("runtime policy seccomp profile is invalid")]
    InvalidSeccompProfile,
    /// Maximum resource limits contain a zero value.
    #[error("runtime policy maximum resource limits are invalid")]
    InvalidResourceLimits,
    /// Lifecycle ownership or actions are incomplete or inconsistent.
    #[error("runtime lifecycle policy is invalid")]
    InvalidLifecyclePolicy,
}

/// A sanitized trusted-policy configuration error.
#[derive(Debug, Clone, Copy, Eq, Error, PartialEq)]
#[error("runtime broker policy configuration is invalid: {code}")]
pub struct PolicyConfigurationError {
    /// Stable code. Configuration values are intentionally omitted.
    pub code: PolicyConfigurationErrorCode,
}

impl From<PolicyConfigurationErrorCode> for PolicyConfigurationError {
    fn from(code: PolicyConfigurationErrorCode) -> Self {
        Self { code }
    }
}

pub(super) fn validate_config(
    config: &ContainerPolicyConfig,
) -> Result<(), PolicyConfigurationError> {
    for (kind, policy) in &config.launch {
        validate_launch_policy(policy)?;
        if let Some(lifecycle) = config.lifecycle.get(kind) {
            if lifecycle.container_name_prefix != policy.container_name_prefix {
                return Err(PolicyConfigurationErrorCode::InvalidLifecyclePolicy.into());
            }
        }
    }
    for policy in config.lifecycle.values() {
        validate_lifecycle_policy(policy)?;
    }
    Ok(())
}

fn validate_launch_policy(policy: &ContainerLaunchPolicy) -> Result<(), PolicyConfigurationError> {
    if !is_immutable_image(&policy.image) {
        return Err(PolicyConfigurationErrorCode::InvalidImage.into());
    }
    if !is_safe_name(&policy.container_name_prefix) {
        return Err(PolicyConfigurationErrorCode::InvalidContainerNamePrefix.into());
    }
    if policy
        .network
        .as_deref()
        .is_some_and(|network| !is_safe_name(network) || network.eq_ignore_ascii_case("host"))
    {
        return Err(PolicyConfigurationErrorCode::InvalidNetwork.into());
    }
    if policy
        .volume_prefixes
        .iter()
        .any(|value| !is_safe_name(value))
        || policy
            .fixed_volumes
            .iter()
            .any(|value| !is_safe_name(value))
    {
        return Err(PolicyConfigurationErrorCode::InvalidVolumePolicy.into());
    }
    if policy
        .mount_targets
        .iter()
        .any(|value| !is_safe_path(value))
    {
        return Err(PolicyConfigurationErrorCode::InvalidMountTarget.into());
    }
    if policy
        .environment_keys
        .iter()
        .any(|value| !is_safe_environment_key(value))
    {
        return Err(PolicyConfigurationErrorCode::InvalidEnvironmentKey.into());
    }
    if !has_safe_static_labels(&policy.static_labels) {
        return Err(PolicyConfigurationErrorCode::InvalidLabels.into());
    }
    if !has_safe_entrypoint(&policy.entrypoint) {
        return Err(PolicyConfigurationErrorCode::InvalidEntrypoint.into());
    }
    if policy.seccomp_profiles.is_empty()
        || policy
            .seccomp_profiles
            .iter()
            .any(|value| !is_safe_seccomp_profile(value))
    {
        return Err(PolicyConfigurationErrorCode::InvalidSeccompProfile.into());
    }
    if !policy
        .maximum_resources
        .is_nonzero_and_within(&policy.maximum_resources)
    {
        return Err(PolicyConfigurationErrorCode::InvalidResourceLimits.into());
    }
    Ok(())
}

fn validate_lifecycle_policy(policy: &LifecyclePolicy) -> Result<(), PolicyConfigurationError> {
    let has_actions = !policy.container_actions.is_empty() || !policy.volume_actions.is_empty();
    let volumes_valid = policy
        .volume_name_prefixes
        .iter()
        .all(|value| is_safe_name(value));
    if !has_actions
        || !is_safe_name(&policy.container_name_prefix)
        || !volumes_valid
        || (!policy.volume_actions.is_empty() && policy.volume_name_prefixes.is_empty())
    {
        return Err(PolicyConfigurationErrorCode::InvalidLifecyclePolicy.into());
    }
    Ok(())
}

fn is_immutable_image(value: &str) -> bool {
    let Some((repository, digest)) = value.rsplit_once("@sha256:") else {
        return false;
    };
    !repository.is_empty()
        && !repository.bytes().any(|byte| byte.is_ascii_whitespace())
        && digest.len() == 64
        && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn is_safe_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn is_safe_path(value: &str) -> bool {
    value.starts_with('/')
        && value != "/"
        && value.len() <= 512
        && !value
            .bytes()
            .any(|byte| byte == 0 || byte.is_ascii_control())
        && value.split('/').all(|segment| segment != "..")
}

fn is_safe_environment_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.as_bytes()[0].is_ascii_uppercase()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
}

fn has_safe_static_labels(labels: &BTreeMap<String, String>) -> bool {
    const RESERVED: [&str; 2] = [
        "browserpane.runtime.operation",
        "browserpane.runtime.resource_id",
    ];
    labels.iter().all(|(key, value)| {
        !RESERVED.contains(&key.as_str())
            && !key.is_empty()
            && key.len() <= 128
            && key.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'/')
            })
            && value.len() <= 1_024
            && !value
                .bytes()
                .any(|byte| byte == 0 || byte.is_ascii_control())
    })
}

fn has_safe_entrypoint(entrypoint: &[String]) -> bool {
    entrypoint.first().is_some_and(|value| is_safe_path(value))
        && entrypoint.iter().all(|value| {
            !value.is_empty()
                && value.len() <= 4_096
                && !value
                    .bytes()
                    .any(|byte| byte == 0 || byte == b'\n' || byte == b'\r')
        })
}

fn is_safe_seccomp_profile(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && !value.eq_ignore_ascii_case("unconfined")
        && !value
            .bytes()
            .any(|byte| byte == 0 || byte.is_ascii_control())
        && value.split('/').all(|segment| segment != "..")
}
