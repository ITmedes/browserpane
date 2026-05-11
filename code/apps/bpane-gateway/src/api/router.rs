use axum::routing::get;

use super::*;

async fn get_workflow_operations(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<WorkflowObservabilitySnapshot>, (StatusCode, Json<ErrorResponse>)> {
    authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    Ok(Json(state.workflow_observability.snapshot().await))
}

pub(crate) fn build_api_router(state: Arc<ApiState>) -> Router {
    Router::new()
        .merge(admin_events::admin_event_routes())
        .merge(sessions::session_routes())
        .merge(extensions::extension_routes())
        .merge(credential_bindings::credential_binding_routes())
        .merge(file_workspaces::file_workspace_routes())
        .merge(workflow_events::workflow_event_subscription_routes())
        .merge(workflow_definitions::workflow_definition_routes())
        .merge(workflows::workflow_routes())
        .merge(workflow_files::workflow_file_routes())
        .merge(credential_bindings::workflow_run_credential_binding_routes())
        .merge(workflow_run_operations::workflow_run_operation_routes())
        .merge(workflow_events::workflow_run_event_routes())
        .merge(automation_tasks::automation_task_routes())
        .merge(recordings::recording_routes())
        .merge(session_files::session_file_routes())
        .merge(sessions::session_operation_routes())
        .merge(recordings::recording_operation_routes())
        .route("/api/v1/workflow/operations", get(get_workflow_operations))
        .merge(sessions::legacy_session_routes())
        .with_state(state)
}
