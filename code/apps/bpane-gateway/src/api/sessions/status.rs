use chrono::Utc;

use super::super::*;

pub(super) async fn get_session_status(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionStatus>, (StatusCode, Json<ErrorResponse>)> {
    let session =
        authorize_runtime_session_request_with_automation_access(&headers, &state, session_id)
            .await?;
    let hub = state
        .registry
        .ensure_hub_for_session(
            session_id,
            &resolve_runtime(&state, session_id).await?.agent_socket_path,
        )
        .await
        .map_err(|error| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: format!("failed to connect to host agent: {error}"),
                }),
            )
        })?;
    let snapshot = hub.telemetry_snapshot().await;
    let recordings = state
        .session_store
        .list_recordings_for_session(session_id)
        .await
        .map_err(map_session_store_error)?;
    let latest_recording = latest_recording(&recordings);
    let playback = prepare_session_recording_playback(session_id, &recordings, Utc::now());

    Ok(Json(session_status_from_snapshot(
        snapshot,
        &session.recording,
        latest_recording,
        playback.resource,
    )))
}

pub(super) async fn session_status(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionStatus>, (StatusCode, Json<ErrorResponse>)> {
    authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    ensure_legacy_runtime_routes_supported(&state)?;
    let Some(session_id) = legacy_runtime_session_id(&state).await else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "no runtime-backed session is available".to_string(),
            }),
        ));
    };
    let runtime = resolve_runtime_compat(&state, session_id)
        .await
        .map_err(map_runtime_compat_status)?;
    let session = state
        .session_store
        .get_session_by_id(session_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("session {session_id} not found"),
                }),
            )
        })?;
    let hub = state
        .registry
        .ensure_hub_for_session(session_id, &runtime.agent_socket_path)
        .await
        .map_err(|error| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: format!("failed to connect to host agent: {error}"),
                }),
            )
        })?;
    let snapshot = hub.telemetry_snapshot().await;
    let recordings = state
        .session_store
        .list_recordings_for_session(session_id)
        .await
        .map_err(map_session_store_error)?;
    let latest_recording = latest_recording(&recordings);
    let playback = prepare_session_recording_playback(session_id, &recordings, Utc::now());

    Ok(Json(session_status_from_snapshot(
        snapshot,
        &session.recording,
        latest_recording,
        playback.resource,
    )))
}
