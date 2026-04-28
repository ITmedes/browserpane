use axum::routing::{get, post};

use super::*;

pub(super) fn workflow_event_subscription_routes() -> Router<Arc<ApiState>> {
    Router::new()
        .route(
            "/api/v1/workflow-event-subscriptions",
            post(create_workflow_event_subscription).get(list_workflow_event_subscriptions),
        )
        .route(
            "/api/v1/workflow-event-subscriptions/{subscription_id}",
            get(get_workflow_event_subscription).delete(delete_workflow_event_subscription),
        )
        .route(
            "/api/v1/workflow-event-subscriptions/{subscription_id}/deliveries",
            get(list_workflow_event_deliveries),
        )
}

pub(super) fn workflow_run_event_routes() -> Router<Arc<ApiState>> {
    Router::new()
        .route(
            "/api/v1/workflow-runs/{run_id}/state",
            post(transition_workflow_run_state),
        )
        .route(
            "/api/v1/workflow-runs/{run_id}/events",
            get(get_workflow_run_events),
        )
        .route(
            "/api/v1/workflow-runs/{run_id}/logs",
            get(get_workflow_run_logs).post(append_workflow_run_log),
        )
}

async fn create_workflow_event_subscription(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CreateWorkflowEventSubscriptionRequest>,
) -> Result<(StatusCode, Json<WorkflowEventSubscriptionResource>), (StatusCode, Json<ErrorResponse>)>
{
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let subscription = state
        .session_store
        .create_workflow_event_subscription(
            &principal,
            PersistWorkflowEventSubscriptionRequest {
                name: request.name,
                target_url: request.target_url,
                event_types: request.event_types,
                signing_secret: request.signing_secret,
            },
        )
        .await
        .map_err(map_session_store_error)?;
    Ok((StatusCode::CREATED, Json(subscription.to_resource())))
}

async fn list_workflow_event_subscriptions(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<WorkflowEventSubscriptionListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let subscriptions = state
        .session_store
        .list_workflow_event_subscriptions_for_owner(&principal)
        .await
        .map_err(map_session_store_error)?
        .into_iter()
        .map(|subscription| subscription.to_resource())
        .collect();
    Ok(Json(WorkflowEventSubscriptionListResponse {
        subscriptions,
    }))
}

async fn get_workflow_event_subscription(
    headers: HeaderMap,
    Path(subscription_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<WorkflowEventSubscriptionResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let subscription = state
        .session_store
        .get_workflow_event_subscription_for_owner(&principal, subscription_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("workflow event subscription {subscription_id} not found"),
                }),
            )
        })?;
    Ok(Json(subscription.to_resource()))
}

async fn delete_workflow_event_subscription(
    headers: HeaderMap,
    Path(subscription_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<WorkflowEventSubscriptionResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let subscription = state
        .session_store
        .delete_workflow_event_subscription_for_owner(&principal, subscription_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("workflow event subscription {subscription_id} not found"),
                }),
            )
        })?;
    Ok(Json(subscription.to_resource()))
}

async fn list_workflow_event_deliveries(
    headers: HeaderMap,
    Path(subscription_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<WorkflowEventDeliveryListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    if state
        .session_store
        .get_workflow_event_subscription_for_owner(&principal, subscription_id)
        .await
        .map_err(map_session_store_error)?
        .is_none()
    {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("workflow event subscription {subscription_id} not found"),
            }),
        ));
    }
    let deliveries = state
        .session_store
        .list_workflow_event_deliveries_for_owner(&principal, subscription_id)
        .await
        .map_err(map_session_store_error)?;
    let attempts = state
        .session_store
        .list_workflow_event_delivery_attempts_for_owner(&principal, subscription_id)
        .await
        .map_err(map_session_store_error)?;
    let attempts_by_delivery = group_attempts_by_delivery(attempts);
    let deliveries = deliveries
        .into_iter()
        .map(|delivery| {
            let attempts = attempts_by_delivery
                .get(&delivery.id)
                .cloned()
                .unwrap_or_default();
            delivery.to_resource(attempts)
        })
        .collect();
    Ok(Json(WorkflowEventDeliveryListResponse { deliveries }))
}

async fn transition_workflow_run_state(
    headers: HeaderMap,
    Path(run_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<TransitionWorkflowRunRequest>,
) -> Result<Json<WorkflowRunResource>, (StatusCode, Json<ErrorResponse>)> {
    let _run =
        authorize_visible_workflow_run_request_with_automation_access(&headers, &state, run_id)
            .await?;
    let run = state
        .session_store
        .transition_workflow_run(
            run_id,
            WorkflowRunTransitionRequest {
                state: request.state,
                output: request.output,
                error: request.error,
                artifact_refs: request.artifact_refs,
                message: request.message,
                data: request.data,
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
        .reconcile_runtime_hold(run.id)
        .await
    {
        warn!(run_id = %run.id, "failed to reconcile workflow runtime hold after run transition: {error}");
    }
    Ok(Json(build_workflow_run_resource(&state, &run).await?))
}

async fn append_workflow_run_log(
    headers: HeaderMap,
    Path(run_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<AppendWorkflowRunLogRequest>,
) -> Result<Json<WorkflowRunLogResource>, (StatusCode, Json<ErrorResponse>)> {
    let _run =
        authorize_visible_workflow_run_request_with_automation_access(&headers, &state, run_id)
            .await?;
    let log = state
        .session_store
        .append_workflow_run_log(
            run_id,
            PersistWorkflowRunLogRequest {
                stream: request.stream,
                message: request.message,
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
    Ok(Json(WorkflowRunLogResource::from_run(run_id, &log)))
}

async fn get_workflow_run_events(
    headers: HeaderMap,
    Path(run_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<WorkflowRunEventListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let run =
        authorize_visible_workflow_run_request_with_automation_access(&headers, &state, run_id)
            .await?;
    let events = workflow_run_event_resources(&state, &run).await?;
    Ok(Json(WorkflowRunEventListResponse { events }))
}

async fn get_workflow_run_logs(
    headers: HeaderMap,
    Path(run_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<WorkflowRunLogListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let run =
        authorize_visible_workflow_run_request_with_automation_access(&headers, &state, run_id)
            .await?;
    let principal = load_session_owner_principal(&state, run.session_id).await?;
    let mut logs = state
        .session_store
        .list_workflow_run_logs_for_owner(&principal, run_id)
        .await
        .map_err(map_session_store_error)?
        .into_iter()
        .map(|log| WorkflowRunLogResource::from_run(run.id, &log))
        .collect::<Vec<_>>();
    let task_logs = state
        .session_store
        .list_automation_task_logs_for_owner(&principal, run.automation_task_id)
        .await
        .map_err(map_session_store_error)?
        .into_iter()
        .map(|log| {
            WorkflowRunLogResource::from_automation_task(run.id, run.automation_task_id, &log)
        });
    logs.extend(task_logs);
    logs.sort_by(|left, right| {
        left.created_at
            .cmp(&right.created_at)
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(Json(WorkflowRunLogListResponse { logs }))
}
