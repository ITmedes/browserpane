use std::sync::Arc;

use axum::extract::multipart::{Field, MultipartError, MultipartRejection};
use axum::extract::{Extension, Multipart, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use bpane_runtime_contract::{
    BrokerApiVersion, RuntimeOperation, RuntimeOperationRequest, RuntimeOperationResponse,
    RuntimeOperationResult, RUNTIME_BROKER_API_VERSION_HEADER, RUNTIME_BROKER_PAYLOAD_BYTES_HEADER,
    RUNTIME_BROKER_PAYLOAD_SHA256_HEADER, RUNTIME_BROKER_REQUEST_ID_HEADER,
    RUNTIME_BROKER_STORAGE_PAYLOAD_MEDIA_TYPE, RUNTIME_BROKER_V1_MEDIA_TYPE,
};
use sha2::{Digest, Sha256};
use tokio::time::timeout;

use crate::ledger::LedgerDecision;
use crate::{ServicePrincipal, StorageExecutionOutput};

use super::media::operation_response;
use super::{
    audit_accepted, audit_failure, capacity_exceeded, idempotency_conflict, map_contract_error,
    map_execution_error, operation_overloaded, operation_timeout_error, replay_conflict, ApiError,
    BrokerApiErrorCode, BrokerState,
};

const REQUEST_FIELD: &str = "request";
const PAYLOAD_FIELD: &str = "payload";

struct StorageTransferInput {
    request: RuntimeOperationRequest,
    payload: Option<Vec<u8>>,
}

pub(super) async fn run_storage_transfer(
    State(state): State<BrokerState>,
    Extension(principal): Extension<ServicePrincipal>,
    headers: HeaderMap,
    multipart: Result<Multipart, MultipartRejection>,
) -> Result<Response, ApiError> {
    require_multipart_media_type(&headers)?;
    let input = parse_transfer(
        multipart.map_err(map_multipart_rejection)?,
        state.settings.max_request_bytes.get(),
        state.settings.max_storage_payload_bytes.get(),
    )
    .await?;
    let fingerprint = transfer_fingerprint(&input.request, input.payload.as_deref())?;
    let idempotency_key = input.request.idempotency_key.as_str();
    let output_action = storage_action(&input.request)?.produces_output_payload();

    loop {
        match state
            .ledger
            .begin(
                &principal.subject,
                idempotency_key,
                input.request.request_id,
                fingerprint,
            )
            .await
        {
            LedgerDecision::Execute => break,
            LedgerDecision::Cached(response) if output_action => {
                let output = execute_storage(&state, &input, None).await?;
                if output.result != response.result {
                    return Err(adapter_output_error());
                }
                return storage_response(response, output.payload, true);
            }
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

    let output = execute_storage(&state, &input, Some(&principal.subject)).await?;
    let response = RuntimeOperationResponse {
        api_version: BrokerApiVersion::V1,
        request_id: input.request.request_id,
        result: output.result,
    };
    state
        .ledger
        .complete(&principal.subject, idempotency_key, response.clone())
        .await;
    audit_accepted(&input.request);
    storage_response(response, output.payload, false)
}

fn require_multipart_media_type(headers: &HeaderMap) -> Result<(), ApiError> {
    let valid = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("multipart/form-data"));
    if !valid {
        return Err(ApiError::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            BrokerApiErrorCode::UnsupportedMediaType,
            "the storage transfer must use multipart/form-data",
        ));
    }
    Ok(())
}

async fn parse_transfer(
    mut multipart: Multipart,
    max_request_bytes: usize,
    max_payload_bytes: usize,
) -> Result<StorageTransferInput, ApiError> {
    let request_field = multipart
        .next_field()
        .await
        .map_err(map_multipart_error)?
        .ok_or_else(request_malformed)?;
    if request_field.name() != Some(REQUEST_FIELD)
        || request_field.content_type() != Some(RUNTIME_BROKER_V1_MEDIA_TYPE)
    {
        return Err(request_malformed());
    }
    let request_bytes = read_field(request_field, max_request_bytes, false).await?;
    let request: RuntimeOperationRequest =
        serde_json::from_slice(&request_bytes).map_err(|_| request_malformed())?;
    request.validate().map_err(|error| {
        let (code, message) = map_contract_error(error.code);
        ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, code, message)
    })?;
    let action = storage_action(&request)?;
    if request.operation.kind() != bpane_runtime_contract::RuntimeOperationKind::StorageHelper {
        return Err(request_malformed());
    }
    let declared = match &request.operation {
        RuntimeOperation::RunStorageHelper(storage) => storage.declared_payload_bytes,
        _ => None,
    };
    if declared.is_some_and(|bytes| bytes > max_payload_bytes as u64) {
        return Err(payload_too_large());
    }

    let payload = match multipart.next_field().await.map_err(map_multipart_error)? {
        Some(field)
            if field.name() == Some(PAYLOAD_FIELD)
                && field.content_type() == Some(RUNTIME_BROKER_STORAGE_PAYLOAD_MEDIA_TYPE) =>
        {
            Some(read_field(field, max_payload_bytes, true).await?)
        }
        Some(_) => return Err(payload_unexpected()),
        None => None,
    };
    if multipart
        .next_field()
        .await
        .map_err(map_multipart_error)?
        .is_some()
    {
        return Err(payload_unexpected());
    }
    validate_input_payload(action.accepts_input_payload(), declared, payload.as_deref())?;
    Ok(StorageTransferInput { request, payload })
}

