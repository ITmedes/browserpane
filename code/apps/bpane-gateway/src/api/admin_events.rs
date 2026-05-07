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
const WORKFLOW_RUNS_SNAPSHOT_EVENT_TYPE: &str = "workflow_runs.snapshot";
const SESSION_FILES_SNAPSHOT_EVENT_TYPE: &str = "session_files.snapshot";
const RECORDINGS_SNAPSHOT_EVENT_TYPE: &str = "recordings.snapshot";

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

#[derive(Debug, Serialize)]
struct AdminWorkflowRunsSnapshotEvent {
    event_type: &'static str,
    sequence: u64,
    created_at: chrono::DateTime<Utc>,
    workflow_runs: Vec<AdminWorkflowRunSummary>,
}

#[derive(Debug, Serialize)]
struct AdminWorkflowRunSummary {
    id: Uuid,
    session_id: Uuid,
    state: crate::workflow::WorkflowRunState,
    updated_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct AdminSessionFilesSnapshotEvent {
    event_type: &'static str,
    sequence: u64,
    created_at: chrono::DateTime<Utc>,
    session_files: Vec<AdminSessionFilesSummary>,
}

#[derive(Debug, Serialize)]
struct AdminSessionFilesSummary {
    session_id: Uuid,
    file_count: usize,
    latest_updated_at: Option<chrono::DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
struct AdminRecordingsSnapshotEvent {
    event_type: &'static str,
    sequence: u64,
    created_at: chrono::DateTime<Utc>,
    recordings: Vec<AdminRecordingsSummary>,
}

#[derive(Debug, Serialize)]
struct AdminRecordingsSummary {
    session_id: Uuid,
    recording_count: usize,
    active_count: usize,
    ready_count: usize,
    latest_updated_at: Option<chrono::DateTime<Utc>>,
}

struct AdminChangedEvent<T> {
    event: T,
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
    let mut previous_sessions_change_key: Option<Vec<u8>> = None;
    let mut previous_workflow_runs_change_key: Option<Vec<u8>> = None;
    let mut previous_session_files_change_key: Option<Vec<u8>> = None;
    let mut previous_recordings_change_key: Option<Vec<u8>> = None;
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

