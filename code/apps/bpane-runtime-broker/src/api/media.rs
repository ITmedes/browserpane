use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use bpane_runtime_contract::{RuntimeOperationResponse, RUNTIME_BROKER_V1_MEDIA_TYPE};

use super::{ApiError, BrokerApiErrorCode};

pub(super) fn operation_response(response: RuntimeOperationResponse, replayed: bool) -> Response {
    let mut response = (StatusCode::ACCEPTED, Json(response)).into_response();
    response.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static(RUNTIME_BROKER_V1_MEDIA_TYPE),
    );
    if replayed {
        response.headers_mut().insert(
            "x-bpane-idempotent-replay",
            HeaderValue::from_static("true"),
        );
    }
    response
}

pub(super) fn require_contract_media_type(headers: &HeaderMap) -> Result<(), ApiError> {
    let valid = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|value| value.trim() == RUNTIME_BROKER_V1_MEDIA_TYPE);
    if !valid {
        return Err(ApiError::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            BrokerApiErrorCode::UnsupportedMediaType,
            "the runtime operation media type is not supported",
        ));
    }
    Ok(())
}
