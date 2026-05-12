mod create;
mod read;

use axum::routing::get;

use super::*;

pub(super) fn workflow_routes() -> Router<Arc<ApiState>> {
    Router::new()
        .route(
            "/api/v1/workflow-runs",
            get(read::list_workflow_runs).post(create::create_workflow_run),
        )
        .route(
            "/api/v1/workflow-runs/{run_id}",
            get(read::get_workflow_run),
        )
}