async fn read_field(
    mut field: Field<'_>,
    limit: usize,
    payload: bool,
) -> Result<Vec<u8>, ApiError> {
    let mut bytes = Vec::new();
    while let Some(chunk) = field.chunk().await.map_err(map_multipart_error)? {
        if bytes.len().saturating_add(chunk.len()) > limit {
            return Err(if payload {
                payload_too_large()
            } else {
                request_too_large()
            });
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn storage_action(
    request: &RuntimeOperationRequest,
) -> Result<bpane_runtime_contract::StorageHelperAction, ApiError> {
    match &request.operation {
        RuntimeOperation::RunStorageHelper(storage) => Ok(storage.action),
        _ => Err(request_malformed()),
    }
}

fn validate_input_payload(
    accepts_payload: bool,
    declared: Option<u64>,
    payload: Option<&[u8]>,
) -> Result<(), ApiError> {
    match (accepts_payload, declared, payload) {
        (true, Some(declared), Some(payload)) if declared == payload.len() as u64 => Ok(()),
        (true, _, None) => Err(payload_missing()),
        (true, _, Some(_)) => Err(payload_length_mismatch()),
        (false, _, Some(_)) => Err(payload_unexpected()),
        (false, _, None) => Ok(()),
    }
}

async fn execute_storage(
    state: &BrokerState,
    input: &StorageTransferInput,
    abort_subject: Option<&str>,
) -> Result<StorageExecutionOutput, ApiError> {
    let permit = match Arc::clone(&state.permits).try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => {
            abort_pending(state, input, abort_subject).await;
            return Err(operation_overloaded());
        }
    };
    let result = timeout(
        state.settings.operation_timeout,
        state
            .executor
            .execute_storage(&input.request, input.payload.as_deref()),
    )
    .await;
    drop(permit);
    let output = match result {
        Ok(Ok(output)) => output,
        Ok(Err(error)) => {
            abort_pending(state, input, abort_subject).await;
            audit_failure(&input.request);
            return Err(map_execution_error(error));
        }
        Err(_) => {
            abort_pending(state, input, abort_subject).await;
            audit_failure(&input.request);
            return Err(operation_timeout_error());
        }
    };
    if let Err(error) = validate_output(
        storage_action(&input.request)?.produces_output_payload(),
        &output,
        state.settings.max_storage_payload_bytes.get(),
    ) {
        abort_pending(state, input, abort_subject).await;
        audit_failure(&input.request);
        return Err(error);
    }
    Ok(output)
}

async fn abort_pending(state: &BrokerState, input: &StorageTransferInput, subject: Option<&str>) {
    if let Some(subject) = subject {
        state
            .ledger
            .abort(subject, input.request.idempotency_key.as_str())
            .await;
    }
}

fn validate_output(
    produces_payload: bool,
    output: &StorageExecutionOutput,
    max_payload_bytes: usize,
) -> Result<(), ApiError> {
    match (&output.result, output.payload.as_deref()) {
        (
            RuntimeOperationResult::StoragePayload {
                payload_bytes,
                sha256_hex,
            },
            Some(payload),
        ) if produces_payload
            && !payload.is_empty()
            && payload.len() <= max_payload_bytes
            && *payload_bytes == payload.len() as u64
            && sha256_hex == &hex::encode(Sha256::digest(payload)) =>
        {
            Ok(())
        }
        (RuntimeOperationResult::Absent, None) if produces_payload => Ok(()),
        (RuntimeOperationResult::StoragePayload { .. }, _) | (_, Some(_)) => {
            Err(adapter_output_error())
        }
        _ if produces_payload => Err(adapter_output_error()),
        _ => Ok(()),
    }
}

fn storage_response(
    response: RuntimeOperationResponse,
    payload: Option<Vec<u8>>,
    replayed: bool,
) -> Result<Response, ApiError> {
    let Some(payload) = payload else {
        return Ok(operation_response(response, replayed));
    };
    let RuntimeOperationResult::StoragePayload {
        payload_bytes,
        sha256_hex,
    } = &response.result
    else {
        return Err(adapter_output_error());
    };
    let mut binary = (StatusCode::OK, payload).into_response();
    let headers = binary.headers_mut();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static(RUNTIME_BROKER_STORAGE_PAYLOAD_MEDIA_TYPE),
    );
    insert_header(headers, RUNTIME_BROKER_API_VERSION_HEADER, "v1")?;
    insert_header(
        headers,
        RUNTIME_BROKER_REQUEST_ID_HEADER,
        &response.request_id.to_string(),
    )?;
    insert_header(
        headers,
        RUNTIME_BROKER_PAYLOAD_BYTES_HEADER,
        &payload_bytes.to_string(),
    )?;
    insert_header(headers, RUNTIME_BROKER_PAYLOAD_SHA256_HEADER, sha256_hex)?;
    if replayed {
        headers.insert(
            "x-bpane-idempotent-replay",
            HeaderValue::from_static("true"),
        );
    }
    Ok(binary)
}

fn insert_header(
    headers: &mut axum::http::HeaderMap,
    name: &'static str,
    value: &str,
) -> Result<(), ApiError> {
    let value = HeaderValue::from_str(value).map_err(|_| adapter_output_error())?;
    headers.insert(name, value);
    Ok(())
}

fn transfer_fingerprint(
    request: &RuntimeOperationRequest,
    payload: Option<&[u8]>,
) -> Result<[u8; 32], ApiError> {
    let canonical = serde_json::to_vec(request).map_err(|_| request_malformed())?;
    let mut digest = Sha256::new();
    digest.update((canonical.len() as u64).to_be_bytes());
    digest.update(canonical);
    if let Some(payload) = payload {
        digest.update(payload);
    }
    Ok(digest.finalize().into())
}

fn map_multipart_rejection(rejection: MultipartRejection) -> ApiError {
    if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE {
        payload_too_large()
    } else if rejection.status() == StatusCode::UNSUPPORTED_MEDIA_TYPE {
        ApiError::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            BrokerApiErrorCode::UnsupportedMediaType,
            "the storage transfer must use multipart/form-data",
        )
    } else {
        request_malformed()
    }
}

