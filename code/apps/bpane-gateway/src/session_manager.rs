use crate::credentials::CredentialProvider;
use crate::runtime_manager::{
    BrokerRuntimeConfig, DockerRuntimeConfig, PersistedRuntimeAssignment, ResolvedSessionRuntime,
    RuntimeAssignmentStatus, RuntimeCapacitySnapshot, RuntimeManagerConfig, RuntimeManagerError,
    RuntimeProfile, RuntimeSessionAccessInfo, SessionRuntimeManager,
};
use crate::session_control::SessionStore;
use crate::workspaces::WorkspaceFileStore;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use bpane_runtime_client::RuntimeBrokerClient;

pub type SessionManagerConfig = RuntimeManagerConfig;
pub type SessionManagerDockerConfig = DockerRuntimeConfig;
pub type SessionManagerBrokerConfig = BrokerRuntimeConfig;
pub type SessionManagerError = RuntimeManagerError;
pub type SessionManagerProfile = RuntimeProfile;
pub type SessionRuntime = ResolvedSessionRuntime;
pub type SessionRuntimeAccess = RuntimeSessionAccessInfo;
pub type SessionRuntimeAssignmentStatus = RuntimeAssignmentStatus;
pub type SessionRuntimeCapacity = RuntimeCapacitySnapshot;
pub type PersistedSessionRuntimeAssignment = PersistedRuntimeAssignment;

/// Internal gateway boundary for session runtime lifecycle.
///
/// `SessionManager` is the only runtime-lifecycle surface that the rest of the
/// gateway should depend on. The concrete worker startup and routing
/// implementation remains in `runtime_manager.rs`.
#[derive(Clone)]
pub struct SessionManager {
    inner: SessionRuntimeManager,
}

impl SessionManager {
    pub fn new(config: SessionManagerConfig) -> Result<Self, SessionManagerError> {
        Ok(Self {
            inner: SessionRuntimeManager::new(config)?,
        })
    }

    pub fn profile(&self) -> &SessionManagerProfile {
        self.inner.profile()
    }

    pub async fn capacity_snapshot(&self) -> SessionRuntimeCapacity {
        self.inner.capacity_snapshot().await
    }

    pub(crate) fn runtime_broker_client(&self) -> Option<Arc<dyn RuntimeBrokerClient>> {
        self.inner.runtime_broker_client()
    }

    pub async fn attach_session_store(&self, store: SessionStore) {
        self.inner.attach_session_store(store).await;
    }

    pub async fn attach_credential_provider(&self, provider: Option<Arc<CredentialProvider>>) {
        self.inner.attach_credential_provider(provider).await;
    }

    pub async fn attach_workspace_file_store(&self, store: Arc<WorkspaceFileStore>) {
        self.inner.attach_workspace_file_store(store).await;
    }

    pub async fn reconcile_persisted_state(&self) -> Result<(), SessionManagerError> {
        self.inner.reconcile_persisted_state().await
    }

    pub async fn check_readiness(&self) -> Result<(), SessionManagerError> {
        self.inner.check_readiness().await
    }

    pub fn describe_session_runtime(&self, session_id: Uuid) -> SessionRuntimeAccess {
        self.inner.describe_session_runtime(session_id)
    }

    pub async fn describe_session_runtime_assignment_status(
        &self,
        session_id: Uuid,
    ) -> Option<SessionRuntimeAssignmentStatus> {
        self.inner
            .describe_session_runtime_assignment_status(session_id)
            .await
    }

    pub async fn active_browser_context_session_id(&self, context_id: Uuid) -> Option<Uuid> {
        self.inner
            .active_browser_context_session_id(context_id)
            .await
    }

    pub async fn resolve(&self, session_id: Uuid) -> Result<SessionRuntime, SessionManagerError> {
        self.inner.resolve(session_id).await
    }

    pub async fn release(&self, session_id: Uuid) {
        self.inner.release(session_id).await;
    }

    pub async fn delete_browser_context_data(
        &self,
        context_id: Uuid,
    ) -> Result<(), SessionManagerError> {
        self.inner.delete_browser_context_data(context_id).await
    }

    pub async fn clone_browser_context_data(
        &self,
        source_context_id: Uuid,
        target_context_id: Uuid,
    ) -> Result<(), SessionManagerError> {
        self.inner
            .clone_browser_context_data(source_context_id, target_context_id)
            .await
    }

    pub async fn export_browser_context_profile_archive(
        &self,
        context_id: Uuid,
    ) -> Result<Option<Vec<u8>>, SessionManagerError> {
        self.inner
            .export_browser_context_profile_archive(context_id)
            .await
    }

    pub async fn import_browser_context_profile_archive(
        &self,
        context_id: Uuid,
        profile_archive: Option<&[u8]>,
    ) -> Result<(), SessionManagerError> {
        self.inner
            .import_browser_context_profile_archive(context_id, profile_archive)
            .await
    }

