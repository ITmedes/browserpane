use std::collections::{BTreeMap, BTreeSet, HashMap};

use bollard::models::{ContainerCreateBody, HostConfig, Mount, MountType};
use bpane_runtime_contract::{
    BrowserRuntimeLaunchRequest, ContainerLaunchSpec, ContainerMount, ContainerSecurity,
    MountSource, RuntimeOperationKind,
};

use super::BrowserRuntimeDockerConfig;

pub(super) struct MaterializedBrowserLaunch {
    pub(super) container_name: String,
    pub(super) policy_spec: ContainerLaunchSpec,
    pub(super) container: ContainerCreateBody,
}

impl MaterializedBrowserLaunch {
    pub(super) fn new(
        config: &BrowserRuntimeDockerConfig,
        request: &BrowserRuntimeLaunchRequest,
    ) -> Self {
        let container_name = config.container_name(request.session_id);
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
        let environment = environment(config, request.session_id);
        let labels = BTreeMap::from([
            (
                "browserpane.runtime.operation".to_string(),
                "browser_runtime".to_string(),
            ),
            (
                "browserpane.runtime.resource_id".to_string(),
                request.session_id.to_string(),
            ),
        ]);
        let policy_spec = ContainerLaunchSpec {
            operation_kind: RuntimeOperationKind::BrowserRuntime,
            resource_id: request.session_id,
            owned_volume_ids,
            image: config.image.clone(),
            container_name: container_name.clone(),
            network: Some(config.network.clone()),
            mounts: mounts
                .iter()
                .map(|mount| ContainerMount {
                    source: MountSource::NamedVolume(mount.source.clone().unwrap_or_default()),
                    target: mount.target.clone().unwrap_or_default(),
                    read_only: mount.read_only.unwrap_or(false),
                })
                .collect(),
            environment_keys: environment
                .iter()
                .filter_map(|value| value.split_once('=').map(|(key, _)| key.to_string()))
                .collect(),
            labels: labels.clone(),
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
        };
        let host_config = HostConfig {
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
        };
        let container = ContainerCreateBody {
            image: Some(config.image.clone()),
            env: Some(environment),
            cmd: Some(config.command.clone()),
            labels: Some(labels.into_iter().collect::<HashMap<_, _>>()),
            host_config: Some(host_config),
            ..Default::default()
        };
        Self {
            container_name,
            policy_spec,
            container,
        }
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

fn environment(config: &BrowserRuntimeDockerConfig, session_id: uuid::Uuid) -> Vec<String> {
    vec![
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
    ]
}
