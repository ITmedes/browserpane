use chrono::Utc;
use serde::Serialize;
use uuid::Uuid;

use super::*;

const SESSIONS_SNAPSHOT_EVENT_TYPE: &str = "sessions.snapshot";
const WORKFLOW_RUNS_SNAPSHOT_EVENT_TYPE: &str = "workflow_runs.snapshot";
const SESSION_FILES_SNAPSHOT_EVENT_TYPE: &str = "session_files.snapshot";
const RECORDINGS_SNAPSHOT_EVENT_TYPE: &str = "recordings.snapshot";
const MCP_DELEGATION_SNAPSHOT_EVENT_TYPE: &str = "mcp_delegation.snapshot";

#[derive(Debug, Serialize)]
pub(super) struct AdminSessionsSnapshotEvent {
    event_type: &'static str,
    sequence: u64,
    created_at: chrono::DateTime<Utc>,
    sessions: Vec<SessionResource>,
}

#[derive(Debug, Serialize)]
struct AdminSessionSnapshotChangeSummary {
    id: Uuid,
    state: crate::session_control::SessionLifecycleState,
    template_id: Option<String>,
    owner_mode: crate::session_control::SessionOwnerMode,
    viewport: crate::session_control::SessionViewport,
    automation_delegate: Option<crate::session_control::SessionAutomationDelegate>,
    idle_timeout_sec: Option<u32>,
    labels: Vec<(String, String)>,
    extensions: Vec<AdminAppliedExtensionSummary>,
    recording: crate::session_control::SessionRecordingPolicy,
    runtime: crate::session_control::SessionRuntimeInfo,
    status: crate::session_control::SessionStatusSummary,
    created_at: chrono::DateTime<Utc>,
    runtime_released_at: Option<chrono::DateTime<Utc>>,
    stopped_at: Option<chrono::DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
struct AdminAppliedExtensionSummary {
    extension_id: Uuid,
    extension_version_id: Uuid,
    name: String,
    version: String,
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

#[derive(Debug, Serialize)]
pub(super) struct AdminMcpDelegationSnapshotEvent {
    event_type: &'static str,
    sequence: u64,
    created_at: chrono::DateTime<Utc>,
    mcp_delegations: Vec<AdminMcpDelegationSummary>,
}

#[derive(Debug, Serialize)]
struct AdminMcpDelegationSummary {
    session_id: Uuid,
    delegated_client_id: Option<String>,
    delegated_issuer: Option<String>,
    mcp_owner: bool,
    updated_at: chrono::DateTime<Utc>,
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
    let change_summaries = resources
        .iter()
        .map(session_snapshot_change_summary)
        .collect::<Vec<_>>();
    let change_key = serialized_change_key(&change_summaries)?;
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

fn session_snapshot_change_summary(session: &SessionResource) -> AdminSessionSnapshotChangeSummary {
    let mut labels = session
        .labels
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<Vec<_>>();
    labels.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));

    let mut extensions = session
        .extensions
        .iter()
        .map(|extension| AdminAppliedExtensionSummary {
            extension_id: extension.extension_id,
            extension_version_id: extension.extension_version_id,
            name: extension.name.clone(),
            version: extension.version.clone(),
        })
        .collect::<Vec<_>>();
    extensions.sort_by(|left, right| {
        left.extension_id
            .cmp(&right.extension_id)
            .then_with(|| left.extension_version_id.cmp(&right.extension_version_id))
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.version.cmp(&right.version))
    });

