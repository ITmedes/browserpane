use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::header::{CACHE_CONTROL, PRAGMA};
use axum::routing::{get, post};
use chrono::DateTime;
use serde::Serialize;
use tokio::time::{interval, MissedTickBehavior};

use super::*;
use snapshots::{
    build_mcp_delegation_snapshot, build_recordings_snapshot, build_session_files_snapshot,
    build_sessions_snapshot, build_workflow_runs_snapshot, AdminChangedEvent,
};

const ADMIN_EVENT_POLL_INTERVAL: Duration = Duration::from_millis(750);

mod authentication;
mod snapshots;

#[derive(Debug, Serialize)]
struct AdminEventAccessTokenResponse {
    token_type: &'static str,
    token: String,
    expires_at: DateTime<Utc>,
    websocket: AdminEventWebSocketAccess,
}

#[derive(Debug, Serialize)]
struct AdminEventWebSocketAccess {
    endpoint_path: &'static str,
    auth_type: &'static str,
    authentication_message_type: &'static str,
    authenticated_message_type: &'static str,
}

pub(super) fn admin_event_routes() -> Router<Arc<ApiState>> {
    Router::new()
        .route("/api/v1/admin/events", get(open_admin_events))
        .route(
            "/api/v1/admin/events/access-tokens",
            post(issue_admin_event_access_token),
        )
}

async fn issue_admin_event_access_token(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
) -> Result<(HeaderMap, Json<AdminEventAccessTokenResponse>), (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let issued = state
        .admin_event_access_token_manager
        .issue_token(&principal)
        .map_err(|error| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: format!("failed to issue admin event access token: {error}"),
                }),
            )
        })?;
    let mut response_headers = HeaderMap::new();
    response_headers.insert(
        CACHE_CONTROL,
        "no-store".parse().expect("valid header value"),
    );
    response_headers.insert(PRAGMA, "no-cache".parse().expect("valid header value"));
    Ok((
        response_headers,
        Json(AdminEventAccessTokenResponse {
            token_type: "admin_event_access_token",
            token: issued.token,
            expires_at: issued.expires_at,
            websocket: AdminEventWebSocketAccess {
                endpoint_path: "/api/v1/admin/events",
                auth_type: "initial_message",
                authentication_message_type: "admin.authenticate",
                authenticated_message_type: "admin.authenticated",
            },
        }),
    ))
}

async fn open_admin_events(State(state): State<Arc<ApiState>>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(move |socket| authenticate_and_stream_admin_events(socket, state))
}

async fn authenticate_and_stream_admin_events(mut socket: WebSocket, state: Arc<ApiState>) {
    let Some(principal) =
        authentication::authenticate_socket(&mut socket, &state.admin_event_access_token_manager)
            .await
    else {
        return;
    };
    stream_admin_events(socket, state, principal).await;
}

async fn stream_admin_events(
    mut socket: WebSocket,
    state: Arc<ApiState>,
    principal: AuthenticatedPrincipal,
) {
    let mut sequence = 1;
    let mut previous_sessions_change_key: Option<Vec<u8>> = None;
    let mut previous_workflow_runs_change_key: Option<Vec<u8>> = None;
    let mut previous_session_files_change_key: Option<Vec<u8>> = None;
    let mut previous_recordings_change_key: Option<Vec<u8>> = None;
    let mut previous_mcp_delegation_change_key: Option<Vec<u8>> = None;
    let mut ticks = interval(ADMIN_EVENT_POLL_INTERVAL);
    ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        let sessions_snapshot = build_sessions_snapshot(&state, &principal, sequence).await;
        if emit_changed_event(
            &mut socket,
            &mut sequence,
            &mut previous_sessions_change_key,
            sessions_snapshot,
        )
        .await
        .is_err()
        {
            return;
        }
        let workflow_runs_snapshot =
            build_workflow_runs_snapshot(&state, &principal, sequence).await;
        if emit_changed_event(
            &mut socket,
            &mut sequence,
            &mut previous_workflow_runs_change_key,
            workflow_runs_snapshot,
        )
        .await
        .is_err()
        {
            return;
        }
        let session_files_snapshot =
            build_session_files_snapshot(&state, &principal, sequence).await;
        if emit_changed_event(
            &mut socket,
            &mut sequence,
            &mut previous_session_files_change_key,
            session_files_snapshot,
        )
        .await
        .is_err()
        {
            return;
        }
        let recordings_snapshot = build_recordings_snapshot(&state, &principal, sequence).await;
        if emit_changed_event(
            &mut socket,
            &mut sequence,
            &mut previous_recordings_change_key,
            recordings_snapshot,
        )
        .await
        .is_err()
        {
            return;
        }
        let mcp_delegation_snapshot =
            build_mcp_delegation_snapshot(&state, &principal, sequence).await;
        if emit_changed_event(
            &mut socket,
            &mut sequence,
            &mut previous_mcp_delegation_change_key,
            mcp_delegation_snapshot,
        )
        .await
        .is_err()
        {
            return;
        }

        ticks.tick().await;
    }
}

async fn emit_changed_event<T: Serialize>(
    socket: &mut WebSocket,
    sequence: &mut u64,
    previous_change_key: &mut Option<Vec<u8>>,
    snapshot: Result<AdminChangedEvent<T>, crate::session_control::SessionStoreError>,
) -> Result<(), ()> {
    match snapshot {
        Ok(snapshot) if previous_change_key.as_ref() != Some(&snapshot.change_key) => {
            let payload = serde_json::to_string(&snapshot.event).map_err(|_| ())?;
            if socket.send(Message::Text(payload.into())).await.is_err() {
                return Err(());
            }
            *previous_change_key = Some(snapshot.change_key);
            *sequence += 1;
        }
        Ok(_) => {}
        Err(error) => {
            emit_admin_error(socket, sequence, error.to_string()).await?;
        }
    }
    Ok(())
}

async fn emit_admin_error(
    socket: &mut WebSocket,
    sequence: &mut u64,
    error: String,
) -> Result<(), ()> {
    let payload = serde_json::json!({
        "event_type": "admin.error",
        "sequence": *sequence,
        "created_at": Utc::now(),
        "error": error
    });
    if socket
        .send(Message::Text(payload.to_string().into()))
        .await
        .is_err()
    {
        return Err(());
    }
    *sequence += 1;
    Ok(())
}
