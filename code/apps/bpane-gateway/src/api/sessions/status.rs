use chrono::Utc;

use super::super::*;

pub(super) async fn get_session_status(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionStatus>, (StatusCode, Json<ErrorResponse>)> {
    let session =
        authorize_visible_session_request_with_automation_access(&headers, &state, session_id)
            .await?;
    let snapshot = state
        .registry
        .telemetry_snapshot_if_live(session_id)
        .await
        .unwrap_or_else(|| state.registry.empty_telemetry_snapshot());
    let summary = session_status_summary(&state, &session)
        .await
        .map_err(map_session_store_error)?;
    let recordings = state
        .session_store
        .list_recordings_for_session(session_id)
        .await
        .map_err(map_session_store_error)?;
    let latest_recording = latest_recording(&recordings);
    let playback = prepare_session_recording_playback(session_id, &recordings, Utc::now());

    Ok(Json(session_status_from_snapshot(
        session.state,
        summary,
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
    let snapshot = state
        .registry
        .telemetry_snapshot_if_live(session_id)
        .await
        .unwrap_or_else(|| state.registry.empty_telemetry_snapshot());
    let summary = session_status_summary(&state, &session)
        .await
        .map_err(map_session_store_error)?;
    let recordings = state
        .session_store
        .list_recordings_for_session(session_id)
        .await
        .map_err(map_session_store_error)?;
    let latest_recording = latest_recording(&recordings);
    let playback = prepare_session_recording_playback(session_id, &recordings, Utc::now());

    Ok(Json(session_status_from_snapshot(
        session.state,
        summary,
        snapshot,
        &session.recording,
        latest_recording,
        playback.resource,
    )))
}
