use axum::routing::post;

use super::super::*;

pub(super) fn workflow_run_control_routes() -> Router<Arc<ApiState>> {
    Router::new()
        .route(
            "/api/v1/workflow-runs/{run_id}/cancel",
            post(cancel_workflow_run),
        )
        .route(
            "/api/v1/workflow-runs/{run_id}/submit-input",
            post(submit_workflow_run_input),
        )
        .route(
            "/api/v1/workflow-runs/{run_id}/resume",
            post(resume_workflow_run),
        )
        .route(
            "/api/v1/workflow-runs/{run_id}/reject",
            post(reject_workflow_run),
        )
}

async fn cancel_workflow_run(
    headers: HeaderMap,
    Path(run_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<WorkflowRunResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let run = state
        .session_store
        .get_workflow_run_for_owner(&principal, run_id)
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
    let _task = state
        .session_store
        .cancel_automation_task_for_owner(&principal, run.automation_task_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!(
                        "automation task {} for workflow run {run_id} not found",
                        run.automation_task_id
                    ),
                }),
            )
        })?;
    let _ = state
        .session_store
        .append_workflow_run_event_for_owner(
            &principal,
            run.id,
            PersistWorkflowRunEventRequest {
                event_type: "workflow_run.cancel_requested".to_string(),
                message: "workflow run cancellation requested".to_string(),
                data: Some(serde_json::json!({
                    "automation_task_id": run.automation_task_id,
                })),
            },
        )
        .await
        .map_err(map_session_store_error)?;
    if let Err(error) = state
        .workflow_lifecycle
        .reconcile_runtime_hold(run.id)
        .await
    {
        warn!(run_id = %run.id, "failed to reconcile workflow runtime hold after cancellation: {error}");
    }
    if let Err(error) = state.workflow_lifecycle.cancel_run(run.id).await {
        warn!(run_id = %run.id, "failed to stop workflow worker after cancel request: {error}");
    }
    let run = state
        .session_store
        .get_workflow_run_for_owner(&principal, run_id)
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
    Ok(Json(build_workflow_run_resource(&state, &run).await?))
}

async fn submit_workflow_run_input(
    headers: HeaderMap,
    Path(run_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<SubmitWorkflowRunInputRequest>,
) -> Result<Json<WorkflowRunResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let run = load_owner_workflow_run(&state, &principal, run_id).await?;
    ensure_run_awaiting_input(&run)?;
    let comment = trim_optional_comment(request.comment)?;
    let intervention = workflow_run_intervention_resource(&state, &run).await?;
    let resolution_data = workflow_run_intervention_resolution_data(
        intervention
            .pending_request
            .as_ref()
            .map(|request| request.request_id),
        "submit_input",
        Some(request.input),
        None,
        &principal,
        request.details,
    );

    state
        .session_store
        .transition_workflow_run(
            run_id,
            WorkflowRunTransitionRequest {
                state: WorkflowRunState::Running,
                output: run.output.clone(),
                error: None,
                artifact_refs: run.artifact_refs.clone(),
                message: Some(
                    comment
                        .clone()
                        .unwrap_or_else(|| "workflow run resumed with operator input".to_string()),
                ),
                data: Some(resolution_data.clone()),
            },
        )
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
    state
        .session_store
        .append_workflow_run_event_for_owner(
            &principal,
            run_id,
            PersistWorkflowRunEventRequest {
                event_type: "workflow_run.input_submitted".to_string(),
                message: comment
                    .unwrap_or_else(|| "operator submitted workflow run input".to_string()),
                data: Some(resolution_data),
            },
        )
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
    if let Err(error) = state
        .workflow_lifecycle
        .reconcile_runtime_hold(run_id)
        .await
    {
        warn!(run_id = %run_id, "failed to reconcile workflow runtime hold after operator input: {error}");
    }
    let run = load_owner_workflow_run(&state, &principal, run_id).await?;
    Ok(Json(build_workflow_run_resource(&state, &run).await?))
}

async fn resume_workflow_run(
    headers: HeaderMap,
    Path(run_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<ResumeWorkflowRunRequest>,
) -> Result<Json<WorkflowRunResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let run = load_owner_workflow_run(&state, &principal, run_id).await?;
    ensure_run_awaiting_input(&run)?;
    let comment = trim_optional_comment(request.comment)?;
    let intervention = workflow_run_intervention_resource(&state, &run).await?;
    let resolution_data = workflow_run_intervention_resolution_data(
        intervention
            .pending_request
            .as_ref()
            .map(|request| request.request_id),
        "resume",
        None,
        None,
        &principal,
        request.details,
    );

    state
        .session_store
        .transition_workflow_run(
            run_id,
            WorkflowRunTransitionRequest {
                state: WorkflowRunState::Running,
                output: run.output.clone(),
                error: None,
                artifact_refs: run.artifact_refs.clone(),
                message: Some(
                    comment
                        .clone()
                        .unwrap_or_else(|| "workflow run resumed by operator".to_string()),
                ),
                data: Some(resolution_data.clone()),
            },
        )
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
    state
        .session_store
        .append_workflow_run_event_for_owner(
            &principal,
            run_id,
            PersistWorkflowRunEventRequest {
                event_type: "workflow_run.resumed".to_string(),
                message: comment.unwrap_or_else(|| "operator resumed workflow run".to_string()),
                data: Some(resolution_data),
            },
        )
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
    if let Err(error) = state
        .workflow_lifecycle
        .reconcile_runtime_hold(run_id)
        .await
    {
        warn!(run_id = %run_id, "failed to reconcile workflow runtime hold after resume: {error}");
    }
    let run = load_owner_workflow_run(&state, &principal, run_id).await?;
    Ok(Json(build_workflow_run_resource(&state, &run).await?))
}

async fn reject_workflow_run(
    headers: HeaderMap,
    Path(run_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<RejectWorkflowRunRequest>,
) -> Result<Json<WorkflowRunResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let run = load_owner_workflow_run(&state, &principal, run_id).await?;
    ensure_run_awaiting_input(&run)?;
    let reason = request.reason.trim();
    if reason.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "reason must not be empty".to_string(),
            }),
        ));
    }
    let intervention = workflow_run_intervention_resource(&state, &run).await?;
    let resolution_data = workflow_run_intervention_resolution_data(
        intervention
            .pending_request
            .as_ref()
            .map(|request| request.request_id),
        "reject",
        None,
        Some(reason.to_string()),
        &principal,
        request.details,
    );

    state
        .session_store
        .transition_workflow_run(
            run_id,
            WorkflowRunTransitionRequest {
                state: WorkflowRunState::Failed,
                output: run.output.clone(),
                error: Some(reason.to_string()),
                artifact_refs: run.artifact_refs.clone(),
                message: Some("workflow run rejected by operator".to_string()),
                data: Some(resolution_data.clone()),
            },
        )
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
    state
        .session_store
        .append_workflow_run_event_for_owner(
            &principal,
            run_id,
            PersistWorkflowRunEventRequest {
                event_type: "workflow_run.rejected".to_string(),
                message: format!("operator rejected workflow run: {reason}"),
                data: Some(resolution_data),
            },
        )
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
    if let Err(error) = state
        .workflow_lifecycle
        .reconcile_runtime_hold(run_id)
        .await
    {
        warn!(run_id = %run_id, "failed to reconcile workflow runtime hold after rejection: {error}");
    }
    let run = load_owner_workflow_run(&state, &principal, run_id).await?;
    Ok(Json(build_workflow_run_resource(&state, &run).await?))
}
