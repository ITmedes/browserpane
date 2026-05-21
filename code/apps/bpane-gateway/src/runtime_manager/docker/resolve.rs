use std::sync::Arc;

use tokio::sync::{futures::OwnedNotified, Notify};
use tokio::time::sleep;
use uuid::Uuid;

use super::*;

enum ResolveAction {
    Return {
        runtime: ResolvedSessionRuntime,
        browser_context_id: Option<Uuid>,
    },
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
        let scope = self.runtime_data_scope_for_session(session_id).await?;
        loop {
            let action = {
                let mut leases = self.leases.lock().await;
                match leases.get_mut(&session_id) {
                    Some(DockerLeaseState::Ready(lease)) => {
                        bump_idle_generation(lease);
                        ResolveAction::Return {
                            runtime: ResolvedSessionRuntime {
                                session_id,
                                agent_socket_path: lease.agent_socket_path.clone(),
                            },
                            browser_context_id: lease.browser_context_id,
                        }
                    }
                    Some(DockerLeaseState::Starting { notify, .. }) => {
                        ResolveAction::Wait(notify.clone().notified_owned())
                    }
                    None => {
                        if let Some(browser_context_id) = scope.browser_context_id {
                            if let Some(active_session_id) =
                                active_browser_context_session_id(&leases, browser_context_id)
                            {
                                return Err(RuntimeManagerError::BrowserContextInUse {
                                    browser_context_id,
                                    active_session_id,
                                });
                            }
                        }
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
                            browser_context_id: scope.browser_context_id,
                            discard_session_data_on_release: scope.discard_session_data_on_release,
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
                ResolveAction::Return {
                    runtime,
                    browser_context_id,
                } => {
                    if let Some(context_id) = browser_context_id {
                        self.mark_browser_context_used_for_session(session_id, context_id)
                            .await;
                    }
                    return Ok(runtime);
                }
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
                                    self.cleanup_runtime_resources(&lease).await;
                                    let _ = self.clear_assignment(session_id).await;
                                    return Err(error);
                                }
                                let mut leases = self.leases.lock().await;
                                leases.insert(session_id, DockerLeaseState::Ready(lease.clone()));
                                notify.notify_waiters();
                                if let Some(context_id) = lease.browser_context_id {
                                    self.mark_browser_context_used_for_session(
                                        session_id, context_id,
                                    )
                                    .await;
                                }
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
                            self.cleanup_runtime_resources(&lease).await;
                            return Err(error);
                        }
                    };
                    notify.notify_waiters();
                    drop(leases);
                    if stop_container.is_some() {
                        self.cleanup_runtime_resources(&lease).await;
                    }
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
            let lease = state.lease().clone();
            self.cleanup_runtime_resources(&lease).await;
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
        let (idle_generation, lease_snapshot) = {
            let mut leases = self.leases.lock().await;
            let Some(DockerLeaseState::Ready(lease)) = leases.get_mut(&session_id) else {
                return;
            };
            bump_idle_generation(lease);
            (lease.idle_generation, lease.clone())
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
                manager.cleanup_runtime_resources(&lease_snapshot).await;
                let _ = manager.clear_assignment(session_id).await;
            }
        });
    }

    async fn cleanup_runtime_resources(&self, lease: &RuntimeLease) {
        if let Some(container_name) = lease.container_name.as_deref() {
            let _ = self.stop_container(container_name).await;
        }
        let _ = remove_socket_path(&lease.agent_socket_path).await;
        if lease.discard_session_data_on_release {
            let _ = self.remove_session_data_volume(lease.session_id).await;
        }
    }
}
