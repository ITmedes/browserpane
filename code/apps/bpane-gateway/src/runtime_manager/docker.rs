mod container;
mod recovery;
mod resolve;

use std::collections::HashMap;
use std::sync::Arc;

use tokio::fs;
use tokio::sync::{Mutex, Notify};
use uuid::Uuid;

use super::*;

pub(super) struct DockerRuntimeManager {
    pub(super) config: DockerRuntimeConfig,
    pub(super) profile: RuntimeProfile,
    pub(super) leases: Mutex<HashMap<Uuid, DockerLeaseState>>,
    pub(super) session_store: Mutex<Option<SessionStore>>,
}

pub(super) enum DockerLeaseState {
    Starting {
        lease: RuntimeLease,
        notify: Arc<Notify>,
    },
    Ready(RuntimeLease),
}

impl DockerRuntimeManager {
    pub(super) fn new(
        config: DockerRuntimeConfig,
        profile: RuntimeProfile,
    ) -> Result<Self, RuntimeManagerError> {
        if config.image.trim().is_empty() {
            return Err(RuntimeManagerError::InvalidConfiguration(
                "docker runtime backend requires a non-empty image".to_string(),
            ));
        }
        if config.network.trim().is_empty() {
            return Err(RuntimeManagerError::InvalidConfiguration(
                "docker runtime backend requires a non-empty network".to_string(),
            ));
        }
        if config.socket_volume.trim().is_empty() {
            return Err(RuntimeManagerError::InvalidConfiguration(
                "docker runtime backend requires a non-empty socket_volume".to_string(),
            ));
        }
        if config
            .session_data_volume_prefix
            .trim()
            .trim_end_matches('-')
            .is_empty()
        {
            return Err(RuntimeManagerError::InvalidConfiguration(
                "docker runtime backend requires a non-empty session_data_volume_prefix"
                    .to_string(),
            ));
        }
        if config.container_name_prefix.trim().is_empty() {
            return Err(RuntimeManagerError::InvalidConfiguration(
                "docker runtime backend requires a non-empty container_name_prefix".to_string(),
            ));
        }
        let socket_root = config.socket_root.trim();
        if socket_root.is_empty() {
            return Err(RuntimeManagerError::InvalidConfiguration(
                "docker runtime backend requires a non-empty socket_root".to_string(),
            ));
        }
        if !socket_root.starts_with('/') {
            return Err(RuntimeManagerError::InvalidConfiguration(
                "docker runtime backend requires an absolute socket_root".to_string(),
            ));
        }
        if socket_root.trim_end_matches('/').is_empty() {
            return Err(RuntimeManagerError::InvalidConfiguration(
                "docker runtime backend requires socket_root below /".to_string(),
            ));
        }
        let session_data_root = config.session_data_root.trim();
        if session_data_root.is_empty() {
            return Err(RuntimeManagerError::InvalidConfiguration(
                "docker runtime backend requires a non-empty session_data_root".to_string(),
            ));
        }
        if !session_data_root.starts_with('/') {
            return Err(RuntimeManagerError::InvalidConfiguration(
                "docker runtime backend requires an absolute session_data_root".to_string(),
            ));
        }
        if session_data_root.trim_end_matches('/').is_empty() {
            return Err(RuntimeManagerError::InvalidConfiguration(
                "docker runtime backend requires session_data_root below /".to_string(),
            ));
        }
        if config.max_active_runtimes == 0 {
            return Err(RuntimeManagerError::InvalidConfiguration(
                "docker runtime backend requires max_active_runtimes >= 1".to_string(),
            ));
        }
        if config.max_starting_runtimes == 0 {
            return Err(RuntimeManagerError::InvalidConfiguration(
                "docker runtime backend requires max_starting_runtimes >= 1".to_string(),
            ));
        }
        if config.max_starting_runtimes > config.max_active_runtimes {
            return Err(RuntimeManagerError::InvalidConfiguration(
                "docker runtime backend requires max_starting_runtimes <= max_active_runtimes"
                    .to_string(),
            ));
        }

        Ok(Self {
            config,
            profile,
            leases: Mutex::new(HashMap::new()),
            session_store: Mutex::new(None),
        })
    }

    pub(super) async fn attach_session_store(&self, store: SessionStore) {
        *self.session_store.lock().await = Some(store);
    }

    async fn session_store(&self) -> Option<SessionStore> {
        self.session_store.lock().await.clone()
    }

    pub(super) fn socket_path_for_session(&self, session_id: Uuid) -> String {
        format!(
            "{}/{}.sock",
            self.config.socket_root.trim_end_matches('/'),
            session_id
        )
    }

    pub(super) fn session_data_volume_for_session(&self, session_id: Uuid) -> String {
        format!(
            "{}-{}",
            self.config.session_data_volume_prefix.trim_end_matches('-'),
            session_id.as_simple()
        )
    }

    pub(super) fn session_data_root(&self) -> &str {
        self.config.session_data_root.trim_end_matches('/')
    }

    pub(super) fn profile_dir_for_session(&self) -> String {
        format!("{}/chromium", self.session_data_root())
    }

    pub(super) fn upload_dir_for_session(&self) -> String {
        format!("{}/uploads", self.session_data_root())
    }

    pub(super) fn download_dir_for_session(&self) -> String {
        format!("{}/downloads", self.session_data_root())
    }

    pub(super) fn container_name_for_session(&self, session_id: Uuid) -> String {
        format!(
            "{}-{}",
            self.config.container_name_prefix.trim_end_matches('-'),
            session_id.as_simple()
        )
    }

    pub(super) fn cdp_endpoint_for_session(&self, session_id: Uuid) -> String {
        format!(
            "http://{}:{}",
            self.container_name_for_session(session_id),
            self.config.cdp_proxy_port
        )
    }

    pub(super) fn describe_runtime(&self, session_id: Uuid) -> RuntimeSessionAccessInfo {
        RuntimeSessionAccessInfo {
            binding: self.profile.runtime_binding.clone(),
            compatibility_mode: self.profile.compatibility_mode.clone(),
            cdp_endpoint: Some(self.cdp_endpoint_for_session(session_id)),
        }
    }

    pub(super) async fn describe_assignment_status(
        &self,
        session_id: Uuid,
    ) -> Option<RuntimeAssignmentStatus> {
        let leases = self.leases.lock().await;
        match leases.get(&session_id) {
            Some(DockerLeaseState::Starting { .. }) => Some(RuntimeAssignmentStatus::Starting),
            Some(DockerLeaseState::Ready(_)) => Some(RuntimeAssignmentStatus::Ready),
            None => None,
        }
    }
}

impl DockerLeaseState {
    fn lease(&self) -> &RuntimeLease {
        match self {
            Self::Starting { lease, .. } => lease,
            Self::Ready(lease) => lease,
        }
    }
}

fn sorted_active_session_ids(leases: &HashMap<Uuid, DockerLeaseState>) -> Vec<Uuid> {
    let mut ids = leases.keys().copied().collect::<Vec<_>>();
    ids.sort_unstable();
    ids
}

async fn remove_socket_path(socket_path: &str) -> Result<(), RuntimeManagerError> {
    match fs::remove_file(socket_path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(RuntimeManagerError::StartupFailed(format!(
            "failed to remove stale runtime socket {socket_path}: {error}"
        ))),
    }
}
