use axum::routing::{get, post};

use super::*;

pub(super) fn automation_task_routes() -> Router<Arc<ApiState>> {
    Router::new()
        .route(
            "/api/v1/automation-tasks",
            post(create_automation_task).get(list_automation_tasks),
        )
        .route(
            "/api/v1/automation-tasks/{task_id}",
            get(get_automation_task),
        )
        .route(
            "/api/v1/automation-tasks/{task_id}/state",
            post(transition_automation_task_state),
        )
        .route(
            "/api/v1/automation-tasks/{task_id}/cancel",
            post(cancel_automation_task),
        )
        .route(
            "/api/v1/automation-tasks/{task_id}/events",
            get(get_automation_task_events),
        )
        .route(
            "/api/v1/automation-tasks/{task_id}/logs",
            get(get_automation_task_logs).post(append_automation_task_log),
        )
}

async fn list_automation_tasks(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<AutomationTaskListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let tasks = state
        .session_store
        .list_automation_tasks_for_owner(&principal)
        .await
        .map_err(map_session_store_error)?
        .into_iter()
        .map(|task| task.to_resource())
        .collect();
    Ok(Json(AutomationTaskListResponse { tasks }))
}

async fn create_automation_task(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CreateAutomationTaskRequest>,
) -> Result<(StatusCode, Json<AutomationTaskResource>), (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let (session, session_source) =
        resolve_task_session_binding(&state, &principal, Some(request.session), None, None).await?;

    let task = state
        .session_store
        .create_automation_task(
            &principal,
            PersistAutomationTaskRequest {
                display_name: request.display_name,
                executor: request.executor,
                session_id: session.id,
                session_source,
                input: request.input,
                labels: request.labels,
            },
        )
        .await
        .map_err(map_session_store_error)?;

    Ok((StatusCode::CREATED, Json(task.to_resource())))
}

async fn get_automation_task(
    headers: HeaderMap,
    Path(task_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<AutomationTaskResource>, (StatusCode, Json<ErrorResponse>)> {
    let task =
        authorize_visible_automation_task_request_with_automation_access(&headers, &state, task_id)
            .await?;
    Ok(Json(task.to_resource()))
}

async fn cancel_automation_task(
    headers: HeaderMap,
    Path(task_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<AutomationTaskResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let task = state
        .session_store
        .cancel_automation_task_for_owner(&principal, task_id)
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
    Ok(Json(task.to_resource()))
}

async fn transition_automation_task_state(
    headers: HeaderMap,
    Path(task_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<TransitionAutomationTaskRequest>,
) -> Result<Json<AutomationTaskResource>, (StatusCode, Json<ErrorResponse>)> {
    let _task =
        authorize_visible_automation_task_request_with_automation_access(&headers, &state, task_id)
            .await?;
    let message = request.message.unwrap_or_else(|| match request.state {
        AutomationTaskState::Pending => "automation task returned to pending state".to_string(),
        AutomationTaskState::Queued => {
            "automation task queued until worker capacity is available".to_string()
        }
        AutomationTaskState::Starting => "automation task started".to_string(),
        AutomationTaskState::Running => "automation task entered running state".to_string(),
        AutomationTaskState::AwaitingInput => "automation task is awaiting input".to_string(),
        AutomationTaskState::Succeeded => "automation task completed successfully".to_string(),
        AutomationTaskState::Failed => "automation task failed".to_string(),
        AutomationTaskState::Cancelled => "automation task cancelled".to_string(),
        AutomationTaskState::TimedOut => "automation task timed out".to_string(),
    });
    let event_type = match request.state {
        AutomationTaskState::Pending => "automation_task.pending",
        AutomationTaskState::Queued => "automation_task.queued",
        AutomationTaskState::Starting => "automation_task.starting",
        AutomationTaskState::Running => "automation_task.running",
        AutomationTaskState::AwaitingInput => "automation_task.awaiting_input",
        AutomationTaskState::Succeeded => "automation_task.succeeded",
        AutomationTaskState::Failed => "automation_task.failed",
        AutomationTaskState::Cancelled => "automation_task.cancelled",
        AutomationTaskState::TimedOut => "automation_task.timed_out",
    };
    let task = state
        .session_store
        .transition_automation_task(
            task_id,
            AutomationTaskTransitionRequest {
                state: request.state,
                output: request.output,
                error: request.error,
                artifact_refs: request.artifact_refs,
                event_type: event_type.to_string(),
                event_message: message,
                event_data: request.data,
            },
        )
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
    Ok(Json(task.to_resource()))
}

async fn append_automation_task_log(
    headers: HeaderMap,
    Path(task_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<AppendAutomationTaskLogRequest>,
) -> Result<
    Json<crate::automation_tasks::AutomationTaskLogLineResource>,
    (StatusCode, Json<ErrorResponse>),
> {
    let _task =
        authorize_visible_automation_task_request_with_automation_access(&headers, &state, task_id)
            .await?;
    let log = state
        .session_store
        .append_automation_task_log(task_id, request.stream, request.message)
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
    Ok(Json(log.to_resource()))
}

async fn get_automation_task_events(
    headers: HeaderMap,
    Path(task_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<AutomationTaskEventListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let task =
        authorize_visible_automation_task_request_with_automation_access(&headers, &state, task_id)
            .await?;
    let principal = load_session_owner_principal(&state, task.session_id).await?;
    let events = state
        .session_store
        .list_automation_task_events_for_owner(&principal, task_id)
        .await
        .map_err(map_session_store_error)?
        .into_iter()
        .map(|event| event.to_resource())
        .collect();
    Ok(Json(AutomationTaskEventListResponse { events }))
}

async fn get_automation_task_logs(
    headers: HeaderMap,
    Path(task_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<AutomationTaskLogListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let task =
        authorize_visible_automation_task_request_with_automation_access(&headers, &state, task_id)
            .await?;
    let principal = load_session_owner_principal(&state, task.session_id).await?;
    let logs = state
        .session_store
        .list_automation_task_logs_for_owner(&principal, task_id)
        .await
        .map_err(map_session_store_error)?
        .into_iter()
        .map(|log| log.to_resource())
        .collect();
    Ok(Json(AutomationTaskLogListResponse { logs }))
}
