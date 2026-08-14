use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bpane_runtime_contract::{
    ContainerLifecycleAction, RuntimeBrokerPolicy, RuntimeOperation, RuntimeOperationRequest,
    RuntimeOperationResult,
};
use tracing::Instrument;

use crate::{ExecutionError, ExecutionErrorCode, RuntimeOperationExecutor};

pub(crate) mod backend;
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
        trace_policy("launch", || {
            self.policy.authorize_launch(&launch.policy_spec)
        })
        .map_err(|_| ExecutionErrorCode::AdapterFailed)?;
        ignore_absent(
            trace_backend(
                "remove_existing",
                self.backend.remove(&launch.container_name),
            )
            .await,
        )?;
        trace_backend(
            "create",
            self.backend
                .create(&launch.container_name, launch.container),
        )
        .await
        .map_err(map_backend_error)?;
        if let Err(error) = trace_backend("start", self.backend.start(&launch.container_name)).await
        {
            let _ = trace_backend("cleanup", self.backend.remove(&launch.container_name)).await;
            return Err(map_backend_error(error));
        }
        Ok(RuntimeOperationResult::Accepted)
    }

    async fn lifecycle(
        &self,
        request: &bpane_runtime_contract::ContainerLifecycleRequest,
    ) -> Result<RuntimeOperationResult, ExecutionError> {
        let target = self.config.lifecycle_target(request);
        trace_policy(lifecycle_action_name(request.action), || {
            self.policy.authorize_container_lifecycle(&target)
        })
        .map_err(|_| ExecutionErrorCode::AdapterFailed)?;
        match request.action {
            ContainerLifecycleAction::Inspect => {
                match trace_backend("inspect", self.backend.inspect(&target.container_name)).await {
                    Ok(_) => Ok(RuntimeOperationResult::Exists),
                    Err(DockerBackendError::NotFound) => Ok(RuntimeOperationResult::Absent),
                    Err(error) => Err(map_backend_error(error)),
                }
            }
            ContainerLifecycleAction::Stop => lifecycle_result(
                trace_backend("stop", self.backend.stop(&target.container_name)).await,
            ),
            ContainerLifecycleAction::Remove => lifecycle_result(
                trace_backend("remove", self.backend.remove(&target.container_name)).await,
            ),
        }
    }
}

#[async_trait]
impl RuntimeOperationExecutor for BrowserRuntimeDockerAdapter {
    async fn check_readiness(&self) -> Result<(), ExecutionError> {
        trace_backend("ping", self.backend.ping())
            .await
            .map_err(map_backend_error)
    }

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

fn trace_policy<T>(
    action: &'static str,
    authorize: impl FnOnce() -> Result<T, bpane_runtime_contract::PolicyViolation>,
) -> Result<T, bpane_runtime_contract::PolicyViolation> {
    let span = tracing::info_span!(
        "browserpane.runtime.policy",
        otel.kind = "internal",
        otel.status_code = tracing::field::Empty,
        "browserpane.operation.kind" = "browser_runtime",
        "browserpane.operation.action" = action,
        "browserpane.result" = tracing::field::Empty,
    );
    let result = span.in_scope(authorize);
    span.record(
        "browserpane.result",
        if result.is_ok() { "accepted" } else { "denied" },
    );
    span.record(
        "otel.status_code",
        if result.is_ok() { "OK" } else { "ERROR" },
    );
    result
}

async fn trace_backend<T>(
    stage: &'static str,
    operation: impl Future<Output = Result<T, DockerBackendError>>,
) -> Result<T, DockerBackendError> {
    let span = tracing::info_span!(
        "browserpane.runtime.docker",
        otel.kind = "client",
        otel.status_code = tracing::field::Empty,
        "browserpane.operation.kind" = "browser_runtime",
        "browserpane.runtime.stage" = stage,
        "browserpane.result" = tracing::field::Empty,
    );
    let result = operation.instrument(span.clone()).await;
    let category = match &result {
        Ok(_) => "accepted",
        Err(DockerBackendError::NotFound) => "not_found",
        Err(DockerBackendError::Failed) => "failed",
    };
    span.record("browserpane.result", category);
    span.record(
        "otel.status_code",
        if matches!(&result, Err(DockerBackendError::Failed)) {
            "ERROR"
        } else {
            "OK"
        },
    );
    result
}

fn lifecycle_action_name(action: ContainerLifecycleAction) -> &'static str {
    match action {
        ContainerLifecycleAction::Inspect => "inspect",
        ContainerLifecycleAction::Stop => "stop",
        ContainerLifecycleAction::Remove => "remove",
    }
}

fn ignore_absent(result: Result<(), DockerBackendError>) -> Result<(), ExecutionError> {
    match result {
        Ok(()) | Err(DockerBackendError::NotFound) => Ok(()),
        Err(error) => Err(map_backend_error(error)),
    }
}

fn lifecycle_result(
    result: Result<(), DockerBackendError>,
) -> Result<RuntimeOperationResult, ExecutionError> {
    match result {
        Ok(()) => Ok(RuntimeOperationResult::Completed {
            exit_code: None,
            omitted_output_bytes: 0,
        }),
        Err(DockerBackendError::NotFound) => Ok(RuntimeOperationResult::Absent),
        Err(error) => Err(map_backend_error(error)),
    }
}

fn map_backend_error(_error: DockerBackendError) -> ExecutionError {
    ExecutionErrorCode::AdapterFailed.into()
}

#[cfg(test)]
mod tests;
