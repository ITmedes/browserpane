use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use tokio::fs;
use tokio::process::Command;
use tokio::sync::{futures::OwnedNotified, Mutex, Notify};
use tokio::time::{sleep, Instant};
use tracing::{info, warn};
use uuid::Uuid;

use super::*;
use crate::session_control::SessionLifecycleState;

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

enum ResolveAction {
    Return(ResolvedSessionRuntime),
    Wait(OwnedNotified),
    Start {
        lease: RuntimeLease,
        notify: Arc<Notify>,
    },
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
            session_store: Mutex::new(None),
        })
    }

    pub(super) async fn attach_session_store(&self, store: SessionStore) {
        *self.session_store.lock().await = Some(store);
    }

    async fn session_store(&self) -> Option<SessionStore> {
        self.session_store.lock().await.clone()
    }

    pub(super) async fn resolve(
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
                    if let Err(error) = self
                        .persist_assignment(&lease, RuntimeAssignmentStatus::Starting)
                        .await
                    {
                        let mut leases = self.leases.lock().await;
                        if matches!(
                            leases.get(&session_id),
                            Some(DockerLeaseState::Starting { .. })
                        ) {
                            leases.remove(&session_id);
                        }
                        notify.notify_waiters();
                        return Err(error);
                    }
                    let result = self.start_container(&lease).await;
                    let mut leases = self.leases.lock().await;
                    let stop_container = match result {
                        Ok(()) => {
                            if matches!(
                                leases.get(&session_id),
                                Some(DockerLeaseState::Starting { .. })
                            ) {
                                drop(leases);
                                if let Err(error) = self
                                    .persist_assignment(&lease, RuntimeAssignmentStatus::Ready)
                                    .await
                                {
                                    let mut leases = self.leases.lock().await;
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
                                    let _ = self.clear_assignment(session_id).await;
                                    return Err(error);
                                }
                                let mut leases = self.leases.lock().await;
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
                            let _ = self.clear_assignment(session_id).await;
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

    pub(super) async fn release(&self, session_id: Uuid) {
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
            let _ = self.clear_assignment(session_id).await;
        }
    }

    pub(super) async fn mark_session_active(&self, session_id: Uuid) {
        let mut leases = self.leases.lock().await;
        if let Some(DockerLeaseState::Ready(lease)) = leases.get_mut(&session_id) {
            bump_idle_generation(lease);
        }
    }

    pub(super) async fn mark_session_idle(self: &Arc<Self>, session_id: Uuid) {
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
                let _ = manager.clear_assignment(session_id).await;
            }
        });
    }

    pub(super) async fn reconcile_persisted_state(&self) -> Result<(), RuntimeManagerError> {
        let Some(store) = self.session_store().await else {
            return Ok(());
        };

        let assignments = store
            .list_runtime_assignments(&self.profile.runtime_binding)
            .await
            .map_err(|error| RuntimeManagerError::PersistenceFailed(error.to_string()))?;
        if assignments.is_empty() {
            return Ok(());
        }

        let mut leases = self.leases.lock().await;
        for assignment in assignments {
            let session = store
                .get_session_by_id(assignment.session_id)
                .await
                .map_err(|error| RuntimeManagerError::PersistenceFailed(error.to_string()))?;
            let session_state = session.as_ref().map(|stored| stored.state);

            let recoverable = matches!(
                session_state,
                Some(
                    SessionLifecycleState::Pending
                        | SessionLifecycleState::Starting
                        | SessionLifecycleState::Ready
                        | SessionLifecycleState::Active
                        | SessionLifecycleState::Idle
                )
            );

            if !recoverable || leases.len() >= self.profile.max_runtime_sessions {
                drop(leases);
                self.cleanup_stale_assignment(&store, &assignment, recoverable)
                    .await?;
                leases = self.leases.lock().await;
                continue;
            }

            let Some(container_name) = assignment.container_name.as_deref() else {
                drop(leases);
                self.cleanup_stale_assignment(&store, &assignment, recoverable)
                    .await?;
                leases = self.leases.lock().await;
                continue;
            };

            let container_exists = self.container_exists(container_name).await?;
            if !container_exists {
                drop(leases);
                self.cleanup_stale_assignment(&store, &assignment, recoverable)
                    .await?;
                leases = self.leases.lock().await;
                continue;
            }

            if !Path::new(&assignment.agent_socket_path).exists() {
                drop(leases);
                self.cleanup_stale_assignment(&store, &assignment, recoverable)
                    .await?;
                leases = self.leases.lock().await;
                continue;
            }

            info!(
                session_id = %assignment.session_id,
                container_name,
                "recovered persisted docker runtime assignment",
            );
            leases.insert(
                assignment.session_id,
                DockerLeaseState::Ready(RuntimeLease {
                    session_id: assignment.session_id,
                    agent_socket_path: assignment.agent_socket_path.clone(),
                    container_name: Some(container_name.to_string()),
                    idle_generation: 0,
                }),
            );
        }

        Ok(())
    }

    pub(super) fn socket_path_for_session(&self, session_id: Uuid) -> String {
        format!(
            "{}/{}.sock",
            self.config.socket_root.trim_end_matches('/'),
            session_id
        )
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

    async fn start_container(&self, lease: &RuntimeLease) -> Result<(), RuntimeManagerError> {
        let container_name = lease.container_name.as_deref().ok_or_else(|| {
            RuntimeManagerError::StartupFailed(
                "docker lease did not include a container name".to_string(),
            )
        })?;

        let _ = self.stop_container(container_name).await;
        let _ = remove_socket_path(&lease.agent_socket_path).await;
        let extension_dirs = self.session_extension_dirs(lease.session_id).await?;

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
        if !extension_dirs.is_empty() {
            command
                .arg("-e")
                .arg(format!("BPANE_EXTENSION_DIRS={}", extension_dirs.join(",")));
        }

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

    async fn session_extension_dirs(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<String>, RuntimeManagerError> {
        let Some(store) = self.session_store().await else {
            return Ok(Vec::new());
        };
        let session = store
            .get_session_by_id(session_id)
            .await
            .map_err(|error| RuntimeManagerError::PersistenceFailed(error.to_string()))?
            .ok_or_else(|| {
                RuntimeManagerError::PersistenceFailed(format!(
                    "session {session_id} not found while starting docker runtime"
                ))
            })?;
        Ok(session
            .extensions
            .into_iter()
            .map(|extension| extension.install_path)
            .collect())
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
            if Path::new(socket_path).exists() {
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

    async fn container_exists(&self, container_name: &str) -> Result<bool, RuntimeManagerError> {
        let output = Command::new(&self.config.docker_bin)
            .arg("inspect")
            .arg("--type")
            .arg("container")
            .arg(container_name)
            .output()
            .await
            .map_err(|error| {
                RuntimeManagerError::StartupFailed(format!(
                    "failed to inspect docker runtime {container_name}: {error}"
                ))
            })?;
        if output.status.success() {
            return Ok(true);
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("No such object") || stderr.contains("No such container") {
            return Ok(false);
        }
        Err(RuntimeManagerError::StartupFailed(format!(
            "docker inspect failed for {container_name}: {}",
            stderr.trim()
        )))
    }

    async fn persist_assignment(
        &self,
        lease: &RuntimeLease,
        status: RuntimeAssignmentStatus,
    ) -> Result<(), RuntimeManagerError> {
        let Some(store) = self.session_store().await else {
            return Ok(());
        };
        store
            .upsert_runtime_assignment(PersistedRuntimeAssignment {
                session_id: lease.session_id,
                runtime_binding: self.profile.runtime_binding.clone(),
                status,
                agent_socket_path: lease.agent_socket_path.clone(),
                container_name: lease.container_name.clone(),
                cdp_endpoint: Some(self.cdp_endpoint_for_session(lease.session_id)),
            })
            .await
            .map_err(|error| RuntimeManagerError::PersistenceFailed(error.to_string()))
    }

    async fn clear_assignment(&self, session_id: Uuid) -> Result<(), RuntimeManagerError> {
        let Some(store) = self.session_store().await else {
            return Ok(());
        };
        store
            .clear_runtime_assignment(session_id)
            .await
            .map_err(|error| RuntimeManagerError::PersistenceFailed(error.to_string()))
    }

    async fn cleanup_stale_assignment(
        &self,
        store: &SessionStore,
        assignment: &PersistedRuntimeAssignment,
        restore_session_ready: bool,
    ) -> Result<(), RuntimeManagerError> {
        if let Some(container_name) = assignment.container_name.as_deref() {
            let _ = self.stop_container(container_name).await;
        }
        let _ = remove_socket_path(&assignment.agent_socket_path).await;
        store
            .clear_runtime_assignment(assignment.session_id)
            .await
            .map_err(|error| RuntimeManagerError::PersistenceFailed(error.to_string()))?;
        if restore_session_ready {
            let _ = store
                .mark_session_ready_after_runtime_loss(assignment.session_id)
                .await
                .map_err(|error| RuntimeManagerError::PersistenceFailed(error.to_string()))?;
        }
        warn!(
            session_id = %assignment.session_id,
            container_name = assignment.container_name.as_deref().unwrap_or("unknown"),
            "cleared stale persisted docker runtime assignment",
        );
        Ok(())
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
