use super::super::*;

pub(super) async fn set_automation_owner(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<SetAutomationDelegateRequest>,
) -> Result<Json<SessionResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let delegate_issuer = request
        .issuer
        .clone()
        .unwrap_or_else(|| principal.issuer.clone());
    let registered_delegate = state
        .session_store
        .get_service_principal_for_owner_by_external_identity(
            &principal,
            &delegate_issuer,
            &request.client_id,
        )
        .await
        .map_err(map_session_store_error)?;
    if registered_delegate
        .as_ref()
        .is_some_and(|delegate| delegate.state == ServicePrincipalState::Disabled)
    {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!(
                    "service principal {} from issuer {} is disabled",
                    request.client_id, delegate_issuer
                ),
            }),
        ));
    }
    let stored = state
        .session_store
        .set_automation_delegate_for_owner(&principal, session_id, request.clone())
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
    if registered_delegate.is_some() {
        let _ = state
            .session_store
            .mark_service_principal_delegated_for_owner(
                &principal,
                &delegate_issuer,
                &request.client_id,
            )
            .await
            .map_err(map_session_store_error)?;
    }

    Ok(Json(
        session_resource(&state, &stored, None)
            .await
            .map_err(map_session_store_error)?,
    ))
}

pub(super) async fn clear_automation_owner(
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

    Ok(Json(
        session_resource(&state, &stored, None)
            .await
            .map_err(map_session_store_error)?,
    ))
}
