use thiserror::Error;

/// Stable broker-client failure codes safe for control-plane logs and decisions.
#[derive(Debug, Clone, Copy, Eq, Error, PartialEq)]
pub enum RuntimeBrokerClientErrorCode {
    /// Local endpoint, limit, or OAuth configuration is invalid.
    #[error("runtime broker client configuration is invalid")]
    InvalidConfiguration,
    /// OAuth client-credentials token acquisition failed.
    #[error("runtime broker service authentication is unavailable")]
    TokenUnavailable,
    /// Typed request serialization failed or exceeded the fixed limit.
    #[error("runtime broker request is invalid")]
    InvalidRequest,
    /// The broker did not respond within the configured deadline.
    #[error("runtime broker request timed out")]
    TimedOut,
    /// Network transport to the broker failed.
    #[error("runtime broker is unreachable")]
    Unreachable,
    /// Broker rejected the service identity.
    #[error("runtime broker rejected service authentication")]
    AuthenticationRejected,
    /// Broker denied a conflicting retry or replay.
    #[error("runtime broker rejected a conflicting operation")]
    Conflict,
    /// Broker concurrency or idempotency capacity is exhausted.
    #[error("runtime broker is overloaded")]
    Overloaded,
    /// Broker or its selected adapter is unavailable.
    #[error("runtime broker operation is unavailable")]
    Unavailable,
    /// Broker rejected the operation for another stable policy reason.
    #[error("runtime broker rejected the operation")]
    Rejected,
    /// Response body exceeded the fixed client limit.
    #[error("runtime broker response exceeds the size limit")]
    ResponseTooLarge,
    /// Broker response was malformed or did not correlate with the request.
    #[error("runtime broker response is invalid")]
    InvalidResponse,
}

/// Sanitized broker-client failure.
#[derive(Debug, Clone, Copy, Eq, Error, PartialEq)]
#[error("runtime broker client failed: {code}")]
pub struct RuntimeBrokerClientError {
    /// Stable failure code. Raw OAuth, HTTP, and response details are omitted.
    pub code: RuntimeBrokerClientErrorCode,
}

impl From<RuntimeBrokerClientErrorCode> for RuntimeBrokerClientError {
    fn from(code: RuntimeBrokerClientErrorCode) -> Self {
        Self { code }
    }
}
