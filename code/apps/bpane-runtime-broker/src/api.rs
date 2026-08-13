use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Bytes;
use axum::extract::rejection::BytesRejection;
use axum::extract::{Extension, Request, State};
use axum::http::header::AUTHORIZATION;
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use bpane_runtime_contract::{
    BrokerApiVersion, ContractErrorCode, RuntimeBrokerAuditEventBuilder, RuntimeOperationRequest,
    RuntimeOperationResponse,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::sync::Semaphore;
use tokio::time::timeout;

use crate::auth::{AuthenticationError, AuthenticationErrorCode};
use crate::executor::{ExecutionError, ExecutionErrorCode};
use crate::ledger::LedgerDecision;
use crate::{BrokerAuthenticator, OperationLedger, RuntimeOperationExecutor, ServicePrincipal};

use self::media::{operation_response, require_contract_media_type};

mod media;

/// Bounded HTTP operation settings.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct BrokerApiSettings {
    /// Maximum JSON body bytes.
    pub max_request_bytes: NonZeroUsize,
    /// Maximum in-flight executor calls.
    pub max_concurrent: NonZeroUsize,
    /// Hard operation deadline.
    pub operation_timeout: Duration,
}

/// Stable public broker API error codes.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BrokerApiErrorCode {
    AuthenticationRequired,
    AuthenticationMalformed,
    AuthenticationExpired,
    AuthenticationIssuerInvalid,
    AuthenticationAudienceInvalid,
    AuthenticationClientDenied,
    AuthenticationKeysUnavailable,
    RequestMalformed,
    RequestTooLarge,
    UnsupportedMediaType,
    InvalidResourceId,
    InvalidOperationParameters,
    PayloadDeclarationRequired,
    PayloadDeclarationNotAllowed,
    IdempotencyConflict,
    ReplayConflict,
    CapacityExceeded,
    OperationOverloaded,
    OperationTimedOut,
    AdapterUnavailable,
    AdapterFailed,
}

#[derive(Debug, Serialize)]
struct ErrorResource {
    code: BrokerApiErrorCode,
    message: &'static str,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: ErrorResource,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    code: BrokerApiErrorCode,
    message: &'static str,
}

impl ApiError {
    fn new(status: StatusCode, code: BrokerApiErrorCode, message: &'static str) -> Self {
        Self {
            status,
            code,
            message,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorBody {
                error: ErrorResource {
                    code: self.code,
                    message: self.message,
                },
            }),
        )
            .into_response()
    }
}

/// Shared broker HTTP state.
#[derive(Clone)]
pub struct BrokerState {
    authenticator: Arc<dyn BrokerAuthenticator>,
    executor: Arc<dyn RuntimeOperationExecutor>,
    ledger: Arc<OperationLedger>,
    permits: Arc<Semaphore>,
    settings: BrokerApiSettings,
}

impl BrokerState {
    /// Builds state from explicit security and adapter boundaries.
    pub fn new(
        authenticator: Arc<dyn BrokerAuthenticator>,
        executor: Arc<dyn RuntimeOperationExecutor>,
        ledger: Arc<OperationLedger>,
        settings: BrokerApiSettings,
    ) -> Self {
        Self {
            authenticator,
            executor,
            ledger,
            permits: Arc::new(Semaphore::new(settings.max_concurrent.get())),
            settings,
        }
    }
}

/// Builds the internal broker router.
pub fn build_router(state: BrokerState) -> Router {
    let operation = Router::new()
        .route("/v1/operations", post(run_operation))
        .route_layer(axum::extract::DefaultBodyLimit::max(
            state.settings.max_request_bytes.get(),
        ))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            authenticate_request,
        ));
    Router::new()
        .route("/livez", get(livez))
        .route("/readyz", get(readyz))
        .merge(operation)
        .with_state(state)
}

async fn livez() -> StatusCode {
    StatusCode::OK
}

async fn readyz() -> StatusCode {
    StatusCode::OK
}

async fn authenticate_request(
    State(state): State<BrokerState>,
    mut request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let token = bearer_token(request.headers())?;
    let principal = state
        .authenticator
        .authenticate(token)
        .await
        .map_err(map_authentication_error)?;
    request.extensions_mut().insert(principal);
    Ok(next.run(request).await)
}

