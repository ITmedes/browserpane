use super::super::*;
use super::headers::{
    authorize_api_request, automation_access_claims_match_session, extract_automation_access_token,
    extract_bearer_token, validate_any_automation_access_request,
};

pub(in crate::api) async fn authorize_visible_automation_task_request_with_automation_access(
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

pub(in crate::api) async fn authorize_visible_workflow_run_request_with_automation_access(
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
