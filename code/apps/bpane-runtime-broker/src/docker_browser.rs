use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bpane_runtime_contract::{
    ContainerLifecycleAction, RuntimeBrokerPolicy, RuntimeOperation, RuntimeOperationRequest,
    RuntimeOperationResult,
};

use crate::{ExecutionError, ExecutionErrorCode, RuntimeOperationExecutor};

mod backend;
mod config;
mod materialize;

use backend::{BollardDockerContainerApi, DockerBackendError, DockerContainerApi};
pub use config::{BrowserRuntimeDockerConfig, BrowserRuntimeExtensionConfig};
use materialize::MaterializedBrowserLaunch;

/// Policy-validating Docker adapter for browser runtime operations.
pub struct BrowserRuntimeDockerAdapter {
    config: BrowserRuntimeDockerConfig,
    policy: RuntimeBrokerPolicy,
    backend: Arc<dyn DockerContainerApi>,
}

impl std::fmt::Debug for BrowserRuntimeDockerAdapter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BrowserRuntimeDockerAdapter")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl BrowserRuntimeDockerAdapter {
    /// Creates an adapter for a private HTTP Docker API endpoint.
    ///
    /// # Errors
    ///
    /// Returns a sanitized adapter failure when trusted policy or endpoint
    /// configuration is invalid.
    pub fn connect(
        config: BrowserRuntimeDockerConfig,
        docker_api_url: &str,
        timeout: Duration,
    ) -> Result<Self, ExecutionError> {
        let backend = Arc::new(BollardDockerContainerApi::connect(docker_api_url, timeout)?);
        Self::with_backend(config, backend)
    }

    fn with_backend(
        config: BrowserRuntimeDockerConfig,
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
        request: &bpane_runtime_contract::BrowserRuntimeLaunchRequest,
    ) -> Result<RuntimeOperationResult, ExecutionError> {
        let launch = MaterializedBrowserLaunch::new(&self.config, request)
            .map_err(|_| ExecutionErrorCode::AdapterFailed)?;
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
        Ok(RuntimeOperationResult::Accepted)
    }

    async fn lifecycle(
        &self,
        request: &bpane_runtime_contract::ContainerLifecycleRequest,
    ) -> Result<RuntimeOperationResult, ExecutionError> {
        let target = self.config.lifecycle_target(request);
        self.policy
            .authorize_container_lifecycle(&target)
            .map_err(|_| ExecutionErrorCode::AdapterFailed)?;
        let result = match request.action {
            ContainerLifecycleAction::Inspect => self.backend.inspect(&target.container_name).await,
            ContainerLifecycleAction::Stop => self.backend.stop(&target.container_name).await,
            ContainerLifecycleAction::Remove => self.backend.remove(&target.container_name).await,
        };
        match result {
            Ok(()) => match request.action {
                ContainerLifecycleAction::Inspect => Ok(RuntimeOperationResult::Exists),
                ContainerLifecycleAction::Stop | ContainerLifecycleAction::Remove => {
                    Ok(RuntimeOperationResult::Completed {
                        exit_code: None,
                        omitted_output_bytes: 0,
                    })
                }
            },
            Err(DockerBackendError::NotFound) => Ok(RuntimeOperationResult::Absent),
            Err(error) => Err(map_backend_error(error)),
        }
    }
}

#[async_trait]
impl RuntimeOperationExecutor for BrowserRuntimeDockerAdapter {
    async fn execute(
        &self,
        request: &RuntimeOperationRequest,
    ) -> Result<RuntimeOperationResult, ExecutionError> {
        match &request.operation {
            RuntimeOperation::LaunchBrowser(request) => self.launch(request).await,
            RuntimeOperation::ContainerLifecycle(request)
                if request.operation_kind
                    == bpane_runtime_contract::RuntimeOperationKind::BrowserRuntime =>
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