fn bearer_token(headers: &HeaderMap) -> Result<&str, ApiError> {
    let value = headers.get(AUTHORIZATION).ok_or_else(|| {
        ApiError::new(
            StatusCode::UNAUTHORIZED,
            BrokerApiErrorCode::AuthenticationRequired,
            "a bearer service credential is required",
        )
    })?;
    let value = value.to_str().map_err(|_| malformed_authentication())?;
    let (scheme, token) = value.split_once(' ').ok_or_else(malformed_authentication)?;
    if !scheme.eq_ignore_ascii_case("bearer") || token.is_empty() || token.contains(' ') {
        return Err(malformed_authentication());
    }
    Ok(token)
}

fn malformed_authentication() -> ApiError {
    ApiError::new(
        StatusCode::UNAUTHORIZED,
        BrokerApiErrorCode::AuthenticationMalformed,
        "the bearer service credential is malformed",
    )
}

async fn run_operation(
    State(state): State<BrokerState>,
    Extension(principal): Extension<ServicePrincipal>,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Result<Response, ApiError> {
    require_contract_media_type(&headers)?;
    let body = body.map_err(map_body_rejection)?;
    let request: RuntimeOperationRequest = serde_json::from_slice(&body).map_err(|_| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            BrokerApiErrorCode::RequestMalformed,
            "the runtime operation request is malformed",
        )
    })?;
    request.validate().map_err(|error| {
        let (code, message) = map_contract_error(error.code);
        ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, code, message)
    })?;
    let fingerprint = request_fingerprint(&request)?;
    let idempotency_key = request.idempotency_key.as_str();

    loop {
        match state
            .ledger
            .begin(
                &principal.subject,
                idempotency_key,
                request.request_id,
                fingerprint,
            )
            .await
        {
            LedgerDecision::Execute => break,
            LedgerDecision::Cached(response) => return Ok(operation_response(response, true)),
            LedgerDecision::Wait(notify) => {
                if timeout(state.settings.operation_timeout, notify.notified())
                    .await
                    .is_err()
                {
                    return Err(operation_timeout_error());
                }
            }
            LedgerDecision::IdempotencyConflict => return Err(idempotency_conflict()),
            LedgerDecision::ReplayConflict => return Err(replay_conflict()),
            LedgerDecision::CapacityExceeded => return Err(capacity_exceeded()),
        }
    }

    let permit = match Arc::clone(&state.permits).try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => {
            state
                .ledger
                .abort(&principal.subject, idempotency_key)
                .await;
            return Err(operation_overloaded());
        }
    };
    let result = timeout(
        state.settings.operation_timeout,
        state.executor.execute(&request),
    )
    .await;
    drop(permit);
    let result = match result {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => {
            state
                .ledger
                .abort(&principal.subject, idempotency_key)
                .await;
            audit_failure(&request);
            return Err(map_execution_error(error));
        }
        Err(_) => {
            state
                .ledger
                .abort(&principal.subject, idempotency_key)
                .await;
            audit_failure(&request);
            return Err(operation_timeout_error());
        }
    };
    let response = RuntimeOperationResponse {
        api_version: BrokerApiVersion::V1,
        request_id: request.request_id,
        result,
    };
    state
        .ledger
        .complete(&principal.subject, idempotency_key, response.clone())
        .await;
    audit_accepted(&request);
    Ok(operation_response(response, false))
}

fn request_fingerprint(request: &RuntimeOperationRequest) -> Result<[u8; 32], ApiError> {
    let canonical = serde_json::to_vec(request).map_err(|_| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            BrokerApiErrorCode::RequestMalformed,
            "the runtime operation request is malformed",
        )
    })?;
    Ok(Sha256::digest(canonical).into())
}

fn audit_accepted(request: &RuntimeOperationRequest) {
    let event =
        RuntimeBrokerAuditEventBuilder::new(request, idempotency_fingerprint(request)).accepted();
    tracing::info!(
        request_id = %event.request_id,
        operation_kind = ?event.resource.operation_kind,
        resource_id = %event.resource.resource_id,
        outcome = ?event.outcome,
        idempotency_key_fingerprint = %event.idempotency_key_fingerprint,
        "runtime broker operation"
    );
}

fn audit_failure(request: &RuntimeOperationRequest) {
    let event =
        RuntimeBrokerAuditEventBuilder::new(request, idempotency_fingerprint(request)).failed();
    tracing::warn!(
        request_id = %event.request_id,
        operation_kind = ?event.resource.operation_kind,
        resource_id = %event.resource.resource_id,
        outcome = ?event.outcome,
        idempotency_key_fingerprint = %event.idempotency_key_fingerprint,
        "runtime broker operation"
    );
}

