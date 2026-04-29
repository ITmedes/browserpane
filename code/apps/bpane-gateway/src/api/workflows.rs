mod create;
mod read;

use axum::routing::{get, post};

use super::*;

pub(super) fn workflow_routes() -> Router<Arc<ApiState>> {
    Router::new()
        .route("/api/v1/workflow-runs", post(create::create_workflow_run))
        .route(
            "/api/v1/workflow-runs/{run_id}",
            get(read::get_workflow_run),
        )
}
