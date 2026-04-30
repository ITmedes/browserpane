use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::time::sleep;
use uuid::Uuid;

use super::*;

pub(super) struct StaticSingleRuntimeManager {
    pub(super) agent_socket_path: String,
    pub(super) cdp_endpoint: Option<String>,
    pub(super) idle_timeout: Duration,
    pub(super) active: Mutex<Option<RuntimeLease>>,
}

impl StaticSingleRuntimeManager {
    pub(super) fn new(
        agent_socket_path: String,
        cdp_endpoint: Option<String>,
        idle_timeout: Duration,
    ) -> Self {
        Self {
            agent_socket_path,
            cdp_endpoint,
            idle_timeout,
            active: Mutex::new(None),
        }
    }

    pub(super) fn describe_runtime(&self, profile: &RuntimeProfile) -> RuntimeSessionAccessInfo {
        RuntimeSessionAccessInfo {
            binding: profile.runtime_binding.clone(),
            compatibility_mode: profile.compatibility_mode.clone(),
            cdp_endpoint: self.cdp_endpoint.clone(),
        }
    }

    pub(super) async fn describe_assignment_status(
        &self,
        session_id: Uuid,
    ) -> Option<RuntimeAssignmentStatus> {
        let active = self.active.lock().await;
        active
            .as_ref()
            .filter(|lease| lease.session_id == session_id)
            .map(|_| RuntimeAssignmentStatus::Ready)
    }

    pub(super) async fn resolve(
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

    pub(super) async fn release(&self, session_id: Uuid) {
        let mut active = self.active.lock().await;
        if active
            .as_ref()
            .is_some_and(|lease| lease.session_id == session_id)
        {
            *active = None;
        }
    }

    pub(super) async fn mark_session_active(&self, session_id: Uuid) {
        let mut active = self.active.lock().await;
        if let Some(lease) = active
            .as_mut()
            .filter(|lease| lease.session_id == session_id)
        {
            bump_idle_generation(lease);
        }
    }

    pub(super) async fn mark_session_idle(self: &Arc<Self>, session_id: Uuid) {
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
