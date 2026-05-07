use chrono::Utc;
use serde::Serialize;
use uuid::Uuid;

use super::*;

const SESSIONS_SNAPSHOT_EVENT_TYPE: &str = "sessions.snapshot";
const WORKFLOW_RUNS_SNAPSHOT_EVENT_TYPE: &str = "workflow_runs.snapshot";
const SESSION_FILES_SNAPSHOT_EVENT_TYPE: &str = "session_files.snapshot";
const RECORDINGS_SNAPSHOT_EVENT_TYPE: &str = "recordings.snapshot";

#[derive(Debug, Serialize)]
pub(super) struct AdminSessionsSnapshotEvent {
    event_type: &'static str,
    sequence: u64,
    created_at: chrono::DateTime<Utc>,
    sessions: Vec<SessionResource>,
}

#[derive(Debug, Serialize)]
pub(super) struct AdminWorkflowRunsSnapshotEvent {
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
pub(super) struct AdminSessionFilesSnapshotEvent {
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
pub(super) struct AdminRecordingsSnapshotEvent {
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

pub(super) struct AdminChangedEvent<T> {
    pub(super) event: T,
    pub(super) change_key: Vec<u8>,
}

pub(super) async fn build_sessions_snapshot(
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

pub(super) async fn build_workflow_runs_snapshot(
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

pub(super) async fn build_session_files_snapshot(
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

pub(super) async fn build_recordings_snapshot(
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

fn serialized_change_key<T: Serialize>(
    value: &T,
) -> Result<Vec<u8>, crate::session_control::SessionStoreError> {
    serde_json::to_vec(value).map_err(|error| {
        crate::session_control::SessionStoreError::Backend(format!(
            "failed to serialize admin event snapshot: {error}"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
