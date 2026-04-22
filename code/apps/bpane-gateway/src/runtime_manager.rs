use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::time::{sleep, Instant};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSessionRuntime {
    pub session_id: Uuid,
    pub agent_socket_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeManagerError {
    RuntimeBusy { active_session_id: Uuid },
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
    DockerSingle(DockerSingleRuntimeConfig),
}

#[derive(Debug, Clone)]
pub struct DockerSingleRuntimeConfig {
    pub docker_bin: String,
    pub image: String,
    pub network: String,
    pub shared_run_volume: String,
    pub container_name: String,
    pub socket_root: String,
    pub shm_size: String,
    pub start_timeout: Duration,
    pub idle_timeout: Duration,
    pub seccomp_unconfined: bool,
    pub env_file: Option<PathBuf>,
}

#[derive(Clone)]
pub struct SessionRuntimeManager {
    backend: RuntimeBackend,
}

#[derive(Clone)]
enum RuntimeBackend {
    StaticSingle(Arc<StaticSingleRuntimeManager>),
    DockerSingle(Arc<DockerSingleRuntimeManager>),
}

impl SessionRuntimeManager {
    pub fn new(config: RuntimeManagerConfig) -> Result<Self, RuntimeManagerError> {
        let backend = match config {
            RuntimeManagerConfig::StaticSingle {
                agent_socket_path,
                idle_timeout,
            } => RuntimeBackend::StaticSingle(Arc::new(StaticSingleRuntimeManager::new(
                agent_socket_path,
                idle_timeout,
            ))),
            RuntimeManagerConfig::DockerSingle(config) => {
                RuntimeBackend::DockerSingle(Arc::new(DockerSingleRuntimeManager::new(config)?))
            }
        };
        Ok(Self { backend })
    }

    pub async fn resolve(
        &self,
        session_id: Uuid,
    ) -> Result<ResolvedSessionRuntime, RuntimeManagerError> {
        match &self.backend {
            RuntimeBackend::StaticSingle(manager) => manager.resolve(session_id).await,
            RuntimeBackend::DockerSingle(manager) => manager.resolve(session_id).await,
        }
    }

    pub async fn release(&self, session_id: Uuid) {
        match &self.backend {
            RuntimeBackend::StaticSingle(manager) => manager.release(session_id).await,
            RuntimeBackend::DockerSingle(manager) => manager.release(session_id).await,
        }
    }

    pub async fn mark_session_active(&self, session_id: Uuid) {
        match &self.backend {
            RuntimeBackend::StaticSingle(manager) => manager.mark_session_active(session_id).await,
            RuntimeBackend::DockerSingle(manager) => manager.mark_session_active(session_id).await,
        }
    }

    pub async fn mark_session_idle(&self, session_id: Uuid) {
        match &self.backend {
            RuntimeBackend::StaticSingle(manager) => manager.mark_session_idle(session_id).await,
            RuntimeBackend::DockerSingle(manager) => manager.mark_session_idle(session_id).await,
        }
    }
}

#[derive(Debug, Clone)]
struct RuntimeLease {
    session_id: Uuid,
    agent_socket_path: String,
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
                lease.idle_generation = lease.idle_generation.saturating_add(1);
                Ok(ResolvedSessionRuntime {
                    session_id,
                    agent_socket_path: lease.agent_socket_path.clone(),
                })
            }
            None => {
                *active = Some(RuntimeLease {
                    session_id,
                    agent_socket_path: self.agent_socket_path.clone(),
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
            lease.idle_generation = lease.idle_generation.saturating_add(1);
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
            lease.idle_generation = lease.idle_generation.saturating_add(1);
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

struct DockerSingleRuntimeManager {
    config: DockerSingleRuntimeConfig,
    active: Mutex<Option<RuntimeLease>>,
}

impl DockerSingleRuntimeManager {
    fn new(config: DockerSingleRuntimeConfig) -> Result<Self, RuntimeManagerError> {
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
        if config.container_name.trim().is_empty() {
            return Err(RuntimeManagerError::InvalidConfiguration(
                "docker runtime backend requires a non-empty container_name".to_string(),
            ));
        }
        if config.socket_root.trim().is_empty() {
            return Err(RuntimeManagerError::InvalidConfiguration(
                "docker runtime backend requires a non-empty socket_root".to_string(),
            ));
        }
        Ok(Self {
            config,
            active: Mutex::new(None),
        })
    }

    async fn resolve(
        self: &Arc<Self>,
        session_id: Uuid,
    ) -> Result<ResolvedSessionRuntime, RuntimeManagerError> {
        let socket_path = self.socket_path_for_session(session_id);
        {
            let mut active = self.active.lock().await;
            match active.as_mut() {
                Some(lease) if lease.session_id != session_id => {
                    return Err(RuntimeManagerError::RuntimeBusy {
                        active_session_id: lease.session_id,
                    });
                }
                Some(lease) => {
                    lease.idle_generation = lease.idle_generation.saturating_add(1);
                    return Ok(ResolvedSessionRuntime {
                        session_id,
                        agent_socket_path: lease.agent_socket_path.clone(),
                    });
                }
                None => {
                    *active = Some(RuntimeLease {
                        session_id,
                        agent_socket_path: socket_path.clone(),
                        idle_generation: 0,
                    });
                }
            }
        }

        if let Err(error) = self.start_container(session_id, &socket_path).await {
            let mut active = self.active.lock().await;
            if active
                .as_ref()
                .is_some_and(|lease| lease.session_id == session_id)
            {
                *active = None;
            }
            return Err(error);
        }

        Ok(ResolvedSessionRuntime {
            session_id,
            agent_socket_path: socket_path,
        })
    }

    async fn release(&self, session_id: Uuid) {
        let should_stop = {
            let mut active = self.active.lock().await;
            if active
                .as_ref()
                .is_some_and(|lease| lease.session_id == session_id)
            {
                *active = None;
                true
            } else {
                false
            }
        };

        if should_stop {
            let _ = self.stop_container().await;
        }
    }

    async fn mark_session_active(&self, session_id: Uuid) {
        let mut active = self.active.lock().await;
        if let Some(lease) = active
            .as_mut()
            .filter(|lease| lease.session_id == session_id)
        {
            lease.idle_generation = lease.idle_generation.saturating_add(1);
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
            lease.idle_generation = lease.idle_generation.saturating_add(1);
            lease.idle_generation
        };

        let manager = Arc::clone(self);
        tokio::spawn(async move {
            sleep(manager.config.idle_timeout).await;
            let should_stop = {
                let mut active = manager.active.lock().await;
                if active.as_ref().is_some_and(|lease| {
                    lease.session_id == session_id && lease.idle_generation == idle_generation
                }) {
                    *active = None;
                    true
                } else {
                    false
                }
            };
            if should_stop {
                let _ = manager.stop_container().await;
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

    async fn start_container(
        &self,
        session_id: Uuid,
        socket_path: &str,
    ) -> Result<(), RuntimeManagerError> {
        let _ = self.stop_container().await;

        let mut command = Command::new(&self.config.docker_bin);
        command.arg("run").arg("-d").arg("--rm");
        command
            .arg("--name")
            .arg(&self.config.container_name)
            .arg("--network")
            .arg(&self.config.network)
            .arg("--network-alias")
            .arg(&self.config.container_name)
            .arg("-v")
            .arg(format!("{}:/run/bpane", self.config.shared_run_volume))
            .arg("--shm-size")
            .arg(&self.config.shm_size)
            .arg("--label")
            .arg(format!("browserpane.session_id={session_id}"))
            .arg("-e")
            .arg(format!("BPANE_SOCKET_PATH={socket_path}"));

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

        self.wait_for_socket(socket_path).await
    }

    async fn stop_container(&self) -> Result<(), RuntimeManagerError> {
        let output = Command::new(&self.config.docker_bin)
            .arg("rm")
            .arg("-f")
            .arg(&self.config.container_name)
            .output()
            .await
            .map_err(|error| {
                RuntimeManagerError::StartupFailed(format!(
                    "failed to stop docker runtime: {error}"
                ))
            })?;
        if output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("No such container") {
            return Ok(());
        }

        Err(RuntimeManagerError::StartupFailed(format!(
            "docker rm failed: {}",
            stderr.trim()
        )))
    }

    async fn wait_for_socket(&self, socket_path: &str) -> Result<(), RuntimeManagerError> {
        let deadline = Instant::now() + self.config.start_timeout;
        loop {
            if std::path::Path::new(socket_path).exists() {
                return Ok(());
            }
            if Instant::now() >= deadline {
                let _ = self.stop_container().await;
                return Err(RuntimeManagerError::StartupFailed(format!(
                    "docker runtime did not create socket {socket_path} before startup timeout"
                )));
            }
            sleep(Duration::from_millis(200)).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn docker_single_runtime_requires_core_configuration() {
        let error = SessionRuntimeManager::new(RuntimeManagerConfig::DockerSingle(
            DockerSingleRuntimeConfig {
                docker_bin: "docker".to_string(),
                image: String::new(),
                network: "bpane".to_string(),
                shared_run_volume: "bpane-run".to_string(),
                container_name: "bpane-runtime".to_string(),
                socket_root: "/run/bpane/sessions".to_string(),
                shm_size: "128m".to_string(),
                start_timeout: Duration::from_secs(30),
                idle_timeout: Duration::from_secs(300),
                seccomp_unconfined: true,
                env_file: None,
            },
        ))
        .err()
        .unwrap();

        assert!(matches!(
            error,
            RuntimeManagerError::InvalidConfiguration(_)
        ));
    }

    #[test]
    fn docker_single_runtime_socket_path_is_session_scoped() {
        let manager = Arc::new(
            DockerSingleRuntimeManager::new(DockerSingleRuntimeConfig {
                docker_bin: "docker".to_string(),
                image: "deploy-host".to_string(),
                network: "deploy_bpane-internal".to_string(),
                shared_run_volume: "deploy_agent-socket".to_string(),
                container_name: "bpane-runtime".to_string(),
                socket_root: "/run/bpane/sessions".to_string(),
                shm_size: "128m".to_string(),
                start_timeout: Duration::from_secs(30),
                idle_timeout: Duration::from_secs(300),
                seccomp_unconfined: true,
                env_file: None,
            })
            .unwrap(),
        );
        let session_id = Uuid::parse_str("019db438-c74a-7ef2-810c-792e298faf11").unwrap();

        assert_eq!(
            manager.socket_path_for_session(session_id),
            "/run/bpane/sessions/019db438-c74a-7ef2-810c-792e298faf11.sock"
        );
    }
}
