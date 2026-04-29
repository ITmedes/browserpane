use axum::routing::{get, post};

use super::*;

mod content;
mod lifecycle;
mod operations;
mod playback;

pub(super) fn recording_routes() -> Router<Arc<ApiState>> {
    Router::new()
        .route(
            "/api/v1/sessions/{session_id}/recordings",
            post(lifecycle::create_session_recording).get(lifecycle::list_session_recordings),
        )
        .route(
            "/api/v1/sessions/{session_id}/recordings/{recording_id}",
            get(lifecycle::get_session_recording),
        )
        .route(
            "/api/v1/sessions/{session_id}/recordings/{recording_id}/stop",
            post(lifecycle::stop_session_recording),
        )
        .route(
            "/api/v1/sessions/{session_id}/recordings/{recording_id}/complete",
            post(lifecycle::complete_session_recording),
        )
        .route(
            "/api/v1/sessions/{session_id}/recordings/{recording_id}/fail",
            post(lifecycle::fail_session_recording),
        )
        .route(
            "/api/v1/sessions/{session_id}/recordings/{recording_id}/content",
            get(content::get_session_recording_content),
        )
        .route(
            "/api/v1/sessions/{session_id}/recording-playback",
            get(playback::get_session_recording_playback),
        )
        .route(
            "/api/v1/sessions/{session_id}/recording-playback/manifest",
            get(playback::get_session_recording_playback_manifest),
        )
        .route(
            "/api/v1/sessions/{session_id}/recording-playback/export",
            get(playback::get_session_recording_playback_export),
        )
}

pub(super) fn recording_operation_routes() -> Router<Arc<ApiState>> {
    Router::new().route(
        "/api/v1/recording/operations",
        get(operations::get_recording_operations),
    )
}
