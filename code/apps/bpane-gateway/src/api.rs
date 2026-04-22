use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tracing::info;
use uuid::Uuid;

use crate::auth::{AuthValidator, AuthenticatedPrincipal};
use crate::connect_ticket::SessionConnectTicketManager;
use crate::runtime_manager::{ResolvedSessionRuntime, RuntimeManagerError, SessionRuntimeManager};
use crate::session_control::{
    CreateSessionRequest, SessionLifecycleState, SessionListResponse, SessionOwnerMode,
    SessionResource, SessionStore, SessionStoreError, SetAutomationDelegateRequest, StoredSession,
};
use crate::session_hub::SessionTelemetrySnapshot;
use crate::session_registry::SessionRegistry;

/// Shared state for the HTTP API.
struct ApiState {
    registry: Arc<SessionRegistry>,
    auth_validator: Arc<AuthValidator>,
    connect_ticket_manager: Arc<SessionConnectTicketManager>,
    session_store: SessionStore,
    runtime_manager: Arc<SessionRuntimeManager>,
    public_gateway_url: String,
    default_owner_mode: SessionOwnerMode,
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

#[derive(Serialize)]
struct SessionAccessTokenResponse {
    session_id: Uuid,
    token_type: String,
    token: String,
    expires_at: chrono::DateTime<chrono::Utc>,
    connect: crate::session_control::SessionConnectInfo,
}

async fn create_session(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CreateSessionRequest>,
) -> Result<(StatusCode, Json<SessionResource>), (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let owner_mode = resolve_owner_mode(&state, request.owner_mode)?;
    let stored = state
        .session_store
        .create_session(&principal, request, owner_mode)
        .await
        .map_err(map_session_store_error)?;

    Ok((
        StatusCode::CREATED,
        Json(stored.to_resource(
            &state.public_gateway_url,
            state.session_store.compatibility_mode(),
            None,
        )),
    ))
}

async fn list_sessions(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let sessions = state
        .session_store
        .list_sessions_for_owner(&principal)
        .await
        .map_err(map_session_store_error)?
        .into_iter()
        .map(|session| {
            session.to_resource(
                &state.public_gateway_url,
                state.session_store.compatibility_mode(),
                None,
            )
        })
        .collect();

    Ok(Json(SessionListResponse { sessions }))
}

async fn get_session(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionResource>, (StatusCode, Json<ErrorResponse>)> {
    let stored = authorize_visible_session_request(&headers, &state, session_id).await?;

    Ok(Json(stored.to_resource(
        &state.public_gateway_url,
        state.session_store.compatibility_mode(),
        None,
    )))
}

async fn set_automation_owner(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<SetAutomationDelegateRequest>,
) -> Result<Json<SessionResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let stored = state
        .session_store
        .set_automation_delegate_for_owner(&principal, session_id, request)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("session {session_id} not found"),
                }),
            )
        })?;

    Ok(Json(stored.to_resource(
        &state.public_gateway_url,
        state.session_store.compatibility_mode(),
        None,
    )))
}

async fn clear_automation_owner(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let stored = state
        .session_store
        .clear_automation_delegate_for_owner(&principal, session_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("session {session_id} not found"),
                }),
            )
        })?;

    Ok(Json(stored.to_resource(
        &state.public_gateway_url,
        state.session_store.compatibility_mode(),
        None,
    )))
}

async fn issue_session_access_token(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionAccessTokenResponse>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let stored = state
        .session_store
        .get_session_for_principal(&principal, session_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("session {session_id} not found"),
                }),
            )
        })?;

    if !stored.state.is_runtime_candidate() {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!(
                    "session {session_id} is not connectable in state {}",
                    stored.state.as_str()
                ),
            }),
        ));
    }

    let issued = state
        .connect_ticket_manager
        .issue_ticket(session_id, &principal)
        .map_err(|error| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: format!("failed to issue session connect ticket: {error}"),
                }),
            )
        })?;
    let resource = stored.to_resource(
        &state.public_gateway_url,
        state.session_store.compatibility_mode(),
        None,
    );

    Ok(Json(SessionAccessTokenResponse {
        session_id,
        token_type: "session_connect_ticket".to_string(),
        token: issued.token,
        expires_at: issued.expires_at,
        connect: resource.connect,
    }))
}

async fn get_session_status(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionStatus>, (StatusCode, Json<ErrorResponse>)> {
    let _session = authorize_runtime_session_request(&headers, &state, session_id).await?;
    let hub = state
        .registry
        .ensure_hub_for_session(
            session_id,
            &resolve_runtime(&state, session_id).await?.agent_socket_path,
        )
        .await
        .map_err(|error| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: format!("failed to connect to host agent: {error}"),
                }),
            )
        })?;
    let snapshot = hub.telemetry_snapshot().await;

    Ok(Json(session_status_from_snapshot(snapshot)))
}

