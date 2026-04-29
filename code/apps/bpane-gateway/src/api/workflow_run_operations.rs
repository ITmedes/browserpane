use super::*;

mod controls;
mod produced_files;

pub(super) fn workflow_run_operation_routes() -> Router<Arc<ApiState>> {
    Router::new()
        .merge(produced_files::workflow_run_produced_file_routes())
        .merge(controls::workflow_run_control_routes())
}
