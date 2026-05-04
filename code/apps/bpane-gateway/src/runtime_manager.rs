use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use uuid::Uuid;

use crate::session_control::SessionStore;
use crate::workspaces::WorkspaceFileStore;

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
    pub socket_volume: String,
    pub session_data_volume_prefix: String,
    pub container_name_prefix: String,
    pub socket_root: String,
    pub session_data_root: String,
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

    pub async fn attach_workspace_file_store(&self, store: Arc<WorkspaceFileStore>) {
        match &self.backend {
            RuntimeBackend::StaticSingle(_) => {}
            RuntimeBackend::Docker(manager) => manager.attach_workspace_file_store(store).await,
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

    pub async fn describe_session_runtime_assignment_status(
        &self,
        session_id: Uuid,
    ) -> Option<RuntimeAssignmentStatus> {
        match &self.backend {
            RuntimeBackend::StaticSingle(manager) => {
                manager.describe_assignment_status(session_id).await
            }
            RuntimeBackend::Docker(manager) => manager.describe_assignment_status(session_id).await,
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
mod tests;