fn idempotency_fingerprint(request: &RuntimeOperationRequest) -> String {
    hex::encode(Sha256::digest(request.idempotency_key.as_str().as_bytes()))
}

fn map_body_rejection(rejection: BytesRejection) -> ApiError {
    if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE {
        ApiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            BrokerApiErrorCode::RequestTooLarge,
            "the runtime operation request exceeds the size limit",
        )
    } else {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            BrokerApiErrorCode::RequestMalformed,
            "the runtime operation request is malformed",
        )
    }
}

fn map_authentication_error(error: AuthenticationError) -> ApiError {
    let (status, code, message) = match error.code {
        AuthenticationErrorCode::Expired => (
            StatusCode::UNAUTHORIZED,
            BrokerApiErrorCode::AuthenticationExpired,
            "the service credential is expired",
        ),
        AuthenticationErrorCode::InvalidIssuer => (
            StatusCode::FORBIDDEN,
            BrokerApiErrorCode::AuthenticationIssuerInvalid,
            "the service credential issuer is not approved",
        ),
        AuthenticationErrorCode::InvalidAudience => (
            StatusCode::FORBIDDEN,
            BrokerApiErrorCode::AuthenticationAudienceInvalid,
            "the service credential audience is not approved",
        ),
        AuthenticationErrorCode::ClientNotAllowed => (
            StatusCode::FORBIDDEN,
            BrokerApiErrorCode::AuthenticationClientDenied,
            "the service client is not authorized",
        ),
        AuthenticationErrorCode::KeyFetchFailed | AuthenticationErrorCode::KeyParseFailed => (
            StatusCode::SERVICE_UNAVAILABLE,
            BrokerApiErrorCode::AuthenticationKeysUnavailable,
            "service identity keys are temporarily unavailable",
        ),
        _ => (
            StatusCode::UNAUTHORIZED,
            BrokerApiErrorCode::AuthenticationMalformed,
            "the bearer service credential is invalid",
        ),
    };
    ApiError::new(status, code, message)
}

fn map_contract_error(code: ContractErrorCode) -> (BrokerApiErrorCode, &'static str) {
    match code {
        ContractErrorCode::InvalidResourceId => (
            BrokerApiErrorCode::InvalidResourceId,
            "the runtime operation contains an invalid resource identifier",
        ),
        ContractErrorCode::InvalidOperationParameters => (
            BrokerApiErrorCode::InvalidOperationParameters,
            "the runtime operation parameters are invalid",
        ),
        ContractErrorCode::PayloadDeclarationRequired => (
            BrokerApiErrorCode::PayloadDeclarationRequired,
            "the runtime operation requires a positive payload declaration",
        ),
        ContractErrorCode::PayloadDeclarationNotAllowed => (
            BrokerApiErrorCode::PayloadDeclarationNotAllowed,
            "the runtime operation does not accept a payload declaration",
        ),
    }
}

fn map_execution_error(error: ExecutionError) -> ApiError {
    match error.code {
        ExecutionErrorCode::AdapterUnavailable => ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            BrokerApiErrorCode::AdapterUnavailable,
            "the requested runtime adapter is not enabled",
        ),
        ExecutionErrorCode::TimedOut => operation_timeout_error(),
        ExecutionErrorCode::AdapterFailed => ApiError::new(
            StatusCode::BAD_GATEWAY,
            BrokerApiErrorCode::AdapterFailed,
            "the runtime adapter failed",
        ),
    }
}

fn idempotency_conflict() -> ApiError {
    ApiError::new(
        StatusCode::CONFLICT,
        BrokerApiErrorCode::IdempotencyConflict,
        "the idempotency key was already used for another operation",
    )
}

fn replay_conflict() -> ApiError {
    ApiError::new(
        StatusCode::CONFLICT,
        BrokerApiErrorCode::ReplayConflict,
        "the request identifier was already used with another idempotency key",
    )
}

fn capacity_exceeded() -> ApiError {
    ApiError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        BrokerApiErrorCode::CapacityExceeded,
        "the runtime operation ledger is at capacity",
    )
}

fn operation_overloaded() -> ApiError {
    ApiError::new(
        StatusCode::TOO_MANY_REQUESTS,
        BrokerApiErrorCode::OperationOverloaded,
        "the runtime broker concurrency limit is reached",
    )
}

fn operation_timeout_error() -> ApiError {
    ApiError::new(
        StatusCode::GATEWAY_TIMEOUT,
        BrokerApiErrorCode::OperationTimedOut,
        "the runtime operation exceeded its deadline",
    )
}

#[cfg(test)]
mod tests;
