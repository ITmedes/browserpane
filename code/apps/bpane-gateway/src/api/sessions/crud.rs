use super::super::*;

pub(super) async fn create_session(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CreateSessionRequest>,
) -> Result<(StatusCode, Json<SessionResource>), (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let owner_mode = resolve_owner_mode(&state, request.owner_mode)?;
    let stored = create_owned_session(&state, &principal, request, owner_mode, None).await?;

    Ok((
        StatusCode::CREATED,
        Json(session_resource(&state, &stored, None)),
    ))
}

pub(super) async fn list_sessions(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let sessions = state
        .session_store
        .list_sessions_for_owner(&principal)
        .await
        .map_err(map_session_store_error)?
        .into_iter()
        .map(|session| session_resource(&state, &session, None))
        .collect();

    Ok(Json(SessionListResponse { sessions }))
}

pub(super) async fn get_session(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionResource>, (StatusCode, Json<ErrorResponse>)> {
    let stored = authorize_visible_session_request(&headers, &state, session_id).await?;

    Ok(Json(session_resource(&state, &stored, None)))
}

pub(super) async fn delete_session(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;

    let stored = state
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

    if should_block_session_stop(
        stored.state,
        state
            .session_manager
            .profile()
            .supports_legacy_global_routes,
        runtime_is_currently_in_use(&state).await,
    ) {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: "cannot stop the legacy single-session runtime while it is in use"
                    .to_string(),
            }),
        ));
    }

    let stopped = state
        .session_store
        .stop_session_for_owner(&principal, session_id)
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

    if let Err(error) = state
        .recording_lifecycle
        .request_stop_and_wait(session_id, SessionRecordingTerminationReason::SessionStop)
        .await
    {
        info!(%session_id, "recording finalization before session stop returned: {error}");
    }
    state.session_manager.release(session_id).await;
    state.registry.remove_session(session_id).await;

    Ok(Json(session_resource(&state, &stopped, None)))
}
