use axum::routing::{delete, get, post};

use super::*;

pub(super) fn session_routes() -> Router<Arc<ApiState>> {
    Router::new()
        .route("/api/v1/sessions", post(create_session).get(list_sessions))
        .route(
            "/api/v1/sessions/{session_id}",
            get(get_session).delete(delete_session),
        )
}

pub(super) fn session_operation_routes() -> Router<Arc<ApiState>> {
    Router::new()
        .route(
            "/api/v1/sessions/{session_id}/access-tokens",
            post(issue_session_access_token),
        )
        .route(
            "/api/v1/sessions/{session_id}/automation-access",
            post(issue_session_automation_access),
        )
        .route(
            "/api/v1/sessions/{session_id}/automation-owner",
            post(set_automation_owner).delete(clear_automation_owner),
        )
        .route(
            "/api/v1/sessions/{session_id}/status",
            get(get_session_status),
        )
        .route(
            "/api/v1/sessions/{session_id}/mcp-owner",
            post(set_session_mcp_owner).delete(clear_session_mcp_owner),
        )
}

pub(super) fn legacy_session_routes() -> Router<Arc<ApiState>> {
    Router::new()
        .route("/api/session/status", get(session_status))
        .route("/api/session/mcp-owner", post(set_mcp_owner))
        .route("/api/session/mcp-owner", delete(clear_mcp_owner))
}

async fn create_session(
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

async fn list_sessions(
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

async fn get_session(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionResource>, (StatusCode, Json<ErrorResponse>)> {
    let stored = authorize_visible_session_request(&headers, &state, session_id).await?;

    Ok(Json(session_resource(&state, &stored, None)))
}

async fn set_automation_owner(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<SetAutomationDelegateRequest>,
) -> Result<Json<SessionResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let stored = state
        .session_store
        .set_automation_delegate_for_owner(&principal, session_id, request)
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

    Ok(Json(session_resource(&state, &stored, None)))
}

async fn clear_automation_owner(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let stored = state
        .session_store
        .clear_automation_delegate_for_owner(&principal, session_id)
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

    Ok(Json(session_resource(&state, &stored, None)))
}

async fn issue_session_access_token(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionAccessTokenResponse>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let connectable = prepare_runtime_access_session(&state, &principal, session_id).await?;

    let issued = state
        .connect_ticket_manager
        .issue_ticket(session_id, &principal)
        .map_err(|error| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: format!("failed to issue session connect ticket: {error}"),
                }),
            )
        })?;
    let resource = session_resource(&state, &connectable, None);

    Ok(Json(SessionAccessTokenResponse {
        session_id,
        token_type: "session_connect_ticket".to_string(),
        token: issued.token,
        expires_at: issued.expires_at,
        connect: resource.connect,
    }))
}

async fn issue_session_automation_access(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionAutomationAccessResponse>, (StatusCode, Json<ErrorResponse>)> {
    let principal =
        authorize_runtime_access_principal_with_automation_access(&headers, &state, session_id)
            .await?;
    let connectable = prepare_runtime_access_session(&state, &principal, session_id).await?;
    resolve_runtime(&state, session_id).await?;
    let resource = session_resource(&state, &connectable, None);
    let endpoint_url = resource.runtime.cdp_endpoint.ok_or_else(|| {
        (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!(
                    "session {session_id} does not expose an automation endpoint for the current runtime"
                ),
            }),
        )
    })?;
    let issued = state
        .automation_access_token_manager
        .issue_token(session_id, &principal)
        .map_err(|error| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: format!("failed to issue session automation access token: {error}"),
                }),
            )
        })?;

    Ok(Json(SessionAutomationAccessResponse {
        session_id,
        token_type: "session_automation_access_token".to_string(),
        token: issued.token,
        expires_at: issued.expires_at,
        automation: SessionAutomationAccessInfo {
            endpoint_url,
            protocol: "chrome_devtools_protocol".to_string(),
            auth_type: "session_automation_access_token".to_string(),
            auth_header: AUTOMATION_ACCESS_TOKEN_HEADER.to_string(),
            status_path: format!("/api/v1/sessions/{session_id}/status"),
            mcp_owner_path: format!("/api/v1/sessions/{session_id}/mcp-owner"),
            compatibility_mode: resource.connect.compatibility_mode,
        },
    }))
}

async fn get_session_status(
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

async fn delete_session(
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

async fn session_status(
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

async fn set_mcp_owner(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
    Json(req): Json<McpOwnerRequest>,
) -> Result<Json<OkResponse>, (StatusCode, Json<ErrorResponse>)> {
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
    let runtime = resolve_runtime(&state, session_id).await?;
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

    hub.set_mcp_owner(req.width, req.height).await;
    state.session_manager.mark_session_active(session_id).await;
    let _ = state.session_store.mark_session_active(session_id).await;

    Ok(Json(OkResponse { ok: true }))
}

async fn set_session_mcp_owner(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(req): Json<McpOwnerRequest>,
) -> Result<Json<OkResponse>, (StatusCode, Json<ErrorResponse>)> {
    let _session =
        authorize_runtime_session_request_with_automation_access(&headers, &state, session_id)
            .await?;
    let runtime = resolve_runtime(&state, session_id).await?;
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

    hub.set_mcp_owner(req.width, req.height).await;
    state.session_manager.mark_session_active(session_id).await;
    let _ = state.session_store.mark_session_active(session_id).await;

    Ok(Json(OkResponse { ok: true }))
}

async fn clear_mcp_owner(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<OkResponse>, StatusCode> {
    authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    if !state
        .session_manager
        .profile()
        .supports_legacy_global_routes
    {
        return Err(StatusCode::CONFLICT);
    }
    let Some(session_id) = legacy_runtime_session_id(&state).await else {
        return Err(StatusCode::NOT_FOUND);
    };
    let runtime = resolve_runtime_compat(&state, session_id).await?;
    let hub = state
        .registry
        .ensure_hub_for_session(session_id, &runtime.agent_socket_path)
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    hub.clear_mcp_owner().await;
    let snapshot = hub.telemetry_snapshot().await;
    if snapshot.browser_clients == 0 && snapshot.viewer_clients == 0 && !snapshot.mcp_owner {
        let _ = state.session_store.mark_session_idle(session_id).await;
        state.session_manager.mark_session_idle(session_id).await;
        schedule_idle_session_stop(
            session_id,
            state.idle_stop_timeout,
            state.registry.clone(),
            state.session_store.clone(),
            state.session_manager.clone(),
            state.recording_lifecycle.clone(),
        );
    }

    Ok(Json(OkResponse { ok: true }))
}

async fn clear_session_mcp_owner(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<OkResponse>, (StatusCode, Json<ErrorResponse>)> {
    let _session =
        authorize_runtime_session_request_with_automation_access(&headers, &state, session_id)
            .await?;
    let runtime = resolve_runtime(&state, session_id).await?;
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

    hub.clear_mcp_owner().await;
    let snapshot = hub.telemetry_snapshot().await;
    if snapshot.browser_clients == 0 && snapshot.viewer_clients == 0 && !snapshot.mcp_owner {
        let _ = state.session_store.mark_session_idle(session_id).await;
        state.session_manager.mark_session_idle(session_id).await;
        schedule_idle_session_stop(
            session_id,
            state.idle_stop_timeout,
            state.registry.clone(),
            state.session_store.clone(),
            state.session_manager.clone(),
            state.recording_lifecycle.clone(),
        );
    }

    Ok(Json(OkResponse { ok: true }))
}
