use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::auth::AuthValidator;
use crate::session_hub::SessionTelemetrySnapshot;
use crate::session_registry::SessionRegistry;

/// Shared state for the HTTP API.
struct ApiState {
    registry: Arc<SessionRegistry>,
    auth_validator: Arc<AuthValidator>,
    agent_socket_path: String,
}

#[derive(Serialize)]
struct SessionStatus {
    browser_clients: u32,
    viewer_clients: u32,
    max_viewers: u32,
    viewer_slots_remaining: u32,
    exclusive_browser_owner: bool,
    mcp_owner: bool,
    resolution: (u16, u16),
    telemetry: SessionTelemetry,
}

#[derive(Serialize)]
struct SessionTelemetry {
    joins_accepted: u64,
    joins_rejected_viewer_cap: u64,
    last_join_latency_ms: u64,
    average_join_latency_ms: f64,
    max_join_latency_ms: u64,
    full_refresh_requests: u64,
    full_refresh_tiles_requested: u64,
    last_full_refresh_tiles: u64,
    max_full_refresh_tiles: u64,
    egress_send_stream_lock_acquires_total: u64,
    egress_send_stream_lock_wait_us_total: u64,
    egress_send_stream_lock_wait_us_average: f64,
    egress_send_stream_lock_wait_us_max: u64,
    egress_lagged_receives_total: u64,
    egress_lagged_frames_total: u64,
}

#[derive(Deserialize)]
struct McpOwnerRequest {
    width: u16,
    height: u16,
}

#[derive(Serialize)]
struct OkResponse {
    ok: bool,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

/// GET /api/session/status
async fn session_status(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionStatus>, StatusCode> {
    authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    let hub = state
        .registry
        .ensure_hub(&state.agent_socket_path)
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    let snapshot = hub.telemetry_snapshot().await;

    Ok(Json(session_status_from_snapshot(snapshot)))
}

fn session_status_from_snapshot(snapshot: SessionTelemetrySnapshot) -> SessionStatus {
    SessionStatus {
        browser_clients: snapshot.browser_clients,
        viewer_clients: snapshot.viewer_clients,
        max_viewers: snapshot.max_viewers,
        viewer_slots_remaining: snapshot.viewer_slots_remaining,
        exclusive_browser_owner: snapshot.exclusive_browser_owner,
        mcp_owner: snapshot.mcp_owner,
        resolution: snapshot.resolution,
        telemetry: SessionTelemetry {
            joins_accepted: snapshot.joins_accepted,
            joins_rejected_viewer_cap: snapshot.joins_rejected_viewer_cap,
            last_join_latency_ms: snapshot.last_join_latency_ms,
            average_join_latency_ms: snapshot.average_join_latency_ms,
            max_join_latency_ms: snapshot.max_join_latency_ms,
            full_refresh_requests: snapshot.full_refresh_requests,
            full_refresh_tiles_requested: snapshot.full_refresh_tiles_requested,
            last_full_refresh_tiles: snapshot.last_full_refresh_tiles,
            max_full_refresh_tiles: snapshot.max_full_refresh_tiles,
            egress_send_stream_lock_acquires_total: snapshot.egress_send_stream_lock_acquires_total,
            egress_send_stream_lock_wait_us_total: snapshot.egress_send_stream_lock_wait_us_total,
            egress_send_stream_lock_wait_us_average: snapshot
                .egress_send_stream_lock_wait_us_average,
            egress_send_stream_lock_wait_us_max: snapshot.egress_send_stream_lock_wait_us_max,
            egress_lagged_receives_total: snapshot.egress_lagged_receives_total,
            egress_lagged_frames_total: snapshot.egress_lagged_frames_total,
        },
    }
}

/// POST /api/session/mcp-owner
async fn set_mcp_owner(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
    Json(req): Json<McpOwnerRequest>,
) -> Result<Json<OkResponse>, (StatusCode, Json<ErrorResponse>)> {
    authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let hub = state
        .registry
        .ensure_hub(&state.agent_socket_path)
        .await
        .map_err(|e| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: format!("failed to connect to host agent: {e}"),
                }),
            )
        })?;

    hub.set_mcp_owner(req.width, req.height).await;

    Ok(Json(OkResponse { ok: true }))
}

/// DELETE /api/session/mcp-owner
async fn clear_mcp_owner(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<OkResponse>, StatusCode> {
    authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    let hub = state
        .registry
        .ensure_hub(&state.agent_socket_path)
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    hub.clear_mcp_owner().await;

    Ok(Json(OkResponse { ok: true }))
}

/// Runs the HTTP API server for MCP bridge communication.
pub async fn run_api_server(
    bind_addr: SocketAddr,
    registry: Arc<SessionRegistry>,
    auth_validator: Arc<AuthValidator>,
    agent_socket_path: String,
) -> anyhow::Result<()> {
    let state = Arc::new(ApiState {
        registry,
        auth_validator,
        agent_socket_path,
    });

    let app = Router::new()
        .route("/api/session/status", get(session_status))
        .route("/api/session/mcp-owner", post(set_mcp_owner))
        .route("/api/session/mcp-owner", delete(clear_mcp_owner))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    info!("HTTP API listening on {bind_addr}");

    axum::serve(listener, app).await?;

    Ok(())
}

async fn authorize_api_request(
    headers: &HeaderMap,
    auth_validator: &AuthValidator,
) -> Result<(), String> {
    let token = extract_bearer_token(headers).ok_or_else(|| "missing bearer token".to_string())?;
    auth_validator
        .validate_token(token)
        .await
        .map_err(|error| format!("invalid bearer token: {error}"))
}

fn extract_bearer_token(headers: &HeaderMap) -> Option<&str> {
    let value = headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    value.strip_prefix("Bearer ")
}
