use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::Query;
use axum::http::header::AUTHORIZATION;
use axum::routing::get;
use serde::{Deserialize, Serialize};
use tokio::time::{interval, MissedTickBehavior};

use super::*;

const ADMIN_EVENT_POLL_INTERVAL: Duration = Duration::from_millis(750);
const SESSIONS_SNAPSHOT_EVENT_TYPE: &str = "sessions.snapshot";

#[derive(Debug, Deserialize)]
struct AdminEventsQuery {
    #[serde(default)]
    access_token: Option<String>,
}

#[derive(Debug, Serialize)]
struct AdminSessionsSnapshotEvent {
    event_type: &'static str,
    sequence: u64,
    created_at: chrono::DateTime<Utc>,
    sessions: Vec<SessionResource>,
}

struct AdminSessionsSnapshot {
    event: AdminSessionsSnapshotEvent,
    change_key: Vec<u8>,
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
    let mut previous_change_key: Option<Vec<u8>> = None;
    let mut ticks = interval(ADMIN_EVENT_POLL_INTERVAL);
    ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        let snapshot = build_sessions_snapshot(&state, &principal, sequence).await;
        match snapshot {
            Ok(snapshot) if previous_change_key.as_ref() != Some(&snapshot.change_key) => {
                let Ok(payload) = serde_json::to_string(&snapshot.event) else {
                    return;
                };
                if socket.send(Message::Text(payload.into())).await.is_err() {
                    return;
                }
                previous_change_key = Some(snapshot.change_key);
                sequence += 1;
            }
            Ok(_) => {}
            Err(error) => {
                let payload = serde_json::json!({
                    "event_type": "admin.error",
                    "sequence": sequence,
                    "created_at": Utc::now(),
                    "error": error.to_string()
                });
                if socket
                    .send(Message::Text(payload.to_string().into()))
                    .await
                    .is_err()
                {
                    return;
                }
                sequence += 1;
            }
        }

        ticks.tick().await;
    }
}

async fn build_sessions_snapshot(
    state: &ApiState,
    principal: &AuthenticatedPrincipal,
    sequence: u64,
) -> Result<AdminSessionsSnapshot, crate::session_control::SessionStoreError> {
    let mut resources = Vec::new();
    for session in state
        .session_store
        .list_sessions_for_owner(principal)
        .await?
    {
        resources.push(session_resource(state, &session, None).await?);
    }
    resources.sort_by_key(|session| session.id);
    let change_key = serialized_change_key(&resources)?;
    Ok(AdminSessionsSnapshot {
        event: AdminSessionsSnapshotEvent {
            event_type: SESSIONS_SNAPSHOT_EVENT_TYPE,
            sequence,
            created_at: Utc::now(),
            sessions: resources,
        },
        change_key,
    })
}

fn serialized_change_key<T: Serialize>(
    value: &T,
) -> Result<Vec<u8>, crate::session_control::SessionStoreError> {
    serde_json::to_vec(value).map_err(|error| {
        crate::session_control::SessionStoreError::Backend(format!(
            "failed to serialize admin event snapshot: {error}"
        ))
    })
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

    #[test]
    fn session_snapshot_change_key_ignores_event_metadata() {
        let payload = vec!["session-a".to_string(), "session-b".to_string()];
        let first_key = serialized_change_key(&payload).unwrap();
        let second_key = serialized_change_key(&payload).unwrap();

        assert_eq!(first_key, second_key);
    }
}
