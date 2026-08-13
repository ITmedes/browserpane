use std::collections::{BTreeMap, BTreeSet, HashMap};

use bollard::models::{ContainerCreateBody, HostConfig, Mount, MountType};
use bpane_runtime_contract::{
    BrowserEgressObservationMode, BrowserEgressSelection, BrowserNetworkIdentity,
    BrowserRuntimeLaunchRequest, ContainerLaunchSpec, ContainerMount, ContainerSecurity,
    MountSource, RuntimeOperationKind,
};

use super::BrowserRuntimeDockerConfig;

pub(super) struct MaterializedBrowserLaunch {
    pub(super) container_name: String,
    pub(super) policy_spec: ContainerLaunchSpec,
    pub(super) container: ContainerCreateBody,
}

struct BrowserLaunchParts {
    mounts: Vec<Mount>,
    owned_volume_ids: BTreeSet<uuid::Uuid>,
    environment: Vec<String>,
    labels: BTreeMap<String, String>,
}

impl BrowserLaunchParts {
    fn new(
        config: &BrowserRuntimeDockerConfig,
        request: &BrowserRuntimeLaunchRequest,
    ) -> Result<Self, &'static str> {
        let (mounts, owned_volume_ids) = launch_mounts(config, request);
        Ok(Self {
            mounts,
            owned_volume_ids,
            environment: environment(config, request)?,
            labels: runtime_labels(request),
        })
    }
}

impl MaterializedBrowserLaunch {
    pub(super) fn new(
        config: &BrowserRuntimeDockerConfig,
        request: &BrowserRuntimeLaunchRequest,
    ) -> Result<Self, &'static str> {
        let container_name = config.container_name(request.session_id);
        let parts = BrowserLaunchParts::new(config, request)?;
        let policy_spec = policy_spec(config, request, &container_name, &parts);
        let container = container_body(config, parts);
        Ok(Self {
            container_name,
            policy_spec,
            container,
        })
    }
}

fn launch_mounts(
    config: &BrowserRuntimeDockerConfig,
    request: &BrowserRuntimeLaunchRequest,
) -> (Vec<Mount>, BTreeSet<uuid::Uuid>) {
    let mut mounts = vec![
        mount(&config.socket_volume, &config.socket_mount_root),
        mount(
            &config.session_data_volume(request.session_id),
            &config.session_data_root,
        ),
    ];
    let mut owned_volume_ids = BTreeSet::from([request.session_id]);
    if let Some(context_id) = request.browser_context_id {
        owned_volume_ids.insert(context_id);
        mounts.push(mount(
            &config.browser_context_volume(context_id),
            &config.profile_dir(),
        ));
    }
    (mounts, owned_volume_ids)
}

fn policy_spec(
    config: &BrowserRuntimeDockerConfig,
    request: &BrowserRuntimeLaunchRequest,
    container_name: &str,
    parts: &BrowserLaunchParts,
) -> ContainerLaunchSpec {
    ContainerLaunchSpec {
        operation_kind: RuntimeOperationKind::BrowserRuntime,
        resource_id: request.session_id,
        owned_volume_ids: parts.owned_volume_ids.clone(),
        image: config.image.clone(),
        container_name: container_name.to_string(),
        network: Some(config.network.clone()),
        mounts: parts.mounts.iter().map(policy_mount).collect(),
        environment_keys: parts
            .environment
            .iter()
            .filter_map(|value| value.split_once('=').map(|(key, _)| key.to_string()))
            .collect(),
        labels: parts.labels.clone(),
        entrypoint: config.command.clone(),
        security: ContainerSecurity {
            privileged: false,
            host_network: false,
            host_pid: false,
            host_ipc: false,
            devices: Vec::new(),
            added_capabilities: BTreeSet::new(),
            no_new_privileges: true,
            read_only_root_filesystem: false,
            seccomp_profile: config.seccomp_profile.clone(),
        },
        resources: config.resources.clone(),
    }
}

fn policy_mount(mount: &Mount) -> ContainerMount {
    ContainerMount {
        source: MountSource::NamedVolume(mount.source.clone().unwrap_or_default()),
        target: mount.target.clone().unwrap_or_default(),
        read_only: mount.read_only.unwrap_or(false),
    }
}

