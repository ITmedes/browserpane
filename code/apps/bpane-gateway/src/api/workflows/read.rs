use super::super::*;

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
