use std::collections::{BTreeMap, BTreeSet};

use bpane_runtime_contract::{
    ContainerLaunchPolicy, ContainerLifecycleAction, ContainerLifecycleRequest,
    ContainerPolicyConfig, ContainerSecurity, LifecyclePolicy, OwnedContainerTarget,
    ResourceLimits, RuntimeBrokerPolicy, RuntimeOperationKind, SecretValue,
};

/// Trusted OIDC bootstrap used by broker-owned worker containers.
#[derive(Clone, Eq, PartialEq)]
pub struct WorkerOidcConfig {
    pub token_url: String,
    pub client_id: String,
    pub client_secret: SecretValue,
    pub scopes: String,
}

impl std::fmt::Debug for WorkerOidcConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WorkerOidcConfig")
            .field("token_url", &self.token_url)
            .field("client_id", &self.client_id)
            .field("client_secret", &"[REDACTED]")
            .field("scopes", &self.scopes)
            .finish()
    }
}

/// Trusted broker configuration for workflow worker containers.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct WorkflowWorkerDockerConfig {
    pub image: String,
    pub network: String,
    pub container_name_prefix: String,
    pub gateway_api_url: String,
    pub work_root: String,
    pub request_timeout_ms: u64,
    pub output_limit_bytes: u64,
    pub command: Vec<String>,
    pub seccomp_profile: String,
    pub resources: ResourceLimits,
    pub oidc: Option<WorkerOidcConfig>,
}

/// Trusted broker configuration for recording worker containers.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RecordingWorkerDockerConfig {
    pub image: String,
    pub network: String,
    pub container_name_prefix: String,
    pub artifact_volume: String,
    pub chrome_executable: String,
    pub gateway_api_url: String,
    pub page_url: String,
    pub connect_gateway_url: String,
    pub output_root: String,
    pub cert_spki: Option<String>,
    pub headless: bool,
    pub connect_timeout_ms: u64,
    pub poll_interval_ms: u64,
    pub request_timeout_ms: u64,
    pub command: Vec<String>,
    pub seccomp_profile: String,
    pub resources: ResourceLimits,
    pub oidc: Option<WorkerOidcConfig>,
}

/// Trusted worker operation families enabled in one broker process.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct WorkerRuntimeDockerConfig {
    pub workflow: Option<WorkflowWorkerDockerConfig>,
    pub recording: Option<RecordingWorkerDockerConfig>,
}

impl WorkerRuntimeDockerConfig {
    pub(super) fn build_policy(&self) -> Result<RuntimeBrokerPolicy, &'static str> {
        if self.workflow.is_none() && self.recording.is_none() {
            return Err("worker runtime Docker configuration is empty");
        }
        let mut launch = BTreeMap::new();
        let mut lifecycle = BTreeMap::new();
        if let Some(config) = &self.workflow {
            validate_workflow(config)?;
            launch.insert(
                RuntimeOperationKind::WorkflowWorker,
                launch_policy(LaunchPolicyInput {
                    image: &config.image,
                    network: &config.network,
                    container_name_prefix: &config.container_name_prefix,
                    fixed_volumes: BTreeSet::new(),
                    mount_targets: BTreeSet::new(),
                    environment_keys: workflow_environment_keys(),
                    command: &config.command,
                    seccomp_profile: &config.seccomp_profile,
                    resources: &config.resources,
                }),
            );
            lifecycle.insert(
                RuntimeOperationKind::WorkflowWorker,
                lifecycle_policy(&config.container_name_prefix),
            );
        }
        if let Some(config) = &self.recording {
            validate_recording(config)?;
            launch.insert(
                RuntimeOperationKind::RecordingWorker,
                launch_policy(LaunchPolicyInput {
                    image: &config.image,
                    network: &config.network,
                    container_name_prefix: &config.container_name_prefix,
                    fixed_volumes: BTreeSet::from([config.artifact_volume.clone()]),
                    mount_targets: BTreeSet::from([config.output_root.clone()]),
                    environment_keys: recording_environment_keys(),
                    command: &config.command,
                    seccomp_profile: &config.seccomp_profile,
                    resources: &config.resources,
                }),
            );
            lifecycle.insert(
                RuntimeOperationKind::RecordingWorker,
                lifecycle_policy(&config.container_name_prefix),
            );
        }
        RuntimeBrokerPolicy::new(ContainerPolicyConfig { launch, lifecycle })
            .map_err(|_| "worker runtime Docker policy is invalid")
    }

    pub(super) fn lifecycle_target(
        &self,
        request: &ContainerLifecycleRequest,
    ) -> Result<OwnedContainerTarget, &'static str> {
        let prefix = match request.operation_kind {
            RuntimeOperationKind::WorkflowWorker => self
                .workflow
                .as_ref()
                .map(|config| config.container_name_prefix.as_str()),
            RuntimeOperationKind::RecordingWorker => self
                .recording
                .as_ref()
                .map(|config| config.container_name_prefix.as_str()),
            _ => None,
        }
        .ok_or("worker operation family is not configured")?;
        Ok(OwnedContainerTarget {
            operation_kind: request.operation_kind,
            resource_id: request.resource_id,
            container_name: owned_name(prefix, request.resource_id),
            action: request.action,
        })
    }
}

