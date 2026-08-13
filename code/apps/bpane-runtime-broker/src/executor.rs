use std::sync::Arc;

use async_trait::async_trait;
use bpane_runtime_contract::{
    RuntimeOperationKind, RuntimeOperationRequest, RuntimeOperationResult,
};
use thiserror::Error;

/// Stable executor failure codes safe to return and audit.
#[derive(Debug, Clone, Copy, Eq, Error, PartialEq)]
pub enum ExecutionErrorCode {
    /// No runtime adapter is enabled for this operation yet.
    #[error("runtime adapter is not available")]
    AdapterUnavailable,
    /// The adapter exceeded its bounded execution deadline.
    #[error("runtime adapter timed out")]
    TimedOut,
    /// The adapter failed without exposing raw backend output.
    #[error("runtime adapter failed")]
    AdapterFailed,
}

/// Sanitized runtime executor failure.
#[derive(Debug, Clone, Copy, Eq, Error, PartialEq)]
#[error("runtime operation failed: {code}")]
pub struct ExecutionError {
    /// Stable failure code.
    pub code: ExecutionErrorCode,
}

impl From<ExecutionErrorCode> for ExecutionError {
    fn from(code: ExecutionErrorCode) -> Self {
        Self { code }
    }
}

/// Adapter boundary for approved BrowserPane runtime operations.
#[async_trait]
pub trait RuntimeOperationExecutor: Send + Sync {
    /// Checks whether the selected adapter dependency is reachable.
    async fn check_readiness(&self) -> Result<(), ExecutionError> {
        Ok(())
    }

    /// Executes one already-authenticated and contract-valid operation.
    async fn execute(
        &self,
        request: &RuntimeOperationRequest,
    ) -> Result<RuntimeOperationResult, ExecutionError>;

    /// Executes a storage operation with a separately bounded binary payload.
    async fn execute_storage(
        &self,
        _request: &RuntimeOperationRequest,
        _payload: Option<&[u8]>,
    ) -> Result<StorageExecutionOutput, ExecutionError> {
        Err(ExecutionErrorCode::AdapterUnavailable.into())
    }
}

/// Storage executor result with optional binary response bytes.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct StorageExecutionOutput {
    /// Sanitized operation result persisted in the idempotency ledger.
    pub result: RuntimeOperationResult,
    /// Optional separately transferred response payload.
    pub payload: Option<Vec<u8>>,
}

/// Safe foundation executor used until operation adapters are enabled.
#[derive(Debug, Default)]
pub struct RejectingRuntimeExecutor;

pub(crate) struct RoutedRuntimeExecutor {
    browser: Arc<dyn RuntimeOperationExecutor>,
    workers: Arc<dyn RuntimeOperationExecutor>,
}

pub(crate) struct StorageRoutedRuntimeExecutor {
    primary: Arc<dyn RuntimeOperationExecutor>,
    storage: Arc<dyn RuntimeOperationExecutor>,
}

impl StorageRoutedRuntimeExecutor {
    pub(crate) fn new(
        primary: Arc<dyn RuntimeOperationExecutor>,
        storage: Arc<dyn RuntimeOperationExecutor>,
    ) -> Self {
        Self { primary, storage }
    }
}

impl std::fmt::Debug for StorageRoutedRuntimeExecutor {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("StorageRoutedRuntimeExecutor([REDACTED])")
    }
}

impl RoutedRuntimeExecutor {
    pub(crate) fn new(
        browser: Arc<dyn RuntimeOperationExecutor>,
        workers: Arc<dyn RuntimeOperationExecutor>,
    ) -> Self {
        Self { browser, workers }
    }
}

impl std::fmt::Debug for RoutedRuntimeExecutor {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("RoutedRuntimeExecutor([REDACTED])")
    }
}

#[async_trait]
impl RuntimeOperationExecutor for RoutedRuntimeExecutor {
    async fn check_readiness(&self) -> Result<(), ExecutionError> {
        self.browser.check_readiness().await?;
        self.workers.check_readiness().await
    }

    async fn execute(
        &self,
        request: &RuntimeOperationRequest,
    ) -> Result<RuntimeOperationResult, ExecutionError> {
        match request.operation.kind() {
            RuntimeOperationKind::BrowserRuntime => self.browser.execute(request).await,
            RuntimeOperationKind::WorkflowWorker | RuntimeOperationKind::RecordingWorker => {
                self.workers.execute(request).await
            }
            RuntimeOperationKind::StorageHelper => {
                Err(ExecutionErrorCode::AdapterUnavailable.into())
            }
        }
    }
}

#[async_trait]
impl RuntimeOperationExecutor for StorageRoutedRuntimeExecutor {
    async fn check_readiness(&self) -> Result<(), ExecutionError> {
        self.primary.check_readiness().await?;
        self.storage.check_readiness().await
    }

    async fn execute(
        &self,
        request: &RuntimeOperationRequest,
    ) -> Result<RuntimeOperationResult, ExecutionError> {
        if request.operation.kind() == RuntimeOperationKind::StorageHelper {
            self.storage.execute(request).await
        } else {
            self.primary.execute(request).await
        }
    }

    async fn execute_storage(
        &self,
        request: &RuntimeOperationRequest,
        payload: Option<&[u8]>,
    ) -> Result<StorageExecutionOutput, ExecutionError> {
        self.storage.execute_storage(request, payload).await
    }
}

#[async_trait]
impl RuntimeOperationExecutor for RejectingRuntimeExecutor {
    async fn execute(
        &self,
        _request: &RuntimeOperationRequest,
    ) -> Result<RuntimeOperationResult, ExecutionError> {
        Err(ExecutionErrorCode::AdapterUnavailable.into())
    }
}
