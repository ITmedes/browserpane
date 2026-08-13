use async_trait::async_trait;
use bpane_runtime_contract::{RuntimeOperationRequest, RuntimeOperationResult};
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
    /// Executes one already-authenticated and contract-valid operation.
    async fn execute(
        &self,
        request: &RuntimeOperationRequest,
    ) -> Result<RuntimeOperationResult, ExecutionError>;
}

/// Safe foundation executor used until operation adapters are enabled.
#[derive(Debug, Default)]
pub struct RejectingRuntimeExecutor;

#[async_trait]
impl RuntimeOperationExecutor for RejectingRuntimeExecutor {
    async fn execute(
        &self,
        _request: &RuntimeOperationRequest,
    ) -> Result<RuntimeOperationResult, ExecutionError> {
        Err(ExecutionErrorCode::AdapterUnavailable.into())
    }
}
