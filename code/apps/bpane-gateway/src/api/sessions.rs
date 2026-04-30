use axum::routing::{delete, get, post};

use super::*;

mod access;
mod crud;
mod kill;
mod mcp;
mod ownership;
mod status;
mod stop;

pub(super) fn session_routes() -> Router<Arc<ApiState>> {
    Router::new()
        .route(
            "/api/v1/sessions",
            post(crud::create_session).get(crud::list_sessions),
        )
        .route(
            "/api/v1/sessions/{session_id}",
            get(crud::get_session).delete(stop::delete_session),
        )
}

pub(super) fn session_operation_routes() -> Router<Arc<ApiState>> {
    Router::new()
        .route(
            "/api/v1/sessions/{session_id}/access-tokens",
            post(access::issue_session_access_token),
        )
        .route(
            "/api/v1/sessions/{session_id}/automation-access",
            post(access::issue_session_automation_access),
        )
        .route(
            "/api/v1/sessions/{session_id}/automation-owner",
            post(ownership::set_automation_owner).delete(ownership::clear_automation_owner),
        )
        .route(
            "/api/v1/sessions/{session_id}/status",
            get(status::get_session_status),
        )
        .route(
            "/api/v1/sessions/{session_id}/kill",
            post(kill::kill_session),
        )
        .route(
            "/api/v1/sessions/{session_id}/stop",
            post(stop::stop_session),
        )
        .route(
            "/api/v1/sessions/{session_id}/mcp-owner",
            post(mcp::set_session_mcp_owner).delete(mcp::clear_session_mcp_owner),
        )
}

pub(super) fn legacy_session_routes() -> Router<Arc<ApiState>> {
    Router::new()
        .route("/api/session/status", get(status::session_status))
        .route("/api/session/mcp-owner", post(mcp::set_mcp_owner))
        .route("/api/session/mcp-owner", delete(mcp::clear_mcp_owner))
}
