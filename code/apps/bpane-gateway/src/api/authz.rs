use super::*;

pub(super) async fn load_session_owner_principal(
    state: &ApiState,
    session_id: Uuid,
) -> Result<AuthenticatedPrincipal, (StatusCode, Json<ErrorResponse>)> {
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
    Ok(AuthenticatedPrincipal {
        subject: session.owner.subject,
        issuer: session.owner.issuer,
        display_name: session.owner.display_name,
        client_id: None,
    })
}

pub(super) async fn authorize_visible_automation_task_request_with_automation_access(
    headers: &HeaderMap,
    state: &ApiState,
    task_id: Uuid,
) -> Result<crate::automation_tasks::StoredAutomationTask, (StatusCode, Json<ErrorResponse>)> {
    if extract_bearer_token(headers).is_some() {
        match authorize_api_request(headers, &state.auth_validator).await {
            Ok(principal) => {
                if let Some(task) = state
                    .session_store
                    .get_automation_task_for_owner(&principal, task_id)
                    .await
                    .map_err(map_session_store_error)?
                {
                    return Ok(task);
                }
                if extract_automation_access_token(headers).is_none() {
                    return Err((
                        StatusCode::NOT_FOUND,
                        Json(ErrorResponse {
                            error: format!("automation task {task_id} not found"),
                        }),
                    ));
                }
            }
            Err(error) if extract_automation_access_token(headers).is_none() => {
                return Err((StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })));
            }
            Err(_) => {}
        }
    }

    let claims = validate_any_automation_access_request(headers, state)?;
    let task = state
        .session_store
        .get_automation_task_by_id(task_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("automation task {task_id} not found"),
                }),
            )
        })?;
    let session = state
        .session_store
        .get_session_by_id(task.session_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("session {} not found", task.session_id),
                }),
            )
        })?;
    if !automation_access_claims_match_session(&claims, &session) {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "automation access token is no longer valid for this session".to_string(),
            }),
        ));
    }
    Ok(task)
}

pub(super) async fn authorize_visible_workflow_run_request_with_automation_access(
    headers: &HeaderMap,
    state: &ApiState,
    run_id: Uuid,
) -> Result<crate::workflow::StoredWorkflowRun, (StatusCode, Json<ErrorResponse>)> {
    if extract_bearer_token(headers).is_some() {
        match authorize_api_request(headers, &state.auth_validator).await {
            Ok(principal) => {
                if let Some(run) = state
                    .session_store
                    .get_workflow_run_for_owner(&principal, run_id)
                    .await
                    .map_err(map_session_store_error)?
                {
                    return Ok(run);
                }
                if extract_automation_access_token(headers).is_none() {
                    return Err((
                        StatusCode::NOT_FOUND,
                        Json(ErrorResponse {
                            error: format!("workflow run {run_id} not found"),
                        }),
                    ));
                }
            }
            Err(error) if extract_automation_access_token(headers).is_none() => {
                return Err((StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })));
            }
            Err(_) => {}
        }
    }

    let claims = validate_any_automation_access_request(headers, state)?;
    let run = state
        .session_store
        .get_workflow_run_by_id(run_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("workflow run {run_id} not found"),
                }),
            )
        })?;
    let session = state
        .session_store
        .get_session_by_id(run.session_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("session {} not found", run.session_id),
                }),
            )
        })?;
    if !automation_access_claims_match_session(&claims, &session) {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "automation access token is no longer valid for this session".to_string(),
            }),
        ));
    }
    Ok(run)
}

pub(super) async fn authorize_api_request(
    headers: &HeaderMap,
    auth_validator: &AuthValidator,
) -> Result<AuthenticatedPrincipal, String> {
    let token = extract_bearer_token(headers).ok_or_else(|| "missing bearer token".to_string())?;
    auth_validator
        .authenticate(token)
        .await
        .map_err(|error| format!("invalid bearer token: {error}"))
}

pub(super) fn extract_bearer_token(headers: &HeaderMap) -> Option<&str> {
    let value = headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    value.strip_prefix("Bearer ")
}

pub(super) fn extract_automation_access_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(AUTOMATION_ACCESS_TOKEN_HEADER)?
        .to_str()
        .ok()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub(super) async fn authorize_runtime_session_request(
    headers: &HeaderMap,
    state: &ApiState,
    session_id: Uuid,
) -> Result<StoredSession, (StatusCode, Json<ErrorResponse>)> {
    let session = authorize_visible_session_request(headers, state, session_id).await?;

    ensure_runtime_candidate_session(session, session_id)
}

