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
        Json(
            session_resource(&state, &stored, None)
                .await
                .map_err(map_session_store_error)?,
        ),
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
        .map_err(map_session_store_error)?;
    let mut resources = Vec::with_capacity(sessions.len());
    for session in sessions {
        resources.push(
            session_resource(&state, &session, None)
                .await
                .map_err(map_session_store_error)?,
        );
    }

    Ok(Json(SessionListResponse {
        sessions: resources,
    }))
}

pub(super) async fn get_session(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionResource>, (StatusCode, Json<ErrorResponse>)> {
    let stored = authorize_visible_session_request(&headers, &state, session_id).await?;

    Ok(Json(
        session_resource(&state, &stored, None)
            .await
            .map_err(map_session_store_error)?,
    ))
}
