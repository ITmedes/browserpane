use crate::runtime_manager::{
    DockerRuntimeConfig, PersistedRuntimeAssignment, ResolvedSessionRuntime,
    RuntimeAssignmentStatus, RuntimeManagerConfig, RuntimeManagerError, RuntimeProfile,
    RuntimeSessionAccessInfo, SessionRuntimeManager,
};
use crate::session_control::SessionStore;
use uuid::Uuid;

pub type SessionManagerConfig = RuntimeManagerConfig;
pub type SessionManagerDockerConfig = DockerRuntimeConfig;
pub type SessionManagerError = RuntimeManagerError;
pub type SessionManagerProfile = RuntimeProfile;
pub type SessionRuntime = ResolvedSessionRuntime;
pub type SessionRuntimeAccess = RuntimeSessionAccessInfo;
pub type SessionRuntimeAssignmentStatus = RuntimeAssignmentStatus;
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

    pub async fn attach_session_store(&self, store: SessionStore) {
        self.inner.attach_session_store(store).await;
    }

    pub async fn reconcile_persisted_state(&self) -> Result<(), SessionManagerError> {
        self.inner.reconcile_persisted_state().await
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

    pub async fn resolve(&self, session_id: Uuid) -> Result<SessionRuntime, SessionManagerError> {
        self.inner.resolve(session_id).await
    }

    pub async fn release(&self, session_id: Uuid) {
        self.inner.release(session_id).await;
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
}
