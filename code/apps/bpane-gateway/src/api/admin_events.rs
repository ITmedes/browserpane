use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::Query;
use axum::http::header::AUTHORIZATION;
use axum::routing::get;
use serde::{Deserialize, Serialize};
use tokio::time::{interval, MissedTickBehavior};

use super::*;
use snapshots::{
    build_mcp_delegation_snapshot, build_recordings_snapshot, build_session_files_snapshot,
    build_sessions_snapshot, build_workflow_runs_snapshot, AdminChangedEvent,
};

const ADMIN_EVENT_POLL_INTERVAL: Duration = Duration::from_millis(750);

mod snapshots;

#[derive(Debug, Deserialize)]
struct AdminEventsQuery {
    #[serde(default)]
    access_token: Option<String>,
}

pub(super) fn admin_event_routes() -> Router<Arc<ApiState>> {
    Router::new().route("/api/v1/admin/events", get(open_admin_events))
}

async fn open_admin_events(
    headers: HeaderMap,
    Query(query): Query<AdminEventsQuery>,
    State(state): State<Arc<ApiState>>,
    ws: WebSocketUpgrade,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let principal = authenticate_admin_events_request(&headers, &query, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;

    Ok(ws.on_upgrade(move |socket| stream_admin_events(socket, state, principal)))
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

async fn authenticate_admin_events_request(
    headers: &HeaderMap,
    query: &AdminEventsQuery,
    auth_validator: &AuthValidator,
) -> Result<AuthenticatedPrincipal, String> {
    let Some(token) = admin_events_token(headers, query) else {
        return Err("missing bearer token".to_string());
    };
    auth_validator
        .authenticate(token)
        .await
        .map_err(|error| format!("invalid bearer token: {error}"))
}

fn admin_events_token<'a>(headers: &'a HeaderMap, query: &'a AdminEventsQuery) -> Option<&'a str> {
    let header_token = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    header_token.or(query.access_token.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;

    #[tokio::test]
    async fn admin_event_auth_accepts_browser_query_token() {
        let auth_validator = AuthValidator::from_hmac_secret(vec![7; 32]);
        let token = auth_validator.generate_token().unwrap();
        let principal = authenticate_admin_events_request(
            &HeaderMap::new(),
            &AdminEventsQuery {
                access_token: Some(token),
            },
            &auth_validator,
        )
        .await
        .unwrap();

        assert_eq!(principal.issuer, "bpane-gateway");
    }

    #[tokio::test]
    async fn admin_event_auth_requires_a_token() {
        let auth_validator = AuthValidator::from_hmac_secret(vec![7; 32]);
        let error = authenticate_admin_events_request(
            &HeaderMap::new(),
            &AdminEventsQuery { access_token: None },
            &auth_validator,
        )
        .await
        .unwrap_err();

        assert_eq!(error, "missing bearer token");
    }
}
