mod container;
mod recovery;
mod resolve;
mod session_files;

#[cfg(test)]
pub(in crate::runtime_manager) use session_files::parse_docker_size_bytes;

use std::collections::HashMap;
use std::sync::Arc;

use tokio::fs;
use tokio::sync::{Mutex, Notify};
use tracing::warn;
use uuid::Uuid;

use super::*;
use crate::auth::AuthenticatedPrincipal;
use crate::credentials::CredentialProvider;
use crate::session_control::{
    EgressProfileState, EgressTrafficObservationMode, SessionBrowserContextMode,
    StoredEgressProfile, StoredSession,
};
use crate::workspaces::WorkspaceFileStore;

pub(super) struct DockerRuntimeManager {
    pub(super) config: DockerRuntimeConfig,
    pub(super) profile: RuntimeProfile,
    pub(super) leases: Mutex<HashMap<Uuid, DockerLeaseState>>,
    pub(super) session_store: Mutex<Option<SessionStore>>,
    pub(super) credential_provider: Mutex<Option<Arc<CredentialProvider>>>,
    pub(super) workspace_file_store: Mutex<Option<Arc<WorkspaceFileStore>>>,
}

pub(super) enum DockerLeaseState {
    Starting {
        lease: RuntimeLease,
        notify: Arc<Notify>,
    },
    Ready(RuntimeLease),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct RuntimeSessionDataScope {
    pub(super) browser_context_id: Option<Uuid>,
    pub(super) discard_session_data_on_release: bool,
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
            credential_provider: Mutex::new(None),
            workspace_file_store: Mutex::new(None),
        })
    }

    pub(super) async fn attach_session_store(&self, store: SessionStore) {
        *self.session_store.lock().await = Some(store);
    }

    async fn session_store(&self) -> Option<SessionStore> {
        self.session_store.lock().await.clone()
    }

    pub(super) async fn attach_credential_provider(
        &self,
        provider: Option<Arc<CredentialProvider>>,
    ) {
        *self.credential_provider.lock().await = provider;
    }

    async fn credential_provider(&self) -> Option<Arc<CredentialProvider>> {
        self.credential_provider.lock().await.clone()
    }

    pub(super) async fn attach_workspace_file_store(&self, store: Arc<WorkspaceFileStore>) {
        *self.workspace_file_store.lock().await = Some(store);
    }

    async fn workspace_file_store(&self) -> Option<Arc<WorkspaceFileStore>> {
        self.workspace_file_store.lock().await.clone()
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

    pub(super) fn browser_context_profile_volume_for_context(&self, context_id: Uuid) -> String {
        format!(
            "{}-browser-context-{}",
            self.config.session_data_volume_prefix.trim_end_matches('-'),
            context_id.as_simple()
        )
    }

    pub(super) fn profile_volume_for_lease(&self, lease: &RuntimeLease) -> Option<String> {
        lease
            .browser_context_id
            .map(|context_id| self.browser_context_profile_volume_for_context(context_id))
    }

    pub(super) fn socket_volume_mount_root(&self) -> String {
        let socket_root = self.config.socket_root.trim_end_matches('/');
        std::path::Path::new(socket_root)
            .parent()
            .and_then(|parent| parent.to_str())
            .filter(|parent| !parent.is_empty() && *parent != "/")
            .unwrap_or(socket_root)
            .to_string()
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

    pub(super) fn session_file_mounts_root(&self) -> String {
        format!("{}/mounts", self.session_data_root())
    }

    pub(super) fn session_file_manifest_path(&self) -> String {
        format!("{}/session-file-bindings.json", self.session_data_root())
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

    pub(super) async fn active_browser_context_session_id(&self, context_id: Uuid) -> Option<Uuid> {
        let leases = self.leases.lock().await;
        active_browser_context_session_id(&leases, context_id)
    }

    pub(super) async fn runtime_data_scope_for_session(
        &self,
        session_id: Uuid,
    ) -> Result<RuntimeSessionDataScope, RuntimeManagerError> {
        let Some(store) = self.session_store().await else {
            return Ok(RuntimeSessionDataScope::default());
        };
        let session = store
            .get_session_by_id(session_id)
            .await
            .map_err(|error| RuntimeManagerError::PersistenceFailed(error.to_string()))?
            .ok_or_else(|| {
                RuntimeManagerError::PersistenceFailed(format!(
                    "session {session_id} not found while resolving docker runtime data scope"
                ))
            })?;
        Self::runtime_data_scope_from_session(&session)
    }

    pub(super) fn runtime_data_scope_from_session(
        session: &StoredSession,
    ) -> Result<RuntimeSessionDataScope, RuntimeManagerError> {
        match session.browser_context.mode {
            SessionBrowserContextMode::Fresh => Ok(RuntimeSessionDataScope::default()),
            SessionBrowserContextMode::Ephemeral => Ok(RuntimeSessionDataScope {
                browser_context_id: None,
                discard_session_data_on_release: true,
            }),
            SessionBrowserContextMode::Reusable => {
                let Some(context_id) = session.browser_context.context_id else {
                    return Err(RuntimeManagerError::PersistenceFailed(format!(
                        "session {} uses reusable browser context mode without a context id",
                        session.id
                    )));
                };
                Ok(RuntimeSessionDataScope {
                    browser_context_id: Some(context_id),
                    discard_session_data_on_release: false,
                })
            }
        }
    }

    pub(super) async fn mark_browser_context_used_for_session(
        &self,
        session_id: Uuid,
        context_id: Uuid,
    ) {
        let Some(store) = self.session_store().await else {
            return;
        };
        let session = match store.get_session_by_id(session_id).await {
            Ok(Some(session)) => session,
            Ok(None) => {
                warn!(
                    session_id = %session_id,
                    browser_context_id = %context_id,
                    "could not mark browser context used because session was not found",
                );
                return;
            }
            Err(error) => {
                warn!(
                    session_id = %session_id,
                    browser_context_id = %context_id,
                    error = %error,
                    "could not load session while marking browser context used",
                );
                return;
            }
        };
        let principal = AuthenticatedPrincipal {
            subject: session.owner.subject,
            issuer: session.owner.issuer,
            display_name: session.owner.display_name,
            client_id: None,
            safe_claims: Default::default(),
        };
        match store
            .mark_browser_context_used_for_owner(&principal, context_id)
            .await
        {
            Ok(Some(_)) => {}
            Ok(None) => {
                warn!(
                    session_id = %session_id,
                    browser_context_id = %context_id,
                    "could not mark browser context used because it was not found",
                );
            }
            Err(error) => {
                warn!(
                    session_id = %session_id,
                    browser_context_id = %context_id,
                    error = %error,
                    "could not mark browser context used",
                );
            }
        }
    }

    pub(super) async fn delete_browser_context_data(
        &self,
        context_id: Uuid,
    ) -> Result<(), RuntimeManagerError> {
        let active_session_id = {
            let leases = self.leases.lock().await;
            active_browser_context_session_id(&leases, context_id)
        };
        if let Some(active_session_id) = active_session_id {
            return Err(RuntimeManagerError::BrowserContextInUse {
                browser_context_id: context_id,
                active_session_id,
            });
        }

        self.remove_browser_context_profile_volume(context_id).await
    }

    pub(super) async fn clone_browser_context_data(
        &self,
        source_context_id: Uuid,
        target_context_id: Uuid,
    ) -> Result<(), RuntimeManagerError> {
        let active_context = {
            let leases = self.leases.lock().await;
            active_browser_context_session_id(&leases, source_context_id)
                .map(|session_id| (source_context_id, session_id))
                .or_else(|| {
                    active_browser_context_session_id(&leases, target_context_id)
                        .map(|session_id| (target_context_id, session_id))
                })
        };
        if let Some((active_context_id, active_session_id)) = active_context {
            return Err(RuntimeManagerError::BrowserContextInUse {
                browser_context_id: active_context_id,
                active_session_id,
            });
        }

        self.clone_browser_context_profile_volume(source_context_id, target_context_id)
            .await
    }

    pub(super) async fn export_browser_context_profile_archive(
        &self,
        context_id: Uuid,
    ) -> Result<Option<Vec<u8>>, RuntimeManagerError> {
        let active_session_id = {
            let leases = self.leases.lock().await;
            active_browser_context_session_id(&leases, context_id)
        };
        if let Some(active_session_id) = active_session_id {
            return Err(RuntimeManagerError::BrowserContextInUse {
                browser_context_id: context_id,
                active_session_id,
            });
        }

        self.export_browser_context_profile_volume_archive(context_id)
            .await
    }

    pub(super) async fn import_browser_context_profile_archive(
        &self,
        context_id: Uuid,
        profile_archive: Option<&[u8]>,
    ) -> Result<(), RuntimeManagerError> {
        let active_session_id = {
            let leases = self.leases.lock().await;
            active_browser_context_session_id(&leases, context_id)
        };
        if let Some(active_session_id) = active_session_id {
            return Err(RuntimeManagerError::BrowserContextInUse {
                browser_context_id: context_id,
                active_session_id,
            });
        }

        self.import_browser_context_profile_volume_archive(context_id, profile_archive)
            .await
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

fn active_browser_context_session_id(
    leases: &HashMap<Uuid, DockerLeaseState>,
    context_id: Uuid,
) -> Option<Uuid> {
    leases.iter().find_map(|(session_id, state)| {
        (state.lease().browser_context_id == Some(context_id)).then_some(*session_id)
    })
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
