use super::super::*;

pub(super) async fn update_session_recording_policy(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(recording): Json<SessionRecordingPolicy>,
) -> Result<Json<SessionResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let previous = state
        .session_store
        .get_session_for_owner(&principal, session_id)
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
    let updated = state
        .session_store
        .update_session_recording_policy_for_owner(&principal, session_id, recording)
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

    if updated.recording.mode == SessionRecordingMode::Always
        && updated.state.is_runtime_candidate()
    {
        if let Err(error) = state
            .recording_lifecycle
            .ensure_auto_recording(&updated)
            .await
        {
            let _ = state
                .session_store
                .update_session_recording_policy_for_owner(
                    &principal,
                    session_id,
                    previous.recording,
                )
                .await;
            return Err(map_recording_lifecycle_error(error));
        }
    } else if previous.recording.mode == SessionRecordingMode::Always
        && updated.recording.mode != SessionRecordingMode::Always
    {
        state
            .recording_lifecycle
            .request_stop_and_wait(session_id, SessionRecordingTerminationReason::ManualStop)
            .await
            .map_err(map_recording_lifecycle_error)?;
    }

    let fresh = state
        .session_store
        .get_session_for_owner(&principal, session_id)
        .await
        .map_err(map_session_store_error)?
        .unwrap_or(updated);

    Ok(Json(
        session_resource(&state, &fresh, None)
            .await
            .map_err(map_session_store_error)?,
    ))
}
