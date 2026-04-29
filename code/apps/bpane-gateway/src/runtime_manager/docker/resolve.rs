use std::sync::Arc;

use tokio::sync::{futures::OwnedNotified, Notify};
use tokio::time::sleep;
use uuid::Uuid;

use super::*;

enum ResolveAction {
    Return(ResolvedSessionRuntime),
    Wait(OwnedNotified),
    Start {
        lease: RuntimeLease,
        notify: Arc<Notify>,
    },
}

impl DockerRuntimeManager {
    pub(in crate::runtime_manager) async fn resolve(
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
                ResolveAction::Wait(waiter) => waiter.await,
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

    pub(in crate::runtime_manager) async fn release(&self, session_id: Uuid) {
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

    pub(in crate::runtime_manager) async fn mark_session_active(&self, session_id: Uuid) {
        let mut leases = self.leases.lock().await;
        if let Some(DockerLeaseState::Ready(lease)) = leases.get_mut(&session_id) {
            bump_idle_generation(lease);
        }
    }

    pub(in crate::runtime_manager) async fn mark_session_idle(self: &Arc<Self>, session_id: Uuid) {
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
}
