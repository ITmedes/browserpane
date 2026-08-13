use std::collections::{BTreeMap, BTreeSet, HashMap};

use bollard::models::{ContainerCreateBody, HostConfig, HostConfigLogConfig, Mount, MountType};
use bpane_runtime_contract::{
    ContainerLaunchSpec, ContainerMount, MountSource, RecordingWorkerLaunchRequest,
    RuntimeOperationKind, WorkflowWorkerLaunchRequest,
};

use super::config::{
    owned_name, policy_security, RecordingWorkerDockerConfig, WorkerOidcConfig,
    WorkflowWorkerDockerConfig,
};

pub(super) struct MaterializedWorkerLaunch {
    pub(super) container_name: String,
    pub(super) policy_spec: ContainerLaunchSpec,
    pub(super) container: ContainerCreateBody,
}

impl MaterializedWorkerLaunch {
    pub(super) fn workflow(
        config: &WorkflowWorkerDockerConfig,
        request: &WorkflowWorkerLaunchRequest,
    ) -> Result<Self, &'static str> {
        let environment = workflow_environment(config, request)?;
        build(
            RuntimeOperationKind::WorkflowWorker,
            request.workflow_run_id,
            &config.image,
            &config.network,
            &config.container_name_prefix,
            &config.command,
            &config.seccomp_profile,
            &config.resources,
            environment,
            Vec::new(),
        )
    }

    pub(super) fn recording(
        config: &RecordingWorkerDockerConfig,
        request: &RecordingWorkerLaunchRequest,
    ) -> Result<Self, &'static str> {
        let environment = recording_environment(config, request)?;
        build(
            RuntimeOperationKind::RecordingWorker,
            request.recording_id,
            &config.image,
            &config.network,
            &config.container_name_prefix,
            &config.command,
            &config.seccomp_profile,
            &config.resources,
            environment,
            vec![mount(&config.artifact_volume, &config.output_root)],
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn build(
    operation_kind: RuntimeOperationKind,
    resource_id: uuid::Uuid,
    image: &str,
    network: &str,
    prefix: &str,
    command: &[String],
    seccomp_profile: &str,
    resources: &bpane_runtime_contract::ResourceLimits,
    environment: Vec<String>,
    mounts: Vec<Mount>,
) -> Result<MaterializedWorkerLaunch, &'static str> {
    if environment.iter().any(|entry| {
        !entry.contains('=')
            || entry
                .bytes()
                .any(|byte| byte == 0 || byte == b'\n' || byte == b'\r')
    }) {
        return Err("worker environment is invalid");
    }
    let container_name = owned_name(prefix, resource_id);
    let labels = BTreeMap::from([
        (
            "browserpane.runtime.operation".to_string(),
            operation_label(operation_kind).to_string(),
        ),
        (
            "browserpane.runtime.resource_id".to_string(),
            resource_id.to_string(),
        ),
    ]);
    let policy_spec = ContainerLaunchSpec {
        operation_kind,
        resource_id,
        owned_volume_ids: BTreeSet::from([resource_id]),
        image: image.to_string(),
        container_name: container_name.clone(),
        network: Some(network.to_string()),
        mounts: mounts.iter().map(policy_mount).collect(),
        environment_keys: environment
            .iter()
            .filter_map(|value| value.split_once('=').map(|(key, _)| key.to_string()))
            .collect(),
        labels: labels.clone(),
        entrypoint: command.to_vec(),
        security: policy_security(seccomp_profile),
        resources: resources.clone(),
    };
    let container = ContainerCreateBody {
        image: Some(image.to_string()),
        env: Some(environment),
        cmd: Some(command.to_vec()),
        labels: Some(labels.into_iter().collect::<HashMap<_, _>>()),
        host_config: Some(host_config(network, resources, mounts)),
        ..Default::default()
    };
    Ok(MaterializedWorkerLaunch {
        container_name,
        policy_spec,
        container,
    })
}

fn host_config(
    network: &str,
    resources: &bpane_runtime_contract::ResourceLimits,
    mounts: Vec<Mount>,
) -> HostConfig {
    HostConfig {
        memory: Some(resources.memory_bytes as i64),
        nano_cpus: Some(i64::from(resources.cpu_millis) * 1_000_000),
        pids_limit: Some(i64::from(resources.pids)),
        network_mode: Some(network.to_string()),
        auto_remove: Some(false),
        mounts: (!mounts.is_empty()).then_some(mounts),
        privileged: Some(false),
        readonly_rootfs: Some(false),
        security_opt: Some(vec!["no-new-privileges:true".to_string()]),
        shm_size: Some(resources.shm_bytes as i64),
        init: Some(true),
        log_config: Some(HostConfigLogConfig {
            typ: Some("local".to_string()),
            config: Some(HashMap::from([
                (
                    "max-size".to_string(),
                    format!("{}b", resources.output_limit_bytes),
                ),
                ("max-file".to_string(), "1".to_string()),
            ])),
        }),
        ..Default::default()
    }
}