fn map_multipart_error(error: MultipartError) -> ApiError {
    if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
        payload_too_large()
    } else {
        request_malformed()
    }
}

fn request_malformed() -> ApiError {
    ApiError::new(
        StatusCode::BAD_REQUEST,
        BrokerApiErrorCode::RequestMalformed,
        "the storage transfer request is malformed",
    )
}

fn request_too_large() -> ApiError {
    ApiError::new(
        StatusCode::PAYLOAD_TOO_LARGE,
        BrokerApiErrorCode::RequestTooLarge,
        "the storage transfer metadata exceeds the size limit",
    )
}

fn payload_missing() -> ApiError {
    ApiError::new(
        StatusCode::UNPROCESSABLE_ENTITY,
        BrokerApiErrorCode::PayloadMissing,
        "the declared storage payload is missing",
    )
}

fn payload_unexpected() -> ApiError {
    ApiError::new(
        StatusCode::UNPROCESSABLE_ENTITY,
        BrokerApiErrorCode::PayloadUnexpected,
        "the storage operation does not accept this payload",
    )
}

fn payload_length_mismatch() -> ApiError {
    ApiError::new(
        StatusCode::UNPROCESSABLE_ENTITY,
        BrokerApiErrorCode::PayloadLengthMismatch,
        "the storage payload length does not match its declaration",
    )
}

fn payload_too_large() -> ApiError {
    ApiError::new(
        StatusCode::PAYLOAD_TOO_LARGE,
        BrokerApiErrorCode::PayloadTooLarge,
        "the storage payload exceeds the size limit",
    )
}

fn adapter_output_error() -> ApiError {
    ApiError::new(
        StatusCode::BAD_GATEWAY,
        BrokerApiErrorCode::AdapterFailed,
        "the runtime adapter returned an invalid storage result",
    )
}

#[cfg(test)]
mod tests;
