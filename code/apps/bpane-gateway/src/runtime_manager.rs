use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use uuid::Uuid;

use crate::session_control::SessionStore;

mod docker;
mod static_single;

use docker::DockerRuntimeManager;
use static_single::StaticSingleRuntimeManager;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSessionRuntime {
    pub session_id: Uuid,
    pub agent_socket_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSessionAccessInfo {
    pub binding: String,
    pub compatibility_mode: String,
    pub cdp_endpoint: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeAssignmentStatus {
    Starting,
    Ready,
}

impl RuntimeAssignmentStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Ready => "ready",
        }
    }
}

impl std::str::FromStr for RuntimeAssignmentStatus {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "starting" => Ok(Self::Starting),
            "ready" => Ok(Self::Ready),
            _ => Err("unknown runtime assignment status"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedRuntimeAssignment {
    pub session_id: Uuid,
    pub runtime_binding: String,
    pub status: RuntimeAssignmentStatus,
    pub agent_socket_path: String,
    pub container_name: Option<String>,
    pub cdp_endpoint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeProfile {
    pub runtime_binding: String,
    pub compatibility_mode: String,
    pub max_runtime_sessions: usize,
    pub supports_legacy_global_routes: bool,
    pub supports_session_extensions: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeManagerError {
    RuntimeBusy {
        active_session_id: Uuid,
    },
    RuntimeCapacityReached {
        max_active_runtimes: usize,
        active_session_ids: Vec<Uuid>,
    },
    RuntimeStartupCapacityReached {
        max_starting_runtimes: usize,
    },
    InvalidConfiguration(String),
    StartupFailed(String),
    PersistenceFailed(String),
}

impl fmt::Display for RuntimeManagerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RuntimeBusy { active_session_id } => write!(
                f,
                "the current gateway runtime is already assigned to active session {active_session_id}"
            ),
            Self::RuntimeCapacityReached {
                max_active_runtimes,
                active_session_ids,
            } => write!(
                f,
                "runtime capacity reached: {} active runtime-backed sessions already exist ({})",
                max_active_runtimes,
                active_session_ids
                    .iter()
                    .map(Uuid::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::RuntimeStartupCapacityReached {
                max_starting_runtimes,
            } => write!(
                f,
                "runtime startup capacity reached: {} runtime workers are already starting",
                max_starting_runtimes
            ),
            Self::InvalidConfiguration(message) => write!(f, "{message}"),
            Self::StartupFailed(message) => write!(f, "{message}"),
            Self::PersistenceFailed(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for RuntimeManagerError {}

#[derive(Debug, Clone)]
pub enum RuntimeManagerConfig {
    StaticSingle {
        agent_socket_path: String,
        cdp_endpoint: Option<String>,
        idle_timeout: Duration,
    },
    DockerSingle(DockerRuntimeConfig),
    DockerPool(DockerRuntimeConfig),
}

#[derive(Debug, Clone)]
pub struct DockerRuntimeConfig {
    pub docker_bin: String,
    pub image: String,
    pub network: String,
    pub shared_run_volume: String,
    pub container_name_prefix: String,
    pub socket_root: String,
    pub cdp_proxy_port: u16,
    pub shm_size: String,
    pub start_timeout: Duration,
    pub idle_timeout: Duration,
    pub max_active_runtimes: usize,
    pub max_starting_runtimes: usize,
    pub seccomp_unconfined: bool,
    pub env_file: Option<PathBuf>,
}

#[derive(Clone)]
pub struct SessionRuntimeManager {
    backend: RuntimeBackend,
    profile: RuntimeProfile,
}

#[derive(Clone)]
enum RuntimeBackend {
    StaticSingle(Arc<StaticSingleRuntimeManager>),
    Docker(Arc<DockerRuntimeManager>),
}

impl SessionRuntimeManager {
    pub fn new(config: RuntimeManagerConfig) -> Result<Self, RuntimeManagerError> {
        match config {
            RuntimeManagerConfig::StaticSingle {
                agent_socket_path,
                cdp_endpoint,
                idle_timeout,
            } => {
                let profile = RuntimeProfile {
                    runtime_binding: "legacy_single_session".to_string(),
                    compatibility_mode: "legacy_single_runtime".to_string(),
                    max_runtime_sessions: 1,
                    supports_legacy_global_routes: true,
                    supports_session_extensions: false,
                };
                Ok(Self {
                    backend: RuntimeBackend::StaticSingle(Arc::new(
                        StaticSingleRuntimeManager::new(
                            agent_socket_path,
                            cdp_endpoint,
                            idle_timeout,
                        ),
                    )),
                    profile,
                })
            }
            RuntimeManagerConfig::DockerSingle(config) => {
                let profile = RuntimeProfile {
                    runtime_binding: "legacy_single_session".to_string(),
                    compatibility_mode: "legacy_single_runtime".to_string(),
                    max_runtime_sessions: 1,
                    supports_legacy_global_routes: true,
                    supports_session_extensions: true,
                };
                let manager = DockerRuntimeManager::new(
                    DockerRuntimeConfig {
                        max_active_runtimes: 1,
                        max_starting_runtimes: 1,
                        ..config
                    },
                    profile.clone(),
                )?;
                Ok(Self {
                    backend: RuntimeBackend::Docker(Arc::new(manager)),
                    profile,
                })
            }
            RuntimeManagerConfig::DockerPool(config) => {
                let max_runtime_sessions = config.max_active_runtimes;
                let profile = RuntimeProfile {
                    runtime_binding: "docker_runtime_pool".to_string(),
                    compatibility_mode: "session_runtime_pool".to_string(),
                    max_runtime_sessions,
                    supports_legacy_global_routes: false,
                    supports_session_extensions: true,
                };
                let manager = DockerRuntimeManager::new(config, profile.clone())?;
                Ok(Self {
                    backend: RuntimeBackend::Docker(Arc::new(manager)),
                    profile,
                })
            }
        }
    }

    pub fn profile(&self) -> &RuntimeProfile {
        &self.profile
    }

    pub async fn attach_session_store(&self, store: SessionStore) {
        match &self.backend {
            RuntimeBackend::StaticSingle(_) => {}
            RuntimeBackend::Docker(manager) => manager.attach_session_store(store).await,
        }
    }

    pub async fn reconcile_persisted_state(&self) -> Result<(), RuntimeManagerError> {
        match &self.backend {
            RuntimeBackend::StaticSingle(_) => Ok(()),
            RuntimeBackend::Docker(manager) => manager.reconcile_persisted_state().await,
        }
    }

    pub fn describe_session_runtime(&self, session_id: Uuid) -> RuntimeSessionAccessInfo {
        match &self.backend {
            RuntimeBackend::StaticSingle(manager) => manager.describe_runtime(self.profile()),
            RuntimeBackend::Docker(manager) => manager.describe_runtime(session_id),
        }
    }

    pub async fn resolve(
        &self,
        session_id: Uuid,
    ) -> Result<ResolvedSessionRuntime, RuntimeManagerError> {
        match &self.backend {
            RuntimeBackend::StaticSingle(manager) => manager.resolve(session_id).await,
            RuntimeBackend::Docker(manager) => manager.resolve(session_id).await,
        }
    }

    pub async fn release(&self, session_id: Uuid) {
        match &self.backend {
            RuntimeBackend::StaticSingle(manager) => manager.release(session_id).await,
            RuntimeBackend::Docker(manager) => manager.release(session_id).await,
        }
    }

    pub async fn mark_session_active(&self, session_id: Uuid) {
        match &self.backend {
            RuntimeBackend::StaticSingle(manager) => manager.mark_session_active(session_id).await,
            RuntimeBackend::Docker(manager) => manager.mark_session_active(session_id).await,
        }
    }

    pub async fn mark_session_idle(&self, session_id: Uuid) {
        match &self.backend {
            RuntimeBackend::StaticSingle(manager) => manager.mark_session_idle(session_id).await,
            RuntimeBackend::Docker(manager) => manager.mark_session_idle(session_id).await,
        }
    }
}

#[derive(Debug, Clone)]
struct RuntimeLease {
    session_id: Uuid,
    agent_socket_path: String,
    container_name: Option<String>,
    idle_generation: u64,
}

fn bump_idle_generation(lease: &mut RuntimeLease) {
    lease.idle_generation = lease.idle_generation.saturating_add(1);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn docker_config() -> DockerRuntimeConfig {
        DockerRuntimeConfig {
            docker_bin: "docker".to_string(),
            image: "deploy-host".to_string(),
            network: "deploy_bpane-internal".to_string(),
            shared_run_volume: "deploy_agent-socket".to_string(),
            container_name_prefix: "bpane-runtime".to_string(),
            socket_root: "/run/bpane/sessions".to_string(),
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
    async fn static_single_runtime_reuses_same_session_assignment() {
        let manager = SessionRuntimeManager::new(RuntimeManagerConfig::StaticSingle {
            agent_socket_path: "/tmp/bpane.sock".to_string(),
            cdp_endpoint: Some("http://host:9223".to_string()),
            idle_timeout: Duration::from_secs(300),
        })
        .unwrap();
        let session_id = Uuid::now_v7();

        let first = manager.resolve(session_id).await.unwrap();
        let second = manager.resolve(session_id).await.unwrap();

        assert_eq!(first, second);
        assert_eq!(first.agent_socket_path, "/tmp/bpane.sock");
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

    #[tokio::test]
    async fn static_single_runtime_blocks_parallel_session_assignment() {
        let manager = SessionRuntimeManager::new(RuntimeManagerConfig::StaticSingle {
            agent_socket_path: "/tmp/bpane.sock".to_string(),
            cdp_endpoint: None,
            idle_timeout: Duration::from_secs(300),
        })
        .unwrap();
        let session_a = Uuid::now_v7();
        let session_b = Uuid::now_v7();

        manager.resolve(session_a).await.unwrap();
        let error = manager.resolve(session_b).await.unwrap_err();

        assert_eq!(
            error,
            RuntimeManagerError::RuntimeBusy {
                active_session_id: session_a,
            }
        );
    }

    #[tokio::test]
    async fn static_single_runtime_release_allows_next_session() {
        let manager = SessionRuntimeManager::new(RuntimeManagerConfig::StaticSingle {
            agent_socket_path: "/tmp/bpane.sock".to_string(),
            cdp_endpoint: None,
            idle_timeout: Duration::from_secs(300),
        })
        .unwrap();
        let session_a = Uuid::now_v7();
        let session_b = Uuid::now_v7();

        manager.resolve(session_a).await.unwrap();
        manager.release(session_a).await;
        let resolved = manager.resolve(session_b).await.unwrap();

        assert_eq!(resolved.session_id, session_b);
    }

    #[test]
    fn docker_runtime_requires_core_configuration() {
        let error =
            SessionRuntimeManager::new(RuntimeManagerConfig::DockerPool(DockerRuntimeConfig {
                image: String::new(),
                ..docker_config()
            }))
            .err()
            .unwrap();

        assert!(matches!(
            error,
            RuntimeManagerError::InvalidConfiguration(_)
        ));
    }

    #[test]
    fn docker_runtime_validates_starting_capacity_limit() {
        let error =
            SessionRuntimeManager::new(RuntimeManagerConfig::DockerPool(DockerRuntimeConfig {
                max_starting_runtimes: 3,
                max_active_runtimes: 2,
                ..docker_config()
            }))
            .err()
            .unwrap();

        assert!(matches!(
            error,
            RuntimeManagerError::InvalidConfiguration(_)
        ));
    }

    #[test]
    fn docker_pool_profile_exposes_runtime_capacity() {
        let manager =
            SessionRuntimeManager::new(RuntimeManagerConfig::DockerPool(docker_config())).unwrap();

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

    #[test]
    fn docker_runtime_names_and_sockets_are_session_scoped() {
        let manager = Arc::new(
            DockerRuntimeManager::new(
                docker_config(),
                RuntimeProfile {
                    runtime_binding: "docker_runtime_pool".to_string(),
                    compatibility_mode: "session_runtime_pool".to_string(),
                    max_runtime_sessions: 2,
                    supports_legacy_global_routes: false,
                    supports_session_extensions: true,
                },
            )
            .unwrap(),
        );
        let session_id = Uuid::parse_str("019db438-c74a-7ef2-810c-792e298faf11").unwrap();

        assert_eq!(
            manager.socket_path_for_session(session_id),
            "/run/bpane/sessions/019db438-c74a-7ef2-810c-792e298faf11.sock"
        );
        assert_eq!(
            manager.container_name_for_session(session_id),
            format!("bpane-runtime-{}", session_id.as_simple())
        );
        assert_eq!(
            manager.cdp_endpoint_for_session(session_id),
            format!("http://bpane-runtime-{}:9223", session_id.as_simple())
        );
    }
}
