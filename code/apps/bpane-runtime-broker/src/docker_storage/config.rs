use std::collections::{BTreeMap, BTreeSet};

use bpane_runtime_contract::{
    ContainerLaunchPolicy, ContainerPolicyConfig, ContainerSecurity, ResourceLimits,
    RuntimeBrokerPolicy, RuntimeOperationKind,
};

use super::STORAGE_HELPER_SCRIPT;

/// Trusted broker configuration for isolated storage helper containers.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct StorageRuntimeDockerConfig {
    pub image: String,
    pub session_data_volume_prefix: String,
    pub browser_context_volume_prefix: String,
    pub container_name_prefix: String,
    pub seccomp_profile: String,
    pub resources: ResourceLimits,
    pub max_payload_bytes: usize,
    pub max_archive_entries: usize,
    pub max_archive_path_bytes: usize,
    pub max_archive_uncompressed_bytes: u64,
}

impl StorageRuntimeDockerConfig {
    pub(super) fn build_policy(&self) -> Result<RuntimeBrokerPolicy, &'static str> {
        if self.session_data_volume_prefix == self.browser_context_volume_prefix
            || self.container_name_prefix.ends_with('-')
            || self.session_data_volume_prefix.ends_with('-')
            || self.browser_context_volume_prefix.ends_with('-')
            || self.resources.memory_bytes > i64::MAX as u64
            || self.resources.shm_bytes > i64::MAX as u64
            || self.max_payload_bytes == 0
            || self.max_payload_bytes > 1_073_741_824
            || self.max_archive_entries == 0
            || self.max_archive_path_bytes == 0
            || self.max_archive_path_bytes > 16_384
            || self.max_archive_uncompressed_bytes == 0
        {
            return Err("storage helper Docker configuration is invalid");
        }
        let launch = ContainerLaunchPolicy {
            image: self.image.clone(),
            container_name_prefix: self.container_name_prefix.clone(),
            network: None,
            volume_prefixes: vec![
                self.session_data_volume_prefix.clone(),
                self.browser_context_volume_prefix.clone(),
            ],
            fixed_volumes: BTreeSet::new(),
            mount_targets: BTreeSet::from([
                self.session_mount_root().to_string(),
                self.profile_dir(),
                self.source_mount_root().to_string(),
                self.target_mount_root().to_string(),
            ]),
            require_read_only_mounts: false,
            environment_keys: storage_environment_keys(),
            static_labels: BTreeMap::new(),
            derived_label_keys: BTreeSet::from([
                "browserpane.storage.action".to_string(),
                "browserpane.storage.target_id".to_string(),
            ]),
            entrypoint: self.command(),
            added_capabilities: BTreeSet::new(),
            seccomp_profiles: BTreeSet::from([self.seccomp_profile.clone()]),
            require_read_only_root_filesystem: true,
            maximum_resources: self.resources.clone(),
        };
        RuntimeBrokerPolicy::new(ContainerPolicyConfig {
            launch: BTreeMap::from([(RuntimeOperationKind::StorageHelper, launch)]),
            lifecycle: BTreeMap::new(),
        })
        .map_err(|_| "storage helper Docker policy is invalid")
    }

    pub(super) fn command(&self) -> Vec<String> {
        vec![
            "/bin/sh".to_string(),
            "-ec".to_string(),
            STORAGE_HELPER_SCRIPT.to_string(),
        ]
    }

    pub(super) fn container_name(&self, request_id: uuid::Uuid) -> String {
        owned_name(&self.container_name_prefix, request_id)
    }

    pub(super) fn session_data_volume(&self, session_id: uuid::Uuid) -> String {
        owned_name(&self.session_data_volume_prefix, session_id)
    }

    pub(super) fn context_volume(&self, context_id: uuid::Uuid) -> String {
        owned_name(&self.browser_context_volume_prefix, context_id)
    }

    pub(super) const fn session_mount_root(&self) -> &'static str {
        "/run/bpane/storage-helper/session"
    }

    pub(super) const fn source_mount_root(&self) -> &'static str {
        "/run/bpane/storage-helper/source"
    }

    pub(super) const fn target_mount_root(&self) -> &'static str {
        "/run/bpane/storage-helper/target"
    }

    pub(super) fn profile_dir(&self) -> String {
        format!("{}/chromium", self.session_mount_root())
    }

    pub(super) fn upload_dir(&self) -> String {
        format!("{}/uploads", self.session_mount_root())
    }

    pub(super) fn download_dir(&self) -> String {
        format!("{}/downloads", self.session_mount_root())
    }

    pub(super) fn mounts_dir(&self) -> String {
        format!("{}/mounts", self.session_mount_root())
    }

    pub(super) fn manifest_path(&self) -> String {
        format!("{}/session-file-bindings.json", self.session_mount_root())
    }

    pub(super) fn proxy_auth_path(&self) -> String {
        format!("{}/egress/proxy-auth.json", self.session_mount_root())
    }

    pub(super) fn trusted_ca_path(&self) -> String {
        format!("{}/egress/custom-ca.pem", self.session_mount_root())
    }

    pub(super) fn policy_security(&self) -> ContainerSecurity {
        ContainerSecurity {
            privileged: false,
            host_network: false,
            host_pid: false,
            host_ipc: false,
            devices: Vec::new(),
            added_capabilities: BTreeSet::new(),
            no_new_privileges: true,
            read_only_root_filesystem: true,
            seccomp_profile: self.seccomp_profile.clone(),
        }
    }
}

fn storage_environment_keys() -> BTreeSet<String> {
    [
        "BPANE_STORAGE_ACTION",
        "BPANE_STORAGE_INPUT_BYTES",
        "BPANE_SESSION_DATA_DIR",
        "BPANE_PROFILE_DIR",
        "BPANE_UPLOAD_DIR",
        "BPANE_DOWNLOAD_DIR",
        "BPANE_SESSION_FILE_MOUNTS_DIR",
        "BPANE_MATERIALIZE_TARGET",
        "BPANE_MATERIALIZE_MODE",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn owned_name(prefix: &str, id: uuid::Uuid) -> String {
    format!("{}-{}", prefix.trim_end_matches('-'), id.simple())
}