async fn delete_session(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;

    let stored = state
        .session_store
        .get_session_for_owner(&principal, session_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("session {session_id} not found"),
                }),
            )
        })?;

    if stored.state.is_runtime_candidate() && runtime_is_currently_in_use(&state).await {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: "cannot stop the legacy single-session runtime while it is in use"
                    .to_string(),
            }),
        ));
    }

    let stopped = state
        .session_store
        .stop_session_for_owner(&principal, session_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("session {session_id} not found"),
                }),
            )
        })?;

    state.runtime_manager.release(session_id).await;

    Ok(Json(stopped.to_resource(
        &state.public_gateway_url,
        state.session_store.compatibility_mode(),
        None,
    )))
}

/// GET /api/session/status
async fn session_status(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionStatus>, (StatusCode, Json<ErrorResponse>)> {
    authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    ensure_legacy_runtime_routes_supported(&state)?;
    let Some(session_id) = legacy_runtime_session_id(&state).await else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "no runtime-backed session is available".to_string(),
            }),
        ));
    };
    let runtime = resolve_runtime_compat(&state, session_id)
        .await
        .map_err(map_runtime_compat_status)?;
    let hub = state
        .registry
        .ensure_hub_for_session(session_id, &runtime.agent_socket_path)
        .await
        .map_err(|error| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: format!("failed to connect to host agent: {error}"),
                }),
            )
        })?;
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
    ensure_legacy_runtime_routes_supported(&state)?;
    let Some(session_id) = legacy_runtime_session_id(&state).await else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "no runtime-backed session is available".to_string(),
            }),
        ));
    };
    let runtime = resolve_runtime(&state, session_id).await?;
    let hub = state
        .registry
        .ensure_hub_for_session(session_id, &runtime.agent_socket_path)
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
    state.runtime_manager.mark_session_active(session_id).await;

    Ok(Json(OkResponse { ok: true }))
}

async fn set_session_mcp_owner(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(req): Json<McpOwnerRequest>,
) -> Result<Json<OkResponse>, (StatusCode, Json<ErrorResponse>)> {
    let _session = authorize_runtime_session_request(&headers, &state, session_id).await?;
    let runtime = resolve_runtime(&state, session_id).await?;
    let hub = state
        .registry
        .ensure_hub_for_session(session_id, &runtime.agent_socket_path)
        .await
        .map_err(|error| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: format!("failed to connect to host agent: {error}"),
                }),
            )
        })?;

    hub.set_mcp_owner(req.width, req.height).await;
    state.runtime_manager.mark_session_active(session_id).await;

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
    if !state
        .runtime_manager
        .profile()
        .supports_legacy_global_routes
    {
        return Err(StatusCode::CONFLICT);
    }
    let Some(session_id) = legacy_runtime_session_id(&state).await else {
        return Err(StatusCode::NOT_FOUND);
    };
    let runtime = resolve_runtime_compat(&state, session_id)
        .await
        .map_err(|status| status)?;
    let hub = state
        .registry
        .ensure_hub_for_session(session_id, &runtime.agent_socket_path)
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    hub.clear_mcp_owner().await;
    let snapshot = hub.telemetry_snapshot().await;
    if snapshot.browser_clients == 0 && snapshot.viewer_clients == 0 && !snapshot.mcp_owner {
        state.runtime_manager.mark_session_idle(session_id).await;
    }

    Ok(Json(OkResponse { ok: true }))
}

async fn clear_session_mcp_owner(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<OkResponse>, (StatusCode, Json<ErrorResponse>)> {
    let _session = authorize_runtime_session_request(&headers, &state, session_id).await?;
    let runtime = resolve_runtime(&state, session_id).await?;
    let hub = state
        .registry
        .ensure_hub_for_session(session_id, &runtime.agent_socket_path)
        .await
        .map_err(|error| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: format!("failed to connect to host agent: {error}"),
                }),
            )
        })?;

    hub.clear_mcp_owner().await;
    let snapshot = hub.telemetry_snapshot().await;
    if snapshot.browser_clients == 0 && snapshot.viewer_clients == 0 && !snapshot.mcp_owner {
        state.runtime_manager.mark_session_idle(session_id).await;
    }

    Ok(Json(OkResponse { ok: true }))
}

/// Runs the HTTP API server for MCP bridge communication.
pub async fn run_api_server(
    bind_addr: SocketAddr,
    registry: Arc<SessionRegistry>,
    auth_validator: Arc<AuthValidator>,
    connect_ticket_manager: Arc<SessionConnectTicketManager>,
    session_store: SessionStore,
    runtime_manager: Arc<SessionRuntimeManager>,
    public_gateway_url: String,
    default_owner_mode: SessionOwnerMode,
) -> anyhow::Result<()> {
    let state = Arc::new(ApiState {
        registry,
        auth_validator,
        connect_ticket_manager,
        session_store,
        runtime_manager,
        public_gateway_url,
        default_owner_mode,
    });

    let app = build_api_router(state);

    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    info!("HTTP API listening on {bind_addr}");

    axum::serve(listener, app).await?;

    Ok(())
}

