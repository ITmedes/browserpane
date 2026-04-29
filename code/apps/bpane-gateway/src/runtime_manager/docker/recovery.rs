use std::path::Path;

use tracing::info;
use tracing::warn;

use super::*;
use crate::session_control::SessionLifecycleState;

impl DockerRuntimeManager {
    pub(in crate::runtime_manager) async fn reconcile_persisted_state(
        &self,
    ) -> Result<(), RuntimeManagerError> {
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

    pub(super) async fn persist_assignment(
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

    pub(super) async fn clear_assignment(
        &self,
        session_id: Uuid,
    ) -> Result<(), RuntimeManagerError> {
        let Some(store) = self.session_store().await else {
            return Ok(());
        };
        store
            .clear_runtime_assignment(session_id)
            .await
            .map_err(|error| RuntimeManagerError::PersistenceFailed(error.to_string()))
    }

    pub(super) async fn cleanup_stale_assignment(
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
