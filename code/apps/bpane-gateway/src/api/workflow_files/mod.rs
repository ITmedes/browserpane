use axum::routing::get;

use super::*;

mod source_snapshot;
mod workspace_inputs;

pub(super) fn workflow_file_routes() -> Router<Arc<ApiState>> {
    Router::new()
        .route(
            "/api/v1/workflow-runs/{run_id}/source-snapshot/content",
            get(source_snapshot::get_workflow_run_source_snapshot_content),
        )
        .route(
            "/api/v1/workflow-runs/{run_id}/workspace-inputs/{input_id}/content",
            get(workspace_inputs::get_workflow_run_workspace_input_content),
        )
}

pub(super) async fn prepare_workflow_run_source_snapshot(
    state: &Arc<ApiState>,
    principal: &AuthenticatedPrincipal,
    workflow: &StoredWorkflowDefinition,
    version: &StoredWorkflowDefinitionVersion,
) -> Result<Option<WorkflowRunSourceSnapshot>, (StatusCode, Json<ErrorResponse>)> {
    source_snapshot::prepare_workflow_run_source_snapshot(state, principal, workflow, version).await
}

pub(super) async fn resolve_workflow_run_workspace_inputs(
    state: &Arc<ApiState>,
    principal: &AuthenticatedPrincipal,
    version: &StoredWorkflowDefinitionVersion,
    requests: Vec<CreateWorkflowRunWorkspaceInputRequest>,
) -> Result<Vec<WorkflowRunWorkspaceInput>, (StatusCode, Json<ErrorResponse>)> {
    workspace_inputs::resolve_workflow_run_workspace_inputs(state, principal, version, requests)
        .await
}
