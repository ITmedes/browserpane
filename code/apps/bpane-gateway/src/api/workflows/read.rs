use super::super::*;

pub(super) async fn list_workflow_runs(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<WorkflowRunListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let _ = state.workflow_lifecycle.reconcile_waiting_runs().await;
    let mut runs = state
        .session_store
        .list_workflow_runs_for_owner(&principal)
        .await
        .map_err(map_session_store_error)?;
    runs.sort_by(|left, right| {
        right
            .created_at
            .cmp(&left.created_at)
            .then_with(|| right.id.cmp(&left.id))
    });
    let mut resources = Vec::with_capacity(runs.len());
    for run in runs {
        resources.push(build_workflow_run_resource(&state, &run).await?);
    }
    Ok(Json(WorkflowRunListResponse { runs: resources }))
}

pub(super) async fn get_workflow_run(
    headers: HeaderMap,
    Path(run_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<WorkflowRunResource>, (StatusCode, Json<ErrorResponse>)> {
    let mut run =
        authorize_visible_workflow_run_request_with_automation_access(&headers, &state, run_id)
            .await?;
    if !run.state.is_terminal() {
        let _ = state.workflow_lifecycle.reconcile_waiting_runs().await;
        if let Some(updated) = state
            .session_store
            .get_workflow_run_by_id(run.id)
            .await
            .map_err(map_session_store_error)?
        {
            run = updated;
        }
    }
    Ok(Json(build_workflow_run_resource(&state, &run).await?))
}
