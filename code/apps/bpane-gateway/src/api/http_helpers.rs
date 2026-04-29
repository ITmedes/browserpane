use super::*;

pub(super) fn required_header_string(
    headers: &HeaderMap,
    name: &str,
) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
    let value = headers
        .get(name)
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("missing required header {name}"),
                }),
            )
        })?
        .to_str()
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("header {name} must be valid UTF-8"),
                }),
            )
        })?
        .trim()
        .to_string();
    if value.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("header {name} must not be empty"),
            }),
        ));
    }
    Ok(value)
}

pub(super) fn parse_optional_json_object_header(
    headers: &HeaderMap,
    name: &str,
) -> Result<Option<Value>, (StatusCode, Json<ErrorResponse>)> {
    let Some(raw) = headers.get(name) else {
        return Ok(None);
    };
    let raw = raw.to_str().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("header {name} must be valid UTF-8"),
            }),
        )
    })?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let value = serde_json::from_str::<Value>(trimmed).map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("header {name} must contain valid JSON: {error}"),
            }),
        )
    })?;
    if !value.is_object() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("header {name} must contain a JSON object"),
            }),
        ));
    }
    Ok(Some(value))
}

pub(super) fn header_value_or_default(value: &str, fallback: &'static str) -> HeaderValue {
    HeaderValue::from_str(value).unwrap_or_else(|_| HeaderValue::from_static(fallback))
}

pub(super) fn sanitize_content_disposition_filename(file_name: &str) -> String {
    file_name.replace(['"', '\\'], "_")
}