fn workflow_environment(
    config: &WorkflowWorkerDockerConfig,
    request: &WorkflowWorkerLaunchRequest,
) -> Result<Vec<String>, &'static str> {
    let mut environment = vec![
        pair(
            "BPANE_WORKFLOW_RUN_ID",
            &request.workflow_run_id.to_string(),
        )?,
        pair("BPANE_GATEWAY_API_URL", &config.gateway_api_url)?,
        pair("BPANE_WORKFLOW_WORK_ROOT", &config.work_root)?,
        pair(
            "BPANE_WORKER_REQUEST_TIMEOUT_MS",
            &config.request_timeout_ms.to_string(),
        )?,
        pair(
            "BPANE_WORKER_MAX_OUTPUT_BYTES",
            &config.output_limit_bytes.to_string(),
        )?,
        pair(
            "BPANE_SESSION_AUTOMATION_ACCESS_TOKEN",
            request
                .credentials
                .session_automation_access_token
                .expose_secret(),
        )?,
    ];
    if let Some(token) = &request.credentials.gateway_bearer_token {
        environment.push(pair("BPANE_WORKFLOW_BEARER_TOKEN", token.expose_secret())?);
    }
    add_oidc_environment(&mut environment, config.oidc.as_ref())?;
    Ok(environment)
}

fn recording_environment(
    config: &RecordingWorkerDockerConfig,
    request: &RecordingWorkerLaunchRequest,
) -> Result<Vec<String>, &'static str> {
    let mut environment = vec![
        pair(
            "BPANE_RECORDING_SESSION_ID",
            &request.session_id.to_string(),
        )?,
        pair("BPANE_RECORDING_ID", &request.recording_id.to_string())?,
        pair("BPANE_RECORDING_CHROME", &config.chrome_executable)?,
        pair("BPANE_GATEWAY_API_URL", &config.gateway_api_url)?,
        pair("BPANE_RECORDING_PAGE_URL", &config.page_url)?,
        pair("BPANE_RECORDING_OUTPUT_ROOT", &config.output_root)?,
        pair(
            "BPANE_RECORDING_CONNECT_TICKET",
            request.credentials.connect_ticket.expose_secret(),
        )?,
        pair("BPANE_RECORDING_CONNECT_TRANSPORT_PATH", "/session")?,
        pair(
            "BPANE_SESSION_AUTOMATION_ACCESS_TOKEN",
            request
                .credentials
                .session_automation_access_token
                .expose_secret(),
        )?,
        pair(
            "BPANE_RECORDING_WORKER_ACCESS_TOKEN",
            request
                .credentials
                .recording_worker_access_token
                .expose_secret(),
        )?,
        pair(
            "BPANE_RECORDING_CONNECT_TIMEOUT_MS",
            &config.connect_timeout_ms.to_string(),
        )?,
        pair(
            "BPANE_RECORDING_POLL_INTERVAL_MS",
            &config.poll_interval_ms.to_string(),
        )?,
        pair(
            "BPANE_WORKER_REQUEST_TIMEOUT_MS",
            &config.request_timeout_ms.to_string(),
        )?,
        pair(
            "BPANE_RECORDING_HEADLESS",
            if config.headless { "true" } else { "false" },
        )?,
        pair(
            "BPANE_RECORDING_CONNECT_GATEWAY_URL",
            &config.connect_gateway_url,
        )?,
    ];
    if let Some(cert_spki) = &config.cert_spki {
        environment.push(pair("BPANE_RECORDING_CERT_SPKI", cert_spki)?);
    }
    if let Some(token) = &request.credentials.gateway_bearer_token {
        environment.push(pair("BPANE_RECORDING_BEARER_TOKEN", token.expose_secret())?);
    }
    add_oidc_environment(&mut environment, config.oidc.as_ref())?;
    Ok(environment)
}

fn add_oidc_environment(
    environment: &mut Vec<String>,
    oidc: Option<&WorkerOidcConfig>,
) -> Result<(), &'static str> {
    let Some(oidc) = oidc else {
        return Ok(());
    };
    environment.extend([
        pair("BPANE_GATEWAY_OIDC_TOKEN_URL", &oidc.token_url)?,
        pair("BPANE_GATEWAY_OIDC_CLIENT_ID", &oidc.client_id)?,
        pair(
            "BPANE_GATEWAY_OIDC_CLIENT_SECRET",
            oidc.client_secret.expose_secret(),
        )?,
    ]);
    if !oidc.scopes.is_empty() {
        environment.push(pair("BPANE_GATEWAY_OIDC_SCOPES", &oidc.scopes)?);
    }
    Ok(())
}

fn pair(key: &str, value: &str) -> Result<String, &'static str> {
    if value.is_empty()
        || value.len() > 16 * 1024
        || value
            .bytes()
            .any(|byte| byte == 0 || byte == b'\n' || byte == b'\r')
    {
        return Err("worker environment value is invalid");
    }
    Ok(format!("{key}={value}"))
}

fn mount(source: &str, target: &str) -> Mount {
    Mount {
        source: Some(source.to_string()),
        target: Some(target.to_string()),
        typ: Some(MountType::VOLUME),
        read_only: Some(false),
        ..Default::default()
    }
}

fn policy_mount(mount: &Mount) -> ContainerMount {
    ContainerMount {
        source: MountSource::NamedVolume(mount.source.clone().unwrap_or_default()),
        target: mount.target.clone().unwrap_or_default(),
        read_only: mount.read_only.unwrap_or(false),
    }
}

fn operation_label(kind: RuntimeOperationKind) -> &'static str {
    match kind {
        RuntimeOperationKind::WorkflowWorker => "workflow_worker",
        RuntimeOperationKind::RecordingWorker => "recording_worker",
        RuntimeOperationKind::BrowserRuntime => "browser_runtime",
        RuntimeOperationKind::StorageHelper => "storage_helper",
    }
}