        ticks.tick().await;
    }
}

async fn build_sessions_snapshot(
    state: &ApiState,
    principal: &AuthenticatedPrincipal,
    sequence: u64,
) -> Result<AdminChangedEvent<AdminSessionsSnapshotEvent>, crate::session_control::SessionStoreError>
{
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
    Ok(AdminChangedEvent {
        event: AdminSessionsSnapshotEvent {
            event_type: SESSIONS_SNAPSHOT_EVENT_TYPE,
            sequence,
            created_at: Utc::now(),
            sessions: resources,
        },
        change_key,
    })
}

async fn build_workflow_runs_snapshot(
    state: &ApiState,
    principal: &AuthenticatedPrincipal,
    sequence: u64,
) -> Result<
    AdminChangedEvent<AdminWorkflowRunsSnapshotEvent>,
    crate::session_control::SessionStoreError,
> {
    let mut runs = state
        .session_store
        .list_workflow_runs_for_owner(principal)
        .await?
        .into_iter()
        .map(|run| AdminWorkflowRunSummary {
            id: run.id,
            session_id: run.session_id,
            state: run.state,
            updated_at: run.updated_at,
        })
        .collect::<Vec<_>>();
    runs.sort_by_key(|run| run.id);
    let change_key = serialized_change_key(&runs)?;
    Ok(AdminChangedEvent {
        event: AdminWorkflowRunsSnapshotEvent {
            event_type: WORKFLOW_RUNS_SNAPSHOT_EVENT_TYPE,
            sequence,
            created_at: Utc::now(),
            workflow_runs: runs,
        },
        change_key,
    })
}

async fn build_session_files_snapshot(
    state: &ApiState,
    principal: &AuthenticatedPrincipal,
    sequence: u64,
) -> Result<
    AdminChangedEvent<AdminSessionFilesSnapshotEvent>,
    crate::session_control::SessionStoreError,
> {
    let mut summaries = Vec::new();
    for session in state
        .session_store
        .list_sessions_for_owner(principal)
        .await?
    {
        let files = state
            .session_store
            .list_session_files_for_session(session.id)
            .await?;
        summaries.push(AdminSessionFilesSummary {
            session_id: session.id,
            file_count: files.len(),
            latest_updated_at: files.iter().map(|file| file.updated_at).max(),
        });
    }
    summaries.sort_by_key(|summary| summary.session_id);
    let change_key = serialized_change_key(&summaries)?;
    Ok(AdminChangedEvent {
        event: AdminSessionFilesSnapshotEvent {
            event_type: SESSION_FILES_SNAPSHOT_EVENT_TYPE,
            sequence,
            created_at: Utc::now(),
            session_files: summaries,
        },
        change_key,
    })
}

async fn build_recordings_snapshot(
    state: &ApiState,
    principal: &AuthenticatedPrincipal,
    sequence: u64,
) -> Result<
    AdminChangedEvent<AdminRecordingsSnapshotEvent>,
    crate::session_control::SessionStoreError,
> {
    let mut summaries = Vec::new();
    for session in state
        .session_store
        .list_sessions_for_owner(principal)
        .await?
    {
        let recordings = state
            .session_store
            .list_recordings_for_session(session.id)
            .await?;
        summaries.push(AdminRecordingsSummary {
            session_id: session.id,
            recording_count: recordings.len(),
            active_count: recordings
                .iter()
                .filter(|recording| recording.state.is_active())
                .count(),
            ready_count: recordings
                .iter()
                .filter(|recording| recording.state == SessionRecordingState::Ready)
                .count(),
            latest_updated_at: recordings
                .iter()
                .map(|recording| recording.updated_at)
                .max(),
        });
    }
    summaries.sort_by_key(|summary| summary.session_id);
    let change_key = serialized_change_key(&summaries)?;
    Ok(AdminChangedEvent {
        event: AdminRecordingsSnapshotEvent {
            event_type: RECORDINGS_SNAPSHOT_EVENT_TYPE,
            sequence,
            created_at: Utc::now(),
            recordings: summaries,
        },
        change_key,
    })
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

    #[test]
    fn workflow_run_snapshot_change_key_tracks_state() {
        let id = Uuid::nil();
        let session_id = Uuid::nil();
        let updated_at = Utc::now();
        let pending_key = serialized_change_key(&vec![AdminWorkflowRunSummary {
            id,
            session_id,
            state: crate::workflow::WorkflowRunState::Pending,
            updated_at,
        }])
        .unwrap();
        let running_key = serialized_change_key(&vec![AdminWorkflowRunSummary {
            id,
            session_id,
            state: crate::workflow::WorkflowRunState::Running,
            updated_at,
        }])
        .unwrap();

        assert_ne!(pending_key, running_key);
    }

    #[test]
    fn session_files_snapshot_change_key_tracks_counts() {
        let session_id = Uuid::nil();
        let empty_key = serialized_change_key(&vec![AdminSessionFilesSummary {
            session_id,
            file_count: 0,
            latest_updated_at: None,
        }])
        .unwrap();
        let file_key = serialized_change_key(&vec![AdminSessionFilesSummary {
            session_id,
            file_count: 1,
            latest_updated_at: Some(Utc::now()),
        }])
        .unwrap();

        assert_ne!(empty_key, file_key);
    }

    #[test]
    fn recordings_snapshot_change_key_tracks_counts() {
        let session_id = Uuid::nil();
        let empty_key = serialized_change_key(&vec![AdminRecordingsSummary {
            session_id,
            recording_count: 0,
            active_count: 0,
            ready_count: 0,
            latest_updated_at: None,
        }])
        .unwrap();
        let recording_key = serialized_change_key(&vec![AdminRecordingsSummary {
            session_id,
            recording_count: 1,
            active_count: 0,
            ready_count: 1,
            latest_updated_at: Some(Utc::now()),
        }])
        .unwrap();

        assert_ne!(empty_key, recording_key);
    }
}