fn container_body(
    config: &BrowserRuntimeDockerConfig,
    parts: BrowserLaunchParts,
) -> ContainerCreateBody {
    ContainerCreateBody {
        image: Some(config.image.clone()),
        env: Some(parts.environment),
        cmd: Some(config.command.clone()),
        labels: Some(parts.labels.into_iter().collect::<HashMap<_, _>>()),
        host_config: Some(host_config(config, parts.mounts)),
        ..Default::default()
    }
}

fn host_config(config: &BrowserRuntimeDockerConfig, mounts: Vec<Mount>) -> HostConfig {
    HostConfig {
        memory: Some(config.resources.memory_bytes as i64),
        nano_cpus: Some(i64::from(config.resources.cpu_millis) * 1_000_000),
        pids_limit: Some(i64::from(config.resources.pids)),
        network_mode: Some(config.network.clone()),
        auto_remove: Some(true),
        mounts: Some(mounts),
        privileged: Some(false),
        readonly_rootfs: Some(false),
        security_opt: Some(vec!["no-new-privileges:true".to_string()]),
        shm_size: Some(config.resources.shm_bytes as i64),
        ..Default::default()
    }
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

fn environment(
    config: &BrowserRuntimeDockerConfig,
    request: &BrowserRuntimeLaunchRequest,
) -> Result<Vec<String>, &'static str> {
    let session_id = request.session_id;
    let mut environment = vec![
        format!("BPANE_SESSION_ID={session_id}"),
        format!("BPANE_SOCKET_PATH={}", config.socket_path(session_id)),
        format!("BPANE_SESSION_DATA_DIR={}", config.session_data_root),
        format!("BPANE_PROFILE_DIR={}", config.profile_dir()),
        format!("BPANE_UPLOAD_DIR={}", config.upload_dir()),
        format!("BPANE_DOWNLOAD_DIR={}", config.download_dir()),
        format!(
            "BPANE_SESSION_FILE_MOUNTS_DIR={}",
            config.session_file_mounts_dir()
        ),
        format!(
            "BPANE_SESSION_FILE_BINDINGS_MANIFEST={}",
            config.session_file_manifest()
        ),
    ];
    add_identity_environment(&mut environment, &request.features.network_identity);
    if let Some(egress) = &request.features.egress {
        add_egress_environment(&mut environment, config, egress);
    }
    add_extension_environment(&mut environment, config, request)?;
    Ok(environment)
}

fn add_identity_environment(environment: &mut Vec<String>, identity: &BrowserNetworkIdentity) {
    if let Some(locale) = &identity.locale {
        let posix_locale = posix_locale(locale);
        push_env(environment, "LANG", &posix_locale);
        push_env(environment, "LC_ALL", &posix_locale);
        push_env(environment, "BPANE_CHROMIUM_LANG", locale);
        if identity.languages.is_empty() {
            push_env(environment, "BPANE_CHROMIUM_ACCEPT_LANG", locale);
        }
    }
    if !identity.languages.is_empty() {
        push_env(environment, "LANGUAGE", &identity.languages.join(":"));
        push_env(
            environment,
            "BPANE_CHROMIUM_ACCEPT_LANG",
            &identity.languages.join(","),
        );
    }
    if let Some(timezone) = &identity.timezone {
        push_env(environment, "TZ", timezone);
    }
    if let Some(geolocation) = &identity.geolocation {
        let value = serde_json::json!({
            "latitude": f64::from(geolocation.latitude_e7) / 10_000_000.0,
            "longitude": f64::from(geolocation.longitude_e7) / 10_000_000.0,
            "accuracy_meters": geolocation.accuracy_mm.map(|value| f64::from(value) / 1_000.0),
        });
        push_env(environment, "BPANE_SESSION_GEOLOCATION", &value.to_string());
    }
    if let Some(user_agent) = &identity.user_agent {
        push_env(environment, "BPANE_CHROMIUM_USER_AGENT", user_agent);
    }
    if let Some(browser_identity) = &identity.browser_identity {
        push_env(environment, "BPANE_BROWSER_IDENTITY", browser_identity);
    }
}