    AdminSessionSnapshotChangeSummary {
        id: session.id,
        state: session.state,
        template_id: session.template_id.clone(),
        owner_mode: session.owner_mode,
        viewport: session.viewport.clone(),
        automation_delegate: session.automation_delegate.clone(),
        idle_timeout_sec: session.idle_timeout_sec,
        labels,
        extensions,
        recording: session.recording.clone(),
        runtime: session.runtime.clone(),
        status: session.status.clone(),
        created_at: session.created_at,
        runtime_released_at: session.runtime_released_at,
        stopped_at: session.stopped_at,
    }
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

pub(super) async fn build_mcp_delegation_snapshot(
    state: &ApiState,
    principal: &AuthenticatedPrincipal,
    sequence: u64,
) -> Result<
    AdminChangedEvent<AdminMcpDelegationSnapshotEvent>,
    crate::session_control::SessionStoreError,
> {
    let mut summaries = Vec::new();
    for session in state
        .session_store
        .list_sessions_for_owner(principal)
        .await?
    {
        let telemetry = state.registry.telemetry_snapshot_if_live(session.id).await;
        summaries.push(AdminMcpDelegationSummary {
            session_id: session.id,
            delegated_client_id: session
                .automation_delegate
                .as_ref()
                .map(|delegate| delegate.client_id.clone()),
            delegated_issuer: session
                .automation_delegate
                .as_ref()
                .map(|delegate| delegate.issuer.clone()),
            mcp_owner: telemetry
                .map(|snapshot| snapshot.mcp_owner)
                .unwrap_or(false),
            updated_at: session.updated_at,
        });
    }
    summaries.sort_by_key(|summary| summary.session_id);
    let change_key = serialized_change_key(&summaries)?;
    Ok(AdminChangedEvent {
        event: AdminMcpDelegationSnapshotEvent {
            event_type: MCP_DELEGATION_SNAPSHOT_EVENT_TYPE,
            sequence,
            created_at: Utc::now(),
            mcp_delegations: summaries,
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
    use crate::session_control::{
        SessionCapabilities, SessionConnectInfo, SessionConnectionCounts, SessionIdleStatus,
        SessionOwner, SessionPresenceState, SessionRuntimeInfo, SessionRuntimeResumeMode,
        SessionRuntimeState, SessionStatusSummary, SessionStopEligibility, SessionViewport,
    };
    use std::collections::HashMap;

    #[test]
    fn session_snapshot_change_key_uses_stable_summary_for_unchanged_resources() {
        let first = session_resource_fixture(HashMap::from([
            ("b".to_string(), "2".to_string()),
            ("a".to_string(), "1".to_string()),
        ]));
        let mut second = session_resource_fixture(HashMap::from([
            ("a".to_string(), "1".to_string()),
            ("b".to_string(), "2".to_string()),
        ]));
        second.created_at = first.created_at;
        second.updated_at = second.updated_at + chrono::Duration::seconds(1);
        let first_key =
            serialized_change_key(&vec![session_snapshot_change_summary(&first)]).unwrap();
        let second_key =
            serialized_change_key(&vec![session_snapshot_change_summary(&second)]).unwrap();

        assert_eq!(first_key, second_key);
    }

    #[test]
    fn session_snapshot_change_key_tracks_session_state() {
        let ready = session_resource_fixture(HashMap::new());
        let mut active = ready.clone();
        active.state = SessionLifecycleState::Active;
        let ready_key =
            serialized_change_key(&vec![session_snapshot_change_summary(&ready)]).unwrap();
        let active_key =
            serialized_change_key(&vec![session_snapshot_change_summary(&active)]).unwrap();

        assert_ne!(ready_key, active_key);
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

    #[test]
    fn mcp_delegation_snapshot_change_key_tracks_delegate() {
        let session_id = Uuid::nil();
        let updated_at = Utc::now();
        let empty_key = serialized_change_key(&vec![AdminMcpDelegationSummary {
            session_id,
            delegated_client_id: None,
            delegated_issuer: None,
            mcp_owner: false,
            updated_at,
        }])
        .unwrap();
        let delegated_key = serialized_change_key(&vec![AdminMcpDelegationSummary {
            session_id,
            delegated_client_id: Some("bpane-mcp-bridge".to_string()),
            delegated_issuer: Some("local-compose".to_string()),
            mcp_owner: false,
            updated_at,
        }])
        .unwrap();

        assert_ne!(empty_key, delegated_key);
    }

    fn session_resource_fixture(labels: HashMap<String, String>) -> SessionResource {
        let now = Utc::now();
        SessionResource {
            id: Uuid::nil(),
            state: SessionLifecycleState::Ready,
            project_id: None,
            project: None,
            admission: crate::session_control::ProjectAdmissionDecision::owner_scope_unbounded(now),
            template_id: None,
            browser_context: crate::session_control::SessionBrowserContextResource {
                mode: crate::session_control::SessionBrowserContextMode::Fresh,
                context_id: None,
            },
            network_identity: crate::session_control::SessionNetworkIdentity::default(),
            effective_egress: crate::session_control::SessionEffectiveEgress::default(),
            egress_diagnostics: crate::session_control::EgressDiagnosticsResource::direct(
                None, None, now,
            ),
            owner_mode: SessionOwnerMode::Collaborative,
            viewport: SessionViewport {
                width: 1600,
                height: 900,
            },
            capabilities: SessionCapabilities::default(),
            owner: SessionOwner {
                subject: "owner".to_string(),
                issuer: "issuer".to_string(),
                display_name: None,
            },
            automation_delegate: None,
            idle_timeout_sec: Some(300),
            labels,
            integration_context: None,
            extensions: Vec::new(),
            recording: SessionRecordingPolicy::default(),
            connect: SessionConnectInfo {
                gateway_url: "https://gateway.example".to_string(),
                transport_path: "/session".to_string(),
                auth_type: "session_connect_ticket".to_string(),
                ticket_path: Some("/api/v1/sessions/000/access-tokens".to_string()),
                compatibility_mode: "session_runtime_pool".to_string(),
            },
            runtime: SessionRuntimeInfo {
                binding: "docker_pool".to_string(),
                compatibility_mode: "session_runtime_pool".to_string(),
                cdp_endpoint: None,
            },
            status: SessionStatusSummary {
                runtime_state: SessionRuntimeState::NotStarted,
                runtime_resume_mode: SessionRuntimeResumeMode::FreshStart,
                presence_state: SessionPresenceState::Empty,
                connection_counts: SessionConnectionCounts::default(),
                stop_eligibility: SessionStopEligibility::default(),
                idle: SessionIdleStatus {
                    idle_timeout_sec: Some(300),
                    idle_since: None,
                    idle_deadline: None,
                },
            },
            created_at: now,
            updated_at: now,
            runtime_released_at: None,
            stopped_at: None,
        }
    }
}
