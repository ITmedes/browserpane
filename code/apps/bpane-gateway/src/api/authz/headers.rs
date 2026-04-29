use super::super::*;

pub(in crate::api) async fn authorize_api_request(
    headers: &HeaderMap,
    auth_validator: &AuthValidator,
) -> Result<AuthenticatedPrincipal, String> {
    let token = extract_bearer_token(headers).ok_or_else(|| "missing bearer token".to_string())?;
    auth_validator
        .authenticate(token)
        .await
        .map_err(|error| format!("invalid bearer token: {error}"))
}

pub(in crate::api) fn extract_bearer_token(headers: &HeaderMap) -> Option<&str> {
    let value = headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    value.strip_prefix("Bearer ")
}

pub(in crate::api) fn extract_automation_access_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(AUTOMATION_ACCESS_TOKEN_HEADER)?
        .to_str()
        .ok()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub(in crate::api) fn validate_automation_access_request(
    headers: &HeaderMap,
    state: &ApiState,
    session_id: Uuid,
) -> Result<SessionAutomationAccessTokenClaims, (StatusCode, Json<ErrorResponse>)> {
    let claims = validate_any_automation_access_request(headers, state)?;
    if claims.session_id != session_id {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "session automation access token does not match the requested session"
                    .to_string(),
            }),
        ));
    }
    Ok(claims)
}

pub(in crate::api) fn validate_any_automation_access_request(
    headers: &HeaderMap,
    state: &ApiState,
) -> Result<SessionAutomationAccessTokenClaims, (StatusCode, Json<ErrorResponse>)> {
    let token = extract_automation_access_token(headers).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "missing bearer token or session automation access token".to_string(),
            }),
        )
    })?;
    state
        .automation_access_token_manager
        .validate_token(token)
        .map_err(|error| {
            (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: format!("invalid session automation access token: {error}"),
                }),
            )
        })
}

pub(in crate::api) fn automation_access_claims_match_session(
    claims: &SessionAutomationAccessTokenClaims,
    session: &StoredSession,
) -> bool {
    if session.owner.subject == claims.subject && session.owner.issuer == claims.issuer {
        return true;
    }

    let Some(delegate) = &session.automation_delegate else {
        return false;
    };
    claims.issuer == delegate.issuer
        && claims.client_id.as_deref() == Some(delegate.client_id.as_str())
}
