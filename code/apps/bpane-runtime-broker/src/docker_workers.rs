use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bpane_runtime_contract::{
    ContainerLifecycleAction, RuntimeBrokerPolicy, RuntimeOperation, RuntimeOperationKind,
    RuntimeOperationRequest, RuntimeOperationResult, WorkerExecutionState,
};

use crate::docker_browser::backend::{
    BollardDockerContainerApi, DockerBackendError, DockerContainerApi, DockerContainerState,
};
use crate::{ExecutionError, ExecutionErrorCode, RuntimeOperationExecutor};

mod config;
mod materialize;

pub use config::{
    RecordingWorkerDockerConfig, WorkerOidcConfig, WorkerRuntimeDockerConfig,
    WorkflowWorkerDockerConfig,
};
use materialize::MaterializedWorkerLaunch;

/// Policy-validating Docker adapter for workflow and recording workers.
pub struct WorkerRuntimeDockerAdapter {
    config: WorkerRuntimeDockerConfig,
    policy: RuntimeBrokerPolicy,
    backend: Arc<dyn DockerContainerApi>,
}

impl std::fmt::Debug for WorkerRuntimeDockerAdapter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WorkerRuntimeDockerAdapter")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl WorkerRuntimeDockerAdapter {
    /// Creates a worker adapter for a private HTTP Docker API endpoint.
    ///
    /// # Errors
    ///
    /// Returns a sanitized adapter failure for invalid trusted configuration.
    pub fn connect(
        config: WorkerRuntimeDockerConfig,
        docker_api_url: &str,
        timeout: Duration,
    ) -> Result<Self, ExecutionError> {
        let backend = Arc::new(BollardDockerContainerApi::connect(docker_api_url, timeout)?);
        Self::with_backend(config, backend)
    }

    fn with_backend(
        config: WorkerRuntimeDockerConfig,
        backend: Arc<dyn DockerContainerApi>,
    ) -> Result<Self, ExecutionError> {
        let policy = config
            .build_policy()
            .map_err(|_| ExecutionErrorCode::AdapterFailed)?;
        Ok(Self {
            config,
            policy,
            backend,
        })
    }

    async fn launch(
        &self,
        launch: Result<MaterializedWorkerLaunch, &'static str>,
    ) -> Result<RuntimeOperationResult, ExecutionError> {
        let launch = launch.map_err(|_| ExecutionErrorCode::AdapterFailed)?;
        self.policy
            .authorize_launch(&launch.policy_spec)
            .map_err(|_| ExecutionErrorCode::AdapterFailed)?;
        ignore_absent(self.backend.remove(&launch.container_name).await)?;
        self.backend
            .create(&launch.container_name, launch.container)
            .await
            .map_err(map_backend_error)?;
        if let Err(error) = self.backend.start(&launch.container_name).await {
            let _ = self.backend.remove(&launch.container_name).await;
            return Err(map_backend_error(error));
        }
        if let Err(error) = self
            .backend
            .send_stdin(&launch.container_name, launch.secrets)
            .await
        {
            let _ = self.backend.remove(&launch.container_name).await;
            return Err(map_backend_error(error));
        }
        Ok(RuntimeOperationResult::Accepted)
    }

    async fn lifecycle(
        &self,
        request: &bpane_runtime_contract::ContainerLifecycleRequest,
    ) -> Result<RuntimeOperationResult, ExecutionError> {
        let target = self
            .config
            .lifecycle_target(request)
            .map_err(|_| ExecutionErrorCode::AdapterUnavailable)?;
        self.policy
            .authorize_container_lifecycle(&target)
            .map_err(|_| ExecutionErrorCode::AdapterFailed)?;
        match request.action {
            ContainerLifecycleAction::Inspect => {
                match self.backend.inspect(&target.container_name).await {
                    Ok(DockerContainerState::Running) => Ok(RuntimeOperationResult::WorkerState {
                        execution_state: WorkerExecutionState::Running,
                        exit_code: None,
                    }),
                    Ok(DockerContainerState::Exited { exit_code }) => {
                        Ok(RuntimeOperationResult::WorkerState {
                            execution_state: WorkerExecutionState::Exited,
                            exit_code,
                        })
                    }
                    Err(DockerBackendError::NotFound) => Ok(RuntimeOperationResult::Absent),
                    Err(error) => Err(map_backend_error(error)),
                }
            }
            ContainerLifecycleAction::Stop => match self.backend.stop(&target.container_name).await
            {
                Ok(()) => Ok(RuntimeOperationResult::Completed {
                    exit_code: None,
                    omitted_output_bytes: 0,
                }),
                Err(DockerBackendError::NotFound) => Ok(RuntimeOperationResult::Absent),
                Err(error) => Err(map_backend_error(error)),
            },
            ContainerLifecycleAction::Remove => {
                match self.backend.remove(&target.container_name).await {
                    Ok(()) => Ok(RuntimeOperationResult::Completed {
                        exit_code: None,
                        omitted_output_bytes: 0,
                    }),
                    Err(DockerBackendError::NotFound) => Ok(RuntimeOperationResult::Absent),
                    Err(error) => Err(map_backend_error(error)),
                }
            }
        }
    }
}

#[async_trait]
impl RuntimeOperationExecutor for WorkerRuntimeDockerAdapter {
    async fn check_readiness(&self) -> Result<(), ExecutionError> {
        self.backend.ping().await.map_err(map_backend_error)
    }

    async fn execute(
        &self,
        request: &RuntimeOperationRequest,
    ) -> Result<RuntimeOperationResult, ExecutionError> {
        match &request.operation {
            RuntimeOperation::LaunchWorkflow(request) => {
                let config = self
                    .config
                    .workflow
                    .as_ref()
                    .ok_or(ExecutionErrorCode::AdapterUnavailable)?;
                self.launch(MaterializedWorkerLaunch::workflow(config, request))
                    .await
            }
            RuntimeOperation::LaunchRecording(request) => {
                let config = self
                    .config
                    .recording
                    .as_ref()
                    .ok_or(ExecutionErrorCode::AdapterUnavailable)?;
                self.launch(MaterializedWorkerLaunch::recording(config, request))
                    .await
            }
            RuntimeOperation::ContainerLifecycle(request)
                if matches!(
                    request.operation_kind,
                    RuntimeOperationKind::WorkflowWorker | RuntimeOperationKind::RecordingWorker
                ) =>
            {
                self.lifecycle(request).await
            }
            _ => Err(ExecutionErrorCode::AdapterUnavailable.into()),
        }
    }
}

fn ignore_absent(result: Result<(), DockerBackendError>) -> Result<(), ExecutionError> {
    match result {
        Ok(()) | Err(DockerBackendError::NotFound) => Ok(()),
        Err(error) => Err(map_backend_error(error)),
    }
}

fn map_backend_error(_error: DockerBackendError) -> ExecutionError {
    ExecutionErrorCode::AdapterFailed.into()
}

#[cfg(test)]
mod tests;
