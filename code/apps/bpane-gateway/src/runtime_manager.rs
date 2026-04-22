use std::fmt;
use std::sync::Arc;

use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSessionRuntime {
    pub session_id: Uuid,
    pub agent_socket_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeManagerError {
    RuntimeBusy { active_session_id: Uuid },
}

impl fmt::Display for RuntimeManagerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RuntimeBusy { active_session_id } => write!(
                f,
                "the current gateway runtime is already assigned to active session {active_session_id}"
            ),
        }
    }
}

impl std::error::Error for RuntimeManagerError {}

#[derive(Clone)]
pub struct SessionRuntimeManager {
    backend: Arc<StaticSingleRuntimeManager>,
}

impl SessionRuntimeManager {
    pub fn static_single(agent_socket_path: String) -> Self {
        Self {
            backend: Arc::new(StaticSingleRuntimeManager {
                agent_socket_path,
                active_session_id: Mutex::new(None),
            }),
        }
    }

    pub async fn resolve(
        &self,
        session_id: Uuid,
    ) -> Result<ResolvedSessionRuntime, RuntimeManagerError> {
        self.backend.resolve(session_id).await
    }

    pub async fn release(&self, session_id: Uuid) {
        self.backend.release(session_id).await;
    }
}

struct StaticSingleRuntimeManager {
    agent_socket_path: String,
    active_session_id: Mutex<Option<Uuid>>,
}

impl StaticSingleRuntimeManager {
    async fn resolve(
        &self,
        session_id: Uuid,
    ) -> Result<ResolvedSessionRuntime, RuntimeManagerError> {
        let mut active = self.active_session_id.lock().await;
        match *active {
            Some(current) if current != session_id => Err(RuntimeManagerError::RuntimeBusy {
                active_session_id: current,
            }),
            Some(_) => Ok(ResolvedSessionRuntime {
                session_id,
                agent_socket_path: self.agent_socket_path.clone(),
            }),
            None => {
                *active = Some(session_id);
                Ok(ResolvedSessionRuntime {
                    session_id,
                    agent_socket_path: self.agent_socket_path.clone(),
                })
            }
        }
    }

    async fn release(&self, session_id: Uuid) {
        let mut active = self.active_session_id.lock().await;
        if active.as_ref() == Some(&session_id) {
            *active = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn static_single_runtime_reuses_same_session_assignment() {
        let manager = SessionRuntimeManager::static_single("/tmp/bpane.sock".to_string());
        let session_id = Uuid::now_v7();

        let first = manager.resolve(session_id).await.unwrap();
        let second = manager.resolve(session_id).await.unwrap();

        assert_eq!(first, second);
        assert_eq!(first.agent_socket_path, "/tmp/bpane.sock");
    }

    #[tokio::test]
    async fn static_single_runtime_blocks_parallel_session_assignment() {
        let manager = SessionRuntimeManager::static_single("/tmp/bpane.sock".to_string());
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
        let manager = SessionRuntimeManager::static_single("/tmp/bpane.sock".to_string());
        let session_a = Uuid::now_v7();
        let session_b = Uuid::now_v7();

        manager.resolve(session_a).await.unwrap();
        manager.release(session_a).await;
        let resolved = manager.resolve(session_b).await.unwrap();

        assert_eq!(resolved.session_id, session_b);
    }
}