struct LaunchPolicyInput<'a> {
    image: &'a str,
    network: &'a str,
    container_name_prefix: &'a str,
    fixed_volumes: BTreeSet<String>,
    mount_targets: BTreeSet<String>,
    environment_keys: BTreeSet<String>,
    command: &'a [String],
    seccomp_profile: &'a str,
    resources: &'a ResourceLimits,
}

fn launch_policy(input: LaunchPolicyInput<'_>) -> ContainerLaunchPolicy {
    ContainerLaunchPolicy {
        image: input.image.to_string(),
        container_name_prefix: input.container_name_prefix.to_string(),
        network: Some(input.network.to_string()),
        volume_prefixes: Vec::new(),
        fixed_volumes: input.fixed_volumes,
        mount_targets: input.mount_targets,
        require_read_only_mounts: false,
        environment_keys: input.environment_keys,
        static_labels: BTreeMap::new(),
        derived_label_keys: BTreeSet::new(),
        entrypoint: input.command.to_vec(),
        added_capabilities: BTreeSet::new(),
        seccomp_profiles: BTreeSet::from([input.seccomp_profile.to_string()]),
        require_read_only_root_filesystem: false,
        maximum_resources: input.resources.clone(),
    }
}

fn lifecycle_policy(prefix: &str) -> LifecyclePolicy {
    LifecyclePolicy {
        container_name_prefix: prefix.to_string(),
        volume_name_prefixes: Vec::new(),
        container_actions: BTreeSet::from([
            ContainerLifecycleAction::Inspect,
            ContainerLifecycleAction::Stop,
            ContainerLifecycleAction::Remove,
        ]),
        volume_actions: BTreeSet::new(),
    }
}

fn validate_workflow(config: &WorkflowWorkerDockerConfig) -> Result<(), &'static str> {
    if !is_safe_url(&config.gateway_api_url)
        || !is_safe_path(&config.work_root)
        || config.request_timeout_ms == 0
        || config.output_limit_bytes == 0
        || !valid_oidc(config.oidc.as_ref())
    {
        return Err("workflow worker Docker configuration is invalid");
    }
    Ok(())
}

fn validate_recording(config: &RecordingWorkerDockerConfig) -> Result<(), &'static str> {
    if !is_safe_name(&config.artifact_volume)
        || !is_safe_path(&config.chrome_executable)
        || !is_safe_path(&config.output_root)
        || !is_safe_url(&config.gateway_api_url)
        || !is_safe_url(&config.page_url)
        || !is_safe_url(&config.connect_gateway_url)
        || config.connect_timeout_ms == 0
        || config.poll_interval_ms == 0
        || config.request_timeout_ms == 0
        || config
            .cert_spki
            .as_deref()
            .is_some_and(|value| value.is_empty() || value.len() > 512 || has_control(value))
        || !valid_oidc(config.oidc.as_ref())
    {
        return Err("recording worker Docker configuration is invalid");
    }
    Ok(())
}

