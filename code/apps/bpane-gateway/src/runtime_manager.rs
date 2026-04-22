use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::fs;
use tokio::process::Command;
use tokio::sync::{futures::OwnedNotified, Mutex, Notify};
use tokio::time::{sleep, Instant};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSessionRuntime {
    pub session_id: Uuid,
    pub agent_socket_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeProfile {
    pub runtime_binding: String,
    pub compatibility_mode: String,
    pub max_runtime_sessions: usize,
    pub supports_legacy_global_routes: bool,
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
        }
    }
}

impl std::error::Error for RuntimeManagerError {}

#[derive(Debug, Clone)]
pub enum RuntimeManagerConfig {
    StaticSingle {
        agent_socket_path: String,
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
                idle_timeout,
            } => {
                let profile = RuntimeProfile {
                    runtime_binding: "legacy_single_session".to_string(),
                    compatibility_mode: "legacy_single_runtime".to_string(),
                    max_runtime_sessions: 1,
                    supports_legacy_global_routes: true,
                };
                Ok(Self {
                    backend: RuntimeBackend::StaticSingle(Arc::new(
                        StaticSingleRuntimeManager::new(agent_socket_path, idle_timeout),
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

struct StaticSingleRuntimeManager {
    agent_socket_path: String,
    idle_timeout: Duration,
    active: Mutex<Option<RuntimeLease>>,
}

impl StaticSingleRuntimeManager {
    fn new(agent_socket_path: String, idle_timeout: Duration) -> Self {
        Self {
            agent_socket_path,
            idle_timeout,
            active: Mutex::new(None),
        }
    }

    async fn resolve(
        self: &Arc<Self>,
        session_id: Uuid,
    ) -> Result<ResolvedSessionRuntime, RuntimeManagerError> {
        let mut active = self.active.lock().await;
        match active.as_mut() {
            Some(lease) if lease.session_id != session_id => {
                Err(RuntimeManagerError::RuntimeBusy {
                    active_session_id: lease.session_id,
                })
            }
            Some(lease) => {
                bump_idle_generation(lease);
                Ok(ResolvedSessionRuntime {
                    session_id,
                    agent_socket_path: lease.agent_socket_path.clone(),
                })
            }
            None => {
                *active = Some(RuntimeLease {
                    session_id,
                    agent_socket_path: self.agent_socket_path.clone(),
                    container_name: None,
                    idle_generation: 0,
                });
                Ok(ResolvedSessionRuntime {
                    session_id,
                    agent_socket_path: self.agent_socket_path.clone(),
                })
            }
        }
    }

    async fn release(&self, session_id: Uuid) {
        let mut active = self.active.lock().await;
        if active
            .as_ref()
            .is_some_and(|lease| lease.session_id == session_id)
        {
            *active = None;
        }
    }

    async fn mark_session_active(&self, session_id: Uuid) {
        let mut active = self.active.lock().await;
        if let Some(lease) = active
            .as_mut()
            .filter(|lease| lease.session_id == session_id)
        {
            bump_idle_generation(lease);
        }
    }

    async fn mark_session_idle(self: &Arc<Self>, session_id: Uuid) {
        let idle_generation = {
            let mut active = self.active.lock().await;
            let Some(lease) = active
                .as_mut()
                .filter(|lease| lease.session_id == session_id)
            else {
                return;
            };
            bump_idle_generation(lease);
            lease.idle_generation
        };

        let manager = Arc::clone(self);
        tokio::spawn(async move {
            sleep(manager.idle_timeout).await;
            let mut active = manager.active.lock().await;
            if active.as_ref().is_some_and(|lease| {
                lease.session_id == session_id && lease.idle_generation == idle_generation
            }) {
                *active = None;
            }
        });
    }
}

struct DockerRuntimeManager {
    config: DockerRuntimeConfig,
    profile: RuntimeProfile,
    leases: Mutex<HashMap<Uuid, DockerLeaseState>>,
}

enum DockerLeaseState {
    Starting {
        lease: RuntimeLease,
        notify: Arc<Notify>,
    },
    Ready(RuntimeLease),
}

enum ResolveAction {
    Return(ResolvedSessionRuntime),
    Wait(OwnedNotified),
    Start {
        lease: RuntimeLease,
        notify: Arc<Notify>,
    },
}

impl DockerRuntimeManager {
    fn new(
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
        if config.shared_run_volume.trim().is_empty() {
            return Err(RuntimeManagerError::InvalidConfiguration(
                "docker runtime backend requires a non-empty shared_run_volume".to_string(),
            ));
        }
        if config.container_name_prefix.trim().is_empty() {
            return Err(RuntimeManagerError::InvalidConfiguration(
                "docker runtime backend requires a non-empty container_name_prefix".to_string(),
            ));
        }
        if config.socket_root.trim().is_empty() {
            return Err(RuntimeManagerError::InvalidConfiguration(
                "docker runtime backend requires a non-empty socket_root".to_string(),
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
        })
    }

    async fn resolve(
        self: &Arc<Self>,
        session_id: Uuid,
    ) -> Result<ResolvedSessionRuntime, RuntimeManagerError> {
        loop {
            let action = {
                let mut leases = self.leases.lock().await;
                match leases.get_mut(&session_id) {
                    Some(DockerLeaseState::Ready(lease)) => {
                        bump_idle_generation(lease);
                        ResolveAction::Return(ResolvedSessionRuntime {
                            session_id,
                            agent_socket_path: lease.agent_socket_path.clone(),
                        })
                    }
                    Some(DockerLeaseState::Starting { notify, .. }) => {
                        ResolveAction::Wait(notify.clone().notified_owned())
                    }
                    None => {
                        if leases.len() >= self.profile.max_runtime_sessions {
                            return Err(RuntimeManagerError::RuntimeCapacityReached {
                                max_active_runtimes: self.profile.max_runtime_sessions,
                                active_session_ids: sorted_active_session_ids(&leases),
                            });
                        }
                        let starting = leases
                            .values()
                            .filter(|state| matches!(state, DockerLeaseState::Starting { .. }))
                            .count();
                        if starting >= self.config.max_starting_runtimes {
                            return Err(RuntimeManagerError::RuntimeStartupCapacityReached {
                                max_starting_runtimes: self.config.max_starting_runtimes,
                            });
                        }
                        let lease = RuntimeLease {
                            session_id,
                            agent_socket_path: self.socket_path_for_session(session_id),
                            container_name: Some(self.container_name_for_session(session_id)),
                            idle_generation: 0,
                        };
                        let notify = Arc::new(Notify::new());
                        leases.insert(
                            session_id,
                            DockerLeaseState::Starting {
                                lease: lease.clone(),
                                notify: notify.clone(),
                            },
                        );
                        ResolveAction::Start { lease, notify }
                    }
                }
            };

            match action {
                ResolveAction::Return(runtime) => return Ok(runtime),
                ResolveAction::Wait(waiter) => {
                    waiter.await;
                }
                ResolveAction::Start { lease, notify } => {
                    let result = self.start_container(&lease).await;
                    let mut leases = self.leases.lock().await;
                    let stop_container = match result {
                        Ok(()) => {
                            if matches!(
                                leases.get(&session_id),
                                Some(DockerLeaseState::Starting { .. })
                            ) {
                                leases.insert(session_id, DockerLeaseState::Ready(lease.clone()));
                                notify.notify_waiters();
                                return Ok(ResolvedSessionRuntime {
                                    session_id,
                                    agent_socket_path: lease.agent_socket_path.clone(),
                                });
                            }
                            lease.container_name.clone()
                        }
                        Err(error) => {
                            if matches!(
                                leases.get(&session_id),
                                Some(DockerLeaseState::Starting { .. })
                            ) {
                                leases.remove(&session_id);
                            }
                            notify.notify_waiters();
                            drop(leases);
                            if let Some(container_name) = &lease.container_name {
                                let _ = self.stop_container(container_name).await;
                            }
                            let _ = remove_socket_path(&lease.agent_socket_path).await;
                            return Err(error);
                        }
                    };
                    notify.notify_waiters();
                    drop(leases);
                    if let Some(container_name) = stop_container {
                        let _ = self.stop_container(&container_name).await;
                    }
                    let _ = remove_socket_path(&lease.agent_socket_path).await;
                }
            }
        }
    }

    async fn release(&self, session_id: Uuid) {
        let removed = {
            let mut leases = self.leases.lock().await;
            leases.remove(&session_id)
        };

        if let Some(state) = removed {
            if let DockerLeaseState::Starting { notify, .. } = &state {
                notify.notify_waiters();
            }
            if let Some(container_name) = state.lease().container_name.as_deref() {
                let _ = self.stop_container(container_name).await;
            }
            let _ = remove_socket_path(&state.lease().agent_socket_path).await;
        }
    }

    async fn mark_session_active(&self, session_id: Uuid) {
        let mut leases = self.leases.lock().await;
        if let Some(DockerLeaseState::Ready(lease)) = leases.get_mut(&session_id) {
            bump_idle_generation(lease);
        }
    }

    async fn mark_session_idle(self: &Arc<Self>, session_id: Uuid) {
        let (idle_generation, container_name, socket_path) = {
            let mut leases = self.leases.lock().await;
            let Some(DockerLeaseState::Ready(lease)) = leases.get_mut(&session_id) else {
                return;
            };
            bump_idle_generation(lease);
            (
                lease.idle_generation,
                lease.container_name.clone().unwrap_or_default(),
                lease.agent_socket_path.clone(),
            )
        };

        let manager = Arc::clone(self);
        tokio::spawn(async move {
            sleep(manager.config.idle_timeout).await;
            let should_stop = {
                let mut leases = manager.leases.lock().await;
                let Some(DockerLeaseState::Ready(lease)) = leases.get(&session_id) else {
                    return;
                };
                if lease.idle_generation != idle_generation {
                    return;
                }
                leases.remove(&session_id);
                true
            };
            if should_stop {
                let _ = manager.stop_container(&container_name).await;
                let _ = remove_socket_path(&socket_path).await;
            }
        });
    }

    fn socket_path_for_session(&self, session_id: Uuid) -> String {
        format!(
            "{}/{}.sock",
            self.config.socket_root.trim_end_matches('/'),
            session_id
        )
    }

    fn container_name_for_session(&self, session_id: Uuid) -> String {
        format!(
            "{}-{}",
            self.config.container_name_prefix.trim_end_matches('-'),
            session_id.as_simple()
        )
    }

    async fn start_container(&self, lease: &RuntimeLease) -> Result<(), RuntimeManagerError> {
        let container_name = lease.container_name.as_deref().ok_or_else(|| {
            RuntimeManagerError::StartupFailed(
                "docker lease did not include a container name".to_string(),
            )
        })?;

        let _ = self.stop_container(container_name).await;
        let _ = remove_socket_path(&lease.agent_socket_path).await;

        let mut command = Command::new(&self.config.docker_bin);
        command.arg("run").arg("-d").arg("--rm");
        command
            .arg("--name")
            .arg(container_name)
            .arg("--network")
            .arg(&self.config.network)
            .arg("--network-alias")
            .arg(container_name)
            .arg("-v")
            .arg(format!("{}:/run/bpane", self.config.shared_run_volume))
            .arg("--shm-size")
            .arg(&self.config.shm_size)
            .arg("--label")
            .arg(format!("browserpane.session_id={}", lease.session_id))
            .arg("-e")
            .arg(format!("BPANE_SESSION_ID={}", lease.session_id))
            .arg("-e")
            .arg(format!("BPANE_SOCKET_PATH={}", lease.agent_socket_path));

        if self.config.seccomp_unconfined {
            command.arg("--security-opt").arg("seccomp=unconfined");
        }
        if let Some(env_file) = &self.config.env_file {
            command.arg("--env-file").arg(env_file);
        }

        command.arg(&self.config.image);

        let output = command.output().await.map_err(|error| {
            RuntimeManagerError::StartupFailed(format!("failed to run docker launcher: {error}"))
        })?;
        if !output.status.success() {
            return Err(RuntimeManagerError::StartupFailed(format!(
                "docker run failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }

        self.wait_for_socket(&lease.agent_socket_path, container_name)
            .await
    }

    async fn stop_container(&self, container_name: &str) -> Result<(), RuntimeManagerError> {
        let stop_output = Command::new(&self.config.docker_bin)
            .arg("stop")
            .arg("-t")
            .arg("20")
            .arg(container_name)
            .output()
            .await
            .map_err(|error| {
                RuntimeManagerError::StartupFailed(format!(
                    "failed to stop docker runtime: {error}"
                ))
            })?;
        if stop_output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&stop_output.stderr);
        if stderr.contains("No such container") {
            return Ok(());
        }

        let rm_output = Command::new(&self.config.docker_bin)
            .arg("rm")
            .arg("-f")
            .arg(container_name)
            .output()
            .await
            .map_err(|error| {
                RuntimeManagerError::StartupFailed(format!(
                    "failed to force-remove docker runtime: {error}"
                ))
            })?;
        if rm_output.status.success() {
            return Ok(());
        }

        let rm_stderr = String::from_utf8_lossy(&rm_output.stderr);
        if rm_stderr.contains("No such container") {
            return Ok(());
        }

        Err(RuntimeManagerError::StartupFailed(format!(
            "docker stop failed: {}; docker rm failed: {}",
            stderr.trim(),
            rm_stderr.trim()
        )))
    }

    async fn wait_for_socket(
        &self,
        socket_path: &str,
        container_name: &str,
    ) -> Result<(), RuntimeManagerError> {
        let deadline = Instant::now() + self.config.start_timeout;
        loop {
            if std::path::Path::new(socket_path).exists() {
                return Ok(());
            }
            if Instant::now() >= deadline {
                let _ = self.stop_container(container_name).await;
                let _ = remove_socket_path(socket_path).await;
                return Err(RuntimeManagerError::StartupFailed(format!(
                    "docker runtime did not create socket {socket_path} before startup timeout"
                )));
            }
            sleep(Duration::from_millis(200)).await;
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

fn bump_idle_generation(lease: &mut RuntimeLease) {
    lease.idle_generation = lease.idle_generation.saturating_add(1);
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
    }

    #[tokio::test]
    async fn static_single_runtime_blocks_parallel_session_assignment() {
        let manager = SessionRuntimeManager::new(RuntimeManagerConfig::StaticSingle {
            agent_socket_path: "/tmp/bpane.sock".to_string(),
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
    }
}