    pub async fn browser_context_profile_storage_bytes(
        &self,
        context_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, u64>, SessionManagerError> {
        self.inner
            .browser_context_profile_storage_bytes(context_ids)
            .await
    }

    pub async fn mark_session_active(&self, session_id: Uuid) {
        self.inner.mark_session_active(session_id).await;
    }

    pub async fn mark_session_idle(&self, session_id: Uuid) {
        self.inner.mark_session_idle(session_id).await;
    }
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::time::Duration;

    use super::*;

    fn docker_config() -> SessionManagerDockerConfig {
        SessionManagerDockerConfig {
            docker_bin: "docker".to_string(),
            image: "deploy-host".to_string(),
            network: "deploy_bpane-internal".to_string(),
            socket_volume: "deploy_agent-socket".to_string(),
            session_data_volume_prefix: "deploy_bpane-session-data".to_string(),
            container_name_prefix: "bpane-runtime".to_string(),
            socket_root: "/run/bpane/sessions".to_string(),
            session_data_root: "/run/bpane/session".to_string(),
            cdp_proxy_port: 9223,
            shm_size: "128m".to_string(),
            start_timeout: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(300),
            max_active_runtimes: 2,
            max_starting_runtimes: 1,
            seccomp_unconfined: true,
            env_file: None,
        }
    }

    #[tokio::test]
    async fn session_manager_reuses_runtime_for_same_static_session() {
        let manager = SessionManager::new(SessionManagerConfig::StaticSingle {
            agent_socket_path: "/tmp/bpane.sock".to_string(),
            cdp_endpoint: Some("http://host:9223".to_string()),
            idle_timeout: Duration::from_secs(300),
        })
        .unwrap();
        let session_id = Uuid::now_v7();

        let first = manager.resolve(session_id).await.unwrap();
        let second = manager.resolve(session_id).await.unwrap();

        assert_eq!(first, second);
        assert_eq!(
            manager.profile().compatibility_mode,
            "legacy_single_runtime"
        );
        assert_eq!(
            manager
                .describe_session_runtime(session_id)
                .cdp_endpoint
                .as_deref(),
            Some("http://host:9223")
        );

        assert_eq!(
            manager.capacity_snapshot().await,
            SessionRuntimeCapacity {
                active_assignments: 1,
                starting_assignments: 0,
                assignment_limit: 1,
            }
        );

        manager.release(session_id).await;
        assert_eq!(manager.capacity_snapshot().await.active_assignments, 0);
    }

    #[test]
    fn session_manager_exposes_docker_pool_capacity_contract() {
        let manager =
            SessionManager::new(SessionManagerConfig::DockerPool(docker_config())).unwrap();

        assert_eq!(manager.profile().compatibility_mode, "session_runtime_pool");
        assert_eq!(manager.profile().max_runtime_sessions, 2);
        assert!(!manager.profile().supports_legacy_global_routes);
        assert_eq!(
            manager
                .describe_session_runtime(Uuid::nil())
                .cdp_endpoint
                .as_deref(),
            Some("http://bpane-runtime-00000000000000000000000000000000:9223")
        );
        assert!(manager.runtime_broker_client().is_none());
    }

    #[test]
    fn session_manager_exposes_the_shared_broker_client_only_for_broker_pool() {
        let manager = SessionManager::new(SessionManagerConfig::BrokerPool(
            SessionManagerBrokerConfig {
                docker: docker_config(),
                base_url: "http://runtime-broker:9070".to_string(),
                token_url: "http://keycloak:8080/realms/browserpane/token".to_string(),
                client_id: "bpane-gateway".to_string(),
                client_secret: bpane_runtime_contract::SecretValue::new("broker-secret").unwrap(),
                request_timeout: Duration::from_secs(5),
                max_response_bytes: 65_536,
            },
        ))
        .unwrap();

        assert!(manager.runtime_broker_client().is_some());
    }

    #[tokio::test]
    async fn session_manager_reconcile_is_exposed_as_a_boundary_operation() {
        let manager = SessionManager::new(SessionManagerConfig::StaticSingle {
            agent_socket_path: "/tmp/bpane.sock".to_string(),
            cdp_endpoint: None,
            idle_timeout: Duration::from_secs(300),
        })
        .unwrap();

        manager.reconcile_persisted_state().await.unwrap();
    }

    #[tokio::test]
    async fn static_runtime_readiness_requires_agent_socket() {
        let temp_dir = tempfile::tempdir().unwrap();
        let socket_path = temp_dir.path().join("agent.sock");
        let manager = SessionManager::new(SessionManagerConfig::StaticSingle {
            agent_socket_path: socket_path.to_string_lossy().into_owned(),
            cdp_endpoint: None,
            idle_timeout: Duration::from_secs(300),
        })
        .unwrap();

        assert!(manager.check_readiness().await.is_err());
        let _listener = tokio::net::UnixListener::bind(&socket_path).unwrap();
        manager.check_readiness().await.unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn docker_runtime_readiness_uses_daemon_probe() {
        let temp_dir = tempfile::tempdir().unwrap();
        let docker_bin = temp_dir.path().join("docker");
        std::fs::write(
            &docker_bin,
            "#!/bin/sh\n[ \"$1\" = info ] && printf 'test-version' && exit 0\nexit 1\n",
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&docker_bin).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&docker_bin, permissions).unwrap();
        let mut config = docker_config();
        config.docker_bin = docker_bin.to_string_lossy().into_owned();
        let manager = SessionManager::new(SessionManagerConfig::DockerPool(config)).unwrap();

        manager.check_readiness().await.unwrap();
    }
}
