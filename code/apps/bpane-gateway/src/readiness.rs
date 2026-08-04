use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures_util::future::join_all;
use serde::Serialize;
use tracing::{info, warn};

use crate::credentials::CredentialProvider;
use crate::lifecycle::{GatewayLifecycle, GatewayLifecycleState};
use crate::recording::RecordingArtifactStore;
use crate::session_control::SessionStore;
use crate::session_manager::SessionManager;
use crate::workspaces::WorkspaceFileStore;

const READINESS_UNKNOWN: u8 = 0;
const READINESS_READY: u8 = 1;
const READINESS_NOT_READY: u8 = 2;

type DependencyFuture<'a> = Pin<Box<dyn Future<Output = DependencyReadiness> + Send + 'a>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ReadinessStatus {
    Ready,
    NotReady,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct DependencyReadiness {
    pub(crate) name: &'static str,
    pub(crate) status: ReadinessStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) reason: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct GatewayReadinessSnapshot {
    pub(crate) status: ReadinessStatus,
    pub(crate) lifecycle: GatewayLifecycleState,
    pub(crate) checks: Vec<DependencyReadiness>,
}

pub(crate) struct GatewayReadiness {
    lifecycle: Arc<GatewayLifecycle>,
    session_store: SessionStore,
    session_manager: Arc<SessionManager>,
    credential_provider: Option<Arc<CredentialProvider>>,
    recording_artifact_store: Arc<RecordingArtifactStore>,
    workspace_file_store: Arc<WorkspaceFileStore>,
    check_timeout: Duration,
    last_observation: AtomicU8,
}

impl GatewayReadiness {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        lifecycle: Arc<GatewayLifecycle>,
        session_store: SessionStore,
        session_manager: Arc<SessionManager>,
        credential_provider: Option<Arc<CredentialProvider>>,
        recording_artifact_store: Arc<RecordingArtifactStore>,
        workspace_file_store: Arc<WorkspaceFileStore>,
        check_timeout: Duration,
    ) -> Self {
        Self {
            lifecycle,
            session_store,
            session_manager,
            credential_provider,
            recording_artifact_store,
            workspace_file_store,
            check_timeout,
            last_observation: AtomicU8::new(READINESS_UNKNOWN),
        }
    }

    pub(crate) async fn snapshot(&self) -> GatewayReadinessSnapshot {
        let lifecycle = self.lifecycle.state();
        if lifecycle != GatewayLifecycleState::Running {
            self.record_lifecycle_withdrawal(lifecycle);
            return GatewayReadinessSnapshot {
                status: ReadinessStatus::NotReady,
                lifecycle,
                checks: Vec::new(),
            };
        }

        let mut futures: Vec<DependencyFuture<'_>> = vec![
            Box::pin(run_dependency_check(
                "session_store",
                self.check_timeout,
                self.session_store.check_readiness(),
            )),
            Box::pin(run_dependency_check(
                "runtime_manager",
                self.check_timeout,
                self.session_manager.check_readiness(),
            )),
            Box::pin(run_dependency_check(
                "recording_artifact_store",
                self.check_timeout,
                self.recording_artifact_store.check_readiness(),
            )),
            Box::pin(run_dependency_check(
                "workspace_file_store",
                self.check_timeout,
                self.workspace_file_store.check_readiness(),
            )),
        ];
        if let Some(provider) = &self.credential_provider {
            futures.push(Box::pin(run_dependency_check(
                "credential_provider",
                self.check_timeout,
                provider.check_readiness(),
            )));
        }

        let checks = join_all(futures).await;
        let ready = checks
            .iter()
            .all(|check| check.status == ReadinessStatus::Ready);
        self.record_transition(ready, &checks);
        GatewayReadinessSnapshot {
            status: if ready {
                ReadinessStatus::Ready
            } else {
                ReadinessStatus::NotReady
            },
            lifecycle,
            checks,
        }
    }

    fn record_transition(&self, ready: bool, checks: &[DependencyReadiness]) {
        let next = if ready {
            READINESS_READY
        } else {
            READINESS_NOT_READY
        };
        let previous = self.last_observation.swap(next, Ordering::AcqRel);
        if previous == next {
            return;
        }
        if ready {
            info!("gateway readiness restored");
            return;
        }
        let failed = checks
            .iter()
            .filter(|check| check.status == ReadinessStatus::NotReady)
            .map(|check| check.name)
            .collect::<Vec<_>>();
        warn!(failed_dependencies = ?failed, "gateway is not ready");
    }

    fn record_lifecycle_withdrawal(&self, lifecycle: GatewayLifecycleState) {
        let previous = self
            .last_observation
            .swap(READINESS_NOT_READY, Ordering::AcqRel);
        if previous != READINESS_NOT_READY {
            info!(
                ?lifecycle,
                "gateway readiness withdrawn for lifecycle transition"
            );
        }
    }
}

async fn run_dependency_check<F, E>(
    name: &'static str,
    timeout: Duration,
    check: F,
) -> DependencyReadiness
where
    F: Future<Output = Result<(), E>>,
{
    match tokio::time::timeout(timeout, check).await {
        Ok(Ok(())) => DependencyReadiness {
            name,
            status: ReadinessStatus::Ready,
            reason: None,
        },
        Ok(Err(_)) => DependencyReadiness {
            name,
            status: ReadinessStatus::NotReady,
            reason: Some("dependency check failed"),
        },
        Err(_) => DependencyReadiness {
            name,
            status: ReadinessStatus::NotReady,
            reason: Some("dependency check timed out"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn dependency_check_reports_success_failure_and_timeout() {
        let success = run_dependency_check("success", Duration::from_secs(1), async {
            Ok::<(), ()>(())
        })
        .await;
        let failure = run_dependency_check("failure", Duration::from_secs(1), async {
            Err::<(), ()>(())
        })
        .await;
        let timeout = run_dependency_check("timeout", Duration::from_millis(1), async {
            tokio::time::sleep(Duration::from_secs(1)).await;
            Ok::<(), ()>(())
        })
        .await;

        assert_eq!(success.status, ReadinessStatus::Ready);
        assert_eq!(failure.reason, Some("dependency check failed"));
        assert_eq!(timeout.reason, Some("dependency check timed out"));
    }
}
