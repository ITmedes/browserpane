use super::*;

pub(super) async fn runtime_is_currently_in_use(state: &ApiState) -> bool {
    let Some(session_id) = legacy_runtime_session_id(state).await else {
        return false;
    };
    let Some(snapshot) = state.registry.telemetry_snapshot_if_live(session_id).await else {
        return false;
    };
    snapshot.browser_clients > 0 || snapshot.viewer_clients > 0 || snapshot.mcp_owner
}

pub(super) fn should_block_session_stop(
    state: SessionLifecycleState,
    supports_legacy_global_routes: bool,
    runtime_in_use: bool,
) -> bool {
    supports_legacy_global_routes && state.is_runtime_candidate() && runtime_in_use
}

pub(super) async fn resolve_runtime(
    state: &ApiState,
    session_id: Uuid,
) -> Result<SessionRuntime, (StatusCode, Json<ErrorResponse>)> {
    state
        .session_manager
        .resolve(session_id)
        .await
        .map_err(map_session_manager_error)
}

pub(super) async fn resolve_runtime_compat(
    state: &ApiState,
    session_id: Uuid,
) -> Result<SessionRuntime, StatusCode> {
    state
        .session_manager
        .resolve(session_id)
        .await
        .map_err(|_| StatusCode::CONFLICT)
}

fn map_session_manager_error(error: SessionManagerError) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::CONFLICT,
        Json(ErrorResponse {
            error: error.to_string(),
        }),
    )
}

pub(super) fn ensure_legacy_runtime_routes_supported(
    state: &ApiState,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    if state
        .session_manager
        .profile()
        .supports_legacy_global_routes
    {
        return Ok(());
    }

    Err((
        StatusCode::CONFLICT,
        Json(ErrorResponse {
            error: "global compatibility routes are disabled for the current runtime backend; use /api/v1/sessions/{id}/status and /api/v1/sessions/{id}/mcp-owner instead".to_string(),
        }),
    ))
}

pub(super) async fn legacy_runtime_session_id(state: &ApiState) -> Option<Uuid> {
    state
        .session_store
        .get_runtime_candidate_session()
        .await
        .ok()
        .flatten()
        .map(|session| session.id)
}