async fn authorize_api_request(
    headers: &HeaderMap,
    auth_validator: &AuthValidator,
) -> Result<AuthenticatedPrincipal, String> {
    let token = extract_bearer_token(headers).ok_or_else(|| "missing bearer token".to_string())?;
    auth_validator
        .authenticate(token)
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

fn build_api_router(state: Arc<ApiState>) -> Router {
    Router::new()
        .route("/api/v1/sessions", post(create_session).get(list_sessions))
        .route(
            "/api/v1/sessions/{session_id}",
            get(get_session).delete(delete_session),
        )
        .route(
            "/api/v1/sessions/{session_id}/access-tokens",
            post(issue_session_access_token),
        )
        .route(
            "/api/v1/sessions/{session_id}/automation-owner",
            post(set_automation_owner).delete(clear_automation_owner),
        )
        .route(
            "/api/v1/sessions/{session_id}/status",
            get(get_session_status),
        )
        .route(
            "/api/v1/sessions/{session_id}/mcp-owner",
            post(set_session_mcp_owner).delete(clear_session_mcp_owner),
        )
        .route("/api/session/status", get(session_status))
        .route("/api/session/mcp-owner", post(set_mcp_owner))
        .route("/api/session/mcp-owner", delete(clear_mcp_owner))
        .with_state(state)
}

fn resolve_owner_mode(
    state: &ApiState,
    requested: Option<SessionOwnerMode>,
) -> Result<SessionOwnerMode, (StatusCode, Json<ErrorResponse>)> {
    let resolved = requested.unwrap_or(state.default_owner_mode);
    if resolved != state.default_owner_mode {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!(
                    "owner_mode {} is not supported by the current gateway runtime",
                    resolved.as_str()
                ),
            }),
        ));
    }
    Ok(resolved)
}

fn map_session_store_error(error: SessionStoreError) -> (StatusCode, Json<ErrorResponse>) {
    match error {
        SessionStoreError::ActiveSessionConflict { .. } => (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        ),
        SessionStoreError::InvalidRequest(_) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        ),
        SessionStoreError::Backend(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        ),
    }
}

async fn authorize_runtime_session_request(
    headers: &HeaderMap,
    state: &ApiState,
    session_id: Uuid,
) -> Result<StoredSession, (StatusCode, Json<ErrorResponse>)> {
    let session = authorize_visible_session_request(headers, state, session_id).await?;

    if !matches!(
        session.state,
        SessionLifecycleState::Pending
            | SessionLifecycleState::Starting
            | SessionLifecycleState::Ready
            | SessionLifecycleState::Active
            | SessionLifecycleState::Idle
    ) {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!(
                    "session {session_id} is not attached to a runtime-compatible state"
                ),
            }),
        ));
    }

    Ok(session)
}

async fn authorize_visible_session_request(
    headers: &HeaderMap,
    state: &ApiState,
    session_id: Uuid,
) -> Result<StoredSession, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let session = state
        .session_store
        .get_session_for_principal(&principal, session_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("session {session_id} not found"),
                }),
            )
        })?;

    Ok(session)
}

async fn runtime_is_currently_in_use(state: &ApiState) -> bool {
    let Some(session_id) = legacy_runtime_session_id(state).await else {
        return false;
    };
    let Some(snapshot) = state.registry.telemetry_snapshot_if_live(session_id).await else {
        return false;
    };
    snapshot.browser_clients > 0 || snapshot.viewer_clients > 0 || snapshot.mcp_owner
}

async fn resolve_runtime(
    state: &ApiState,
    session_id: Uuid,
) -> Result<ResolvedSessionRuntime, (StatusCode, Json<ErrorResponse>)> {
    state
        .runtime_manager
        .resolve(session_id)
        .await
        .map_err(map_runtime_manager_error)
}

async fn resolve_runtime_compat(
    state: &ApiState,
    session_id: Uuid,
) -> Result<ResolvedSessionRuntime, StatusCode> {
    state
        .runtime_manager
        .resolve(session_id)
        .await
        .map_err(|_| StatusCode::CONFLICT)
}

fn map_runtime_manager_error(error: RuntimeManagerError) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::CONFLICT,
        Json(ErrorResponse {
            error: error.to_string(),
        }),
    )
}

fn map_runtime_compat_status(status: StatusCode) -> (StatusCode, Json<ErrorResponse>) {
    (
        status,
        Json(ErrorResponse {
            error: if status == StatusCode::CONFLICT {
                "runtime is not currently available for the requested compatibility route"
                    .to_string()
            } else {
                "compatibility route failed".to_string()
            },
        }),
    )
}

fn ensure_legacy_runtime_routes_supported(
    state: &ApiState,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    if state
        .runtime_manager
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

async fn legacy_runtime_session_id(state: &ApiState) -> Option<Uuid> {
    state
        .session_store
        .get_runtime_candidate_session()
        .await
        .ok()
        .flatten()
        .map(|session| session.id)
}

#[cfg(test)]
mod tests;
