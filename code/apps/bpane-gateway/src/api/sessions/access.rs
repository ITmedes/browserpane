use super::super::*;

pub(super) async fn issue_session_access_token(
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
    let resource = session_resource(&state, &connectable, None).await;

    Ok(Json(SessionAccessTokenResponse {
        session_id,
        token_type: "session_connect_ticket".to_string(),
        token: issued.token,
        expires_at: issued.expires_at,
        connect: resource.connect,
    }))
}

pub(super) async fn issue_session_automation_access(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionAutomationAccessResponse>, (StatusCode, Json<ErrorResponse>)> {
    let principal =
        authorize_runtime_access_principal_with_automation_access(&headers, &state, session_id)
            .await?;
    let connectable = prepare_runtime_access_session(&state, &principal, session_id).await?;
    resolve_runtime(&state, session_id).await?;
    let resource = session_resource(&state, &connectable, None).await;
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
