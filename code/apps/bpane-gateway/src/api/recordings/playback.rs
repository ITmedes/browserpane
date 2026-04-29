use axum::response::Response;
use chrono::Utc;

use super::*;

pub(super) async fn get_session_recording_playback(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionRecordingPlaybackResource>, (StatusCode, Json<ErrorResponse>)> {
    let _session = authorize_visible_session_request(&headers, &state, session_id).await?;
    let playback = load_session_recording_playback(&state, session_id).await?;
    Ok(Json(playback.resource))
}

pub(super) async fn get_session_recording_playback_manifest(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionRecordingPlaybackManifest>, (StatusCode, Json<ErrorResponse>)> {
    let _session = authorize_visible_session_request(&headers, &state, session_id).await?;
    state
        .recording_observability
        .record_playback_manifest_request();
    let playback = load_session_recording_playback(&state, session_id).await?;
    Ok(Json(playback.manifest))
}

pub(super) async fn get_session_recording_playback_export(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let _session = authorize_visible_session_request(&headers, &state, session_id).await?;
    state
        .recording_observability
        .record_playback_export_request();
    let playback = load_session_recording_playback(&state, session_id).await?;
    let bytes = playback
        .export_bundle(&state.recording_artifact_store)
        .await
        .map_err(|error| {
            state
                .recording_observability
                .record_playback_export_failure();
            map_recording_playback_error(error)
        })?;
    state
        .recording_observability
        .record_playback_export_success(bytes.len() as u64, Utc::now())
        .await;

    let filename = format!("browserpane-{session_id}-recording-playback.zip");
    let mut response = Response::new(axum::body::Body::from(bytes.clone()));
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/zip"));
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
