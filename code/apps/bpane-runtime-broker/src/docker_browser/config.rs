use std::collections::{BTreeMap, BTreeSet};

use bpane_runtime_contract::{
    ContainerLaunchPolicy, ContainerLifecycleAction, ContainerLifecycleRequest,
    ContainerPolicyConfig, LifecyclePolicy, OwnedContainerTarget, ResourceLimits,
    RuntimeBrokerPolicy, RuntimeOperationKind, VolumeLifecycleAction,
};

/// Trusted broker configuration for browser runtime containers.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BrowserRuntimeDockerConfig {
    /// Exact immutable browser runtime image reference.
    pub image: String,
    /// Fixed private runtime network.
    pub network: String,
    /// Fixed shared socket volume.
    pub socket_volume: String,
    /// Prefix for session-owned data volumes.
    pub session_data_volume_prefix: String,
    /// Prefix for reusable browser-context profile volumes.
    pub browser_context_volume_prefix: String,
    /// Prefix for broker-owned browser containers.
    pub container_name_prefix: String,
    /// Socket volume mount target.
    pub socket_mount_root: String,
    /// Directory below the socket mount that contains session sockets.
    pub socket_path_root: String,
    /// Session data volume mount target.
    pub session_data_root: String,
    /// Fixed image command.
    pub command: Vec<String>,
    /// Docker seccomp profile identity used for policy evidence.
    pub seccomp_profile: String,
    /// Enforced browser runtime resource bounds.
    pub resources: ResourceLimits,
}

impl BrowserRuntimeDockerConfig {
    pub(super) fn build_policy(&self) -> Result<RuntimeBrokerPolicy, &'static str> {
        if !is_safe_container_path(&self.socket_mount_root)
            || !is_safe_container_path(&self.socket_path_root)
            || !is_safe_container_path(&self.session_data_root)
            || !self.socket_path_root.starts_with(&format!(
                "{}/",
                self.socket_mount_root.trim_end_matches('/')
            ))
            || self.resources.memory_bytes > i64::MAX as u64
            || self.resources.shm_bytes > i64::MAX as u64
            || self.container_name_prefix.ends_with('-')
            || self.session_data_volume_prefix.ends_with('-')
            || self.browser_context_volume_prefix.ends_with('-')
            || self.session_data_volume_prefix == self.browser_context_volume_prefix
        {
            return Err("browser runtime Docker configuration is invalid");
        }
        let mount_targets = BTreeSet::from([
            self.socket_mount_root.clone(),
            self.session_data_root.clone(),
            self.profile_dir(),
        ]);
        let environment_keys = self
            .base_environment_keys()
            .into_iter()
            .map(str::to_string)
            .collect();
        let launch = ContainerLaunchPolicy {
            image: self.image.clone(),
            container_name_prefix: self.container_name_prefix.clone(),
            network: Some(self.network.clone()),
            volume_prefixes: vec![
                self.session_data_volume_prefix.clone(),
                self.browser_context_volume_prefix.clone(),
            ],
            fixed_volumes: BTreeSet::from([self.socket_volume.clone()]),
            mount_targets,
            require_read_only_mounts: false,
            environment_keys,
            static_labels: BTreeMap::new(),
            entrypoint: self.command.clone(),
            added_capabilities: BTreeSet::new(),
            seccomp_profiles: BTreeSet::from([self.seccomp_profile.clone()]),
            require_read_only_root_filesystem: false,
            maximum_resources: self.resources.clone(),
        };
        let lifecycle = LifecyclePolicy {
            container_name_prefix: self.container_name_prefix.clone(),
            volume_name_prefixes: vec![
                self.session_data_volume_prefix.clone(),
                self.browser_context_volume_prefix.clone(),
            ],
            container_actions: BTreeSet::from([
                ContainerLifecycleAction::Inspect,
                ContainerLifecycleAction::Stop,
                ContainerLifecycleAction::Remove,
            ]),
            volume_actions: BTreeSet::from([
                VolumeLifecycleAction::Inspect,
                VolumeLifecycleAction::Remove,
            ]),
        };
        RuntimeBrokerPolicy::new(ContainerPolicyConfig {
            launch: BTreeMap::from([(RuntimeOperationKind::BrowserRuntime, launch)]),
            lifecycle: BTreeMap::from([(RuntimeOperationKind::BrowserRuntime, lifecycle)]),
        })
        .map_err(|_| "browser runtime Docker policy is invalid")
    }

    pub(super) fn container_name(&self, resource_id: uuid::Uuid) -> String {
        owned_name(&self.container_name_prefix, resource_id)
    }

    pub(super) fn session_data_volume(&self, resource_id: uuid::Uuid) -> String {
        owned_name(&self.session_data_volume_prefix, resource_id)
    }

    pub(super) fn browser_context_volume(&self, resource_id: uuid::Uuid) -> String {
        owned_name(&self.browser_context_volume_prefix, resource_id)
    }

    pub(super) fn socket_path(&self, session_id: uuid::Uuid) -> String {
        format!(
            "{}/{}.sock",
            self.socket_path_root.trim_end_matches('/'),
            session_id
        )
    }

    pub(super) fn profile_dir(&self) -> String {
        format!("{}/chromium", self.session_data_root.trim_end_matches('/'))
    }

    pub(super) fn upload_dir(&self) -> String {
        format!("{}/uploads", self.session_data_root.trim_end_matches('/'))
    }

    pub(super) fn download_dir(&self) -> String {
        format!("{}/downloads", self.session_data_root.trim_end_matches('/'))
    }

    pub(super) fn session_file_mounts_dir(&self) -> String {
        format!("{}/mounts", self.session_data_root.trim_end_matches('/'))
    }

    pub(super) fn session_file_manifest(&self) -> String {
        format!(
            "{}/session-file-bindings.json",
            self.session_data_root.trim_end_matches('/')
        )
    }

    pub(super) fn base_environment_keys(&self) -> [&'static str; 8] {
        [
            "BPANE_SESSION_ID",
            "BPANE_SOCKET_PATH",
            "BPANE_SESSION_DATA_DIR",
            "BPANE_PROFILE_DIR",
            "BPANE_UPLOAD_DIR",
            "BPANE_DOWNLOAD_DIR",
            "BPANE_SESSION_FILE_MOUNTS_DIR",
            "BPANE_SESSION_FILE_BINDINGS_MANIFEST",
        ]
    }

    pub(super) fn lifecycle_target(
        &self,
        request: &ContainerLifecycleRequest,
    ) -> OwnedContainerTarget {
        OwnedContainerTarget {
            operation_kind: request.operation_kind,
            resource_id: request.resource_id,
            container_name: self.container_name(request.resource_id),
            action: request.action,
        }
    }
}

fn owned_name(prefix: &str, resource_id: uuid::Uuid) -> String {
    format!("{}-{}", prefix.trim_end_matches('-'), resource_id.simple())
}

fn is_safe_container_path(value: &str) -> bool {
    value.starts_with('/')
        && value != "/"
        && value.len() <= 512
        && !value
            .bytes()
            .any(|byte| byte == 0 || byte.is_ascii_control())
        && value.split('/').all(|segment| segment != "..")
}
