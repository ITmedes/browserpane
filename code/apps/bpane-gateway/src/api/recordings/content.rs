use axum::response::Response;

use super::*;

pub(super) async fn get_session_recording_content(
    headers: HeaderMap,
    Path((session_id, recording_id)): Path<(Uuid, Uuid)>,
    State(state): State<Arc<ApiState>>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let _session = authorize_visible_session_request(&headers, &state, session_id).await?;
    let recording = load_session_recording(&state, session_id, recording_id).await?;
    let artifact_ref = recording.artifact_ref.as_ref().ok_or_else(|| {
        if recording.state.is_terminal() {
            (
                StatusCode::GONE,
                Json(ErrorResponse {
                    error: format!("recording artifact for {recording_id} is no longer available"),
                }),
            )
        } else {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("recording {recording_id} does not have an artifact yet"),
                }),
            )
        }
    })?;
    let bytes = state
        .recording_artifact_store
        .read(artifact_ref)
        .await
        .map_err(|error| match error.io_kind() {
            Some(std::io::ErrorKind::NotFound) => (
                StatusCode::GONE,
                Json(ErrorResponse {
                    error: format!("recording artifact for {recording_id} is no longer available"),
                }),
            ),
            _ => (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: format!("failed to read recording artifact: {error}"),
                }),
            ),
        })?;

    let filename = format!("browserpane-{session_id}-{recording_id}.webm");
    let mime_type = recording
        .mime_type
        .as_deref()
        .unwrap_or(recording_mime_type(recording.format));

    let mut response = Response::new(axum::body::Body::from(bytes.clone()));
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_str(mime_type).map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("failed to encode content type header: {error}"),
                }),
            )
        })?,
    );
    response.headers_mut().insert(
        CONTENT_LENGTH,
        HeaderValue::from_str(&bytes.len().to_string()).map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("failed to encode content length header: {error}"),
                }),
            )
        })?,
    );
    response.headers_mut().insert(
        CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{filename}\"")).map_err(
            |error| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("failed to encode content disposition header: {error}"),
                    }),
                )
            },
        )?,
    );
    Ok(response)
}