pub(super) async fn authorize_runtime_session_request_with_automation_access(
    headers: &HeaderMap,
    state: &ApiState,
    session_id: Uuid,
) -> Result<StoredSession, (StatusCode, Json<ErrorResponse>)> {
    let session =
        authorize_visible_session_request_with_automation_access(headers, state, session_id)
            .await?;

    ensure_runtime_candidate_session(session, session_id)
}

fn ensure_runtime_candidate_session(
    session: StoredSession,
    session_id: Uuid,
) -> Result<StoredSession, (StatusCode, Json<ErrorResponse>)> {
    if !matches!(
        session.state,
        SessionLifecycleState::Pending
            | SessionLifecycleState::Starting
            | SessionLifecycleState::Ready
            | SessionLifecycleState::Active
            | SessionLifecycleState::Idle
    ) {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!(
                    "session {session_id} is not attached to a runtime-compatible state"
                ),
            }),
        ));
    }

    Ok(session)
}

pub(super) async fn prepare_runtime_access_session(
    state: &ApiState,
    principal: &AuthenticatedPrincipal,
    session_id: Uuid,
) -> Result<StoredSession, (StatusCode, Json<ErrorResponse>)> {
    let stored = state
        .session_store
        .get_session_for_principal(principal, session_id)
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
    let was_stopped = stored.state == SessionLifecycleState::Stopped;

    let connectable = if was_stopped {
        let prepared = state
            .session_store
            .prepare_session_for_connect(session_id)
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
        schedule_idle_session_stop(
            session_id,
            state.idle_stop_timeout,
            state.registry.clone(),
            state.session_store.clone(),
            state.session_manager.clone(),
            state.recording_lifecycle.clone(),
        );
        prepared
    } else {
        stored
    };

    if let Err(error) = state
        .recording_lifecycle
        .ensure_auto_recording(&connectable)
        .await
    {
        if was_stopped {
            let _ = state.session_store.stop_session_if_idle(session_id).await;
            state.session_manager.release(session_id).await;
            state.registry.remove_session(session_id).await;
        }
        return Err(map_recording_lifecycle_error(error));
    }

    if !connectable.state.is_runtime_candidate() {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!(
                    "session {session_id} is not connectable in state {}",
                    connectable.state.as_str()
                ),
            }),
        ));
    }

    Ok(connectable)
}

pub(super) async fn authorize_visible_session_request_with_automation_access(
    headers: &HeaderMap,
    state: &ApiState,
    session_id: Uuid,
) -> Result<StoredSession, (StatusCode, Json<ErrorResponse>)> {
    if extract_bearer_token(headers).is_some() {
        match authorize_visible_session_request(headers, state, session_id).await {
            Ok(session) => return Ok(session),
            Err(error) if extract_automation_access_token(headers).is_none() => return Err(error),
            Err(_) => {}
        }
    }

    let claims = validate_automation_access_request(headers, state, session_id)?;
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
    if !automation_access_claims_match_session(&claims, &session) {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "automation access token is no longer valid for this session".to_string(),
            }),
        ));
    }

    Ok(session)
}

pub(super) async fn authorize_runtime_access_principal_with_automation_access(
    headers: &HeaderMap,
    state: &ApiState,
    session_id: Uuid,
) -> Result<AuthenticatedPrincipal, (StatusCode, Json<ErrorResponse>)> {
    match authorize_api_request(headers, &state.auth_validator).await {
        Ok(principal) => Ok(principal),
        Err(error) if extract_automation_access_token(headers).is_none() => {
            Err((StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))
        }
        Err(_) => {
            let claims = validate_automation_access_request(headers, state, session_id)?;
            Ok(AuthenticatedPrincipal {
                subject: claims.subject,
                issuer: claims.issuer,
                display_name: None,
                client_id: claims.client_id,
            })
        }
    }
}

pub(super) fn validate_automation_access_request(
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

fn validate_any_automation_access_request(
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

pub(super) fn automation_access_claims_match_session(
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

pub(super) async fn authorize_visible_session_request(
    headers: &HeaderMap,
    state: &ApiState,
    session_id: Uuid,
) -> Result<StoredSession, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let session = state
        .session_store
        .get_session_for_principal(&principal, session_id)
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

    Ok(session)
}