fn add_egress_environment(
    environment: &mut Vec<String>,
    config: &BrowserRuntimeDockerConfig,
    egress: &BrowserEgressSelection,
) {
    push_env(
        environment,
        "BPANE_EGRESS_PROFILE_ID",
        &egress.profile_id.to_string(),
    );
    push_env(
        environment,
        "BPANE_EGRESS_OBSERVATION_MODE",
        egress.observation_mode.as_str(),
    );
    if let Some(proxy) = &egress.proxy {
        push_env(environment, "BPANE_CHROMIUM_PROXY_SERVER", &proxy.url);
        if proxy.authentication.is_some() {
            push_env(
                environment,
                "BPANE_CHROMIUM_PROXY_AUTH_FILE",
                &config.proxy_auth_path(),
            );
            push_env(environment, "BPANE_URL", "about:blank");
        }
    }
    if !egress.bypass_rules.is_empty() {
        push_env(
            environment,
            "BPANE_CHROMIUM_PROXY_BYPASS_LIST",
            &egress.bypass_rules.join(";"),
        );
    }
    if egress.custom_ca.is_some() {
        push_env(
            environment,
            "BPANE_CHROMIUM_TRUSTED_CA_BUNDLE",
            &config.trusted_ca_path(),
        );
        push_env(
            environment,
            "BPANE_CHROMIUM_TRUSTED_CA_NAME",
            "BrowserPane Egress Interception CA",
        );
    }
}

fn add_extension_environment(
    environment: &mut Vec<String>,
    config: &BrowserRuntimeDockerConfig,
    request: &BrowserRuntimeLaunchRequest,
) -> Result<(), &'static str> {
    let extension_dirs = config.extension_dirs(&request.features.extension_version_ids)?;
    if !extension_dirs.is_empty() {
        push_env(
            environment,
            "BPANE_EXTENSION_DIRS",
            &extension_dirs.join(","),
        );
    }
    Ok(())
}

fn runtime_labels(request: &BrowserRuntimeLaunchRequest) -> BTreeMap<String, String> {
    let mut labels = BTreeMap::from([
        (
            "browserpane.runtime.operation".to_string(),
            "browser_runtime".to_string(),
        ),
        (
            "browserpane.runtime.resource_id".to_string(),
            request.session_id.to_string(),
        ),
    ]);
    add_egress_labels(&mut labels, request);
    labels
}

fn add_egress_labels(labels: &mut BTreeMap<String, String>, request: &BrowserRuntimeLaunchRequest) {
    let Some(egress) = &request.features.egress else {
        return;
    };
    let tls_interception = egress.observation_mode == BrowserEgressObservationMode::TlsIntercept;
    labels.extend([
        (
            "browserpane.egress_profile_id".to_string(),
            egress.profile_id.to_string(),
        ),
        (
            "browserpane.egress_observation_mode".to_string(),
            egress.observation_mode.as_str().to_string(),
        ),
        (
            "browserpane.egress_proxy_configured".to_string(),
            egress.proxy.is_some().to_string(),
        ),
        (
            "browserpane.egress_proxy_auth_configured".to_string(),
            egress
                .proxy
                .as_ref()
                .is_some_and(|proxy| proxy.authentication.is_some())
                .to_string(),
        ),
        (
            "browserpane.egress_bypass_rule_count".to_string(),
            egress.bypass_rules.len().to_string(),
        ),
        (
            "browserpane.egress_custom_ca_configured".to_string(),
            egress.custom_ca.is_some().to_string(),
        ),
        (
            "browserpane.egress_tls_interception_enabled".to_string(),
            tls_interception.to_string(),
        ),
        (
            "browserpane.egress_sensitive_log_sink_configured".to_string(),
            egress.sensitive_log_sink_configured.to_string(),
        ),
    ]);
}

fn push_env(environment: &mut Vec<String>, key: &str, value: &str) {
    environment.push(format!("{key}={value}"));
}

fn posix_locale(locale: &str) -> String {
    let normalized = locale.replace('-', "_");
    if normalized.contains('.') {
        normalized
    } else {
        format!("{normalized}.UTF-8")
    }
}