fn valid_oidc(config: Option<&WorkerOidcConfig>) -> bool {
    config.is_none_or(|config| {
        is_safe_url(&config.token_url)
            && is_safe_text(&config.client_id, 256)
            && config.scopes.len() <= 1_024
            && !has_control(&config.scopes)
            && is_safe_secret(config.client_secret.expose_secret())
    })
}

fn is_safe_secret(value: &str) -> bool {
    !value.is_empty() && !has_control(value)
}

fn is_safe_text(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && !has_control(value)
}

fn has_control(value: &str) -> bool {
    value
        .bytes()
        .any(|byte| byte == 0 || byte.is_ascii_control())
}

fn is_safe_url(value: &str) -> bool {
    reqwest::Url::parse(value).is_ok_and(|url| {
        matches!(url.scheme(), "http" | "https")
            && !url.cannot_be_a_base()
            && url.username().is_empty()
            && url.password().is_none()
            && url.fragment().is_none()
    })
}

fn is_safe_path(value: &str) -> bool {
    value.starts_with('/')
        && value != "/"
        && value.len() <= 512
        && !has_control(value)
        && value.split('/').all(|segment| segment != "..")
}

fn is_safe_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

pub(super) fn owned_name(prefix: &str, resource_id: uuid::Uuid) -> String {
    format!("{}-{}", prefix.trim_end_matches('-'), resource_id.simple())
}

fn common_oidc_environment_keys() -> [&'static str; 3] {
    [
        "BPANE_GATEWAY_OIDC_TOKEN_URL",
        "BPANE_GATEWAY_OIDC_CLIENT_ID",
        "BPANE_GATEWAY_OIDC_SCOPES",
    ]
}

fn workflow_environment_keys() -> BTreeSet<String> {
    [
        "BPANE_WORKFLOW_RUN_ID",
        "BPANE_GATEWAY_API_URL",
        "BPANE_WORKFLOW_WORK_ROOT",
        "BPANE_WORKER_REQUEST_TIMEOUT_MS",
        "BPANE_WORKER_MAX_OUTPUT_BYTES",
        "BPANE_WORKER_SECRETS_FILE",
    ]
    .into_iter()
    .chain(common_oidc_environment_keys())
    .map(str::to_string)
    .collect()
}

fn recording_environment_keys() -> BTreeSet<String> {
    [
        "BPANE_RECORDING_SESSION_ID",
        "BPANE_RECORDING_ID",
        "BPANE_RECORDING_CHROME",
        "BPANE_GATEWAY_API_URL",
        "BPANE_RECORDING_PAGE_URL",
        "BPANE_RECORDING_OUTPUT_ROOT",
        "BPANE_RECORDING_CONNECT_TRANSPORT_PATH",
        "BPANE_WORKER_SECRETS_FILE",
        "BPANE_RECORDING_CONNECT_TIMEOUT_MS",
        "BPANE_RECORDING_POLL_INTERVAL_MS",
        "BPANE_WORKER_REQUEST_TIMEOUT_MS",
        "BPANE_RECORDING_HEADLESS",
        "BPANE_RECORDING_CERT_SPKI",
        "BPANE_RECORDING_CONNECT_GATEWAY_URL",
    ]
    .into_iter()
    .chain(common_oidc_environment_keys())
    .map(str::to_string)
    .collect()
}

pub(super) fn policy_security(config: &str) -> ContainerSecurity {
    ContainerSecurity {
        privileged: false,
        host_network: false,
        host_pid: false,
        host_ipc: false,
        devices: Vec::new(),
        added_capabilities: BTreeSet::new(),
        no_new_privileges: true,
        read_only_root_filesystem: false,
        seccomp_profile: config.to_string(),
    }
}
