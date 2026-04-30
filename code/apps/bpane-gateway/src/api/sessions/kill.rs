use tracing::{info, warn};

use super::super::*;
use crate::automation_tasks::{AutomationTaskState, AutomationTaskTransitionRequest};
use crate::session_control::SessionRecordingTerminationReason;
use crate::session_hub::SessionTerminationReason;
use crate::workflow::{WorkflowRunState, WorkflowRunTransitionRequest};

pub(super) async fn kill_session(
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

    if stored.state == SessionLifecycleState::Stopped {
        return Ok(Json(
            session_resource(&state, &stored, None)
                .await
                .map_err(map_session_store_error)?,
        ));
    }

    terminate_session_workloads(&state, &stored).await?;

    if let Err(error) = state
        .recording_lifecycle
        .request_stop_and_wait(session_id, SessionRecordingTerminationReason::SessionKill)
        .await
    {
        warn!(%session_id, "recording finalization before session kill returned: {error}");
    }

    let terminated_clients = state
        .registry
        .terminate_session_clients(session_id, SessionTerminationReason::SessionKilled)
        .await;

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

    state.session_manager.release(session_id).await;
    state.registry.remove_session(session_id).await;
    info!(
        %session_id,
        terminated_clients,
        "force killed session and released runtime"
    );

    Ok(Json(
        session_resource(&state, &stopped, None)
            .await
            .map_err(map_session_store_error)?,
    ))
}

async fn terminate_session_workloads(
    state: &Arc<ApiState>,
    session: &StoredSession,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    let owner = AuthenticatedPrincipal {
        subject: session.owner.subject.clone(),
        issuer: session.owner.issuer.clone(),
        display_name: session.owner.display_name.clone(),
        client_id: None,
    };
    let active_runs = state
        .session_store
        .list_workflow_runs_for_owner(&owner)
        .await
        .map_err(map_session_store_error)?
        .into_iter()
        .filter(|run| run.session_id == session.id && !run.state.is_terminal())
        .collect::<Vec<_>>();
    let workflow_task_ids = active_runs
        .iter()
        .map(|run| run.automation_task_id)
        .collect::<std::collections::HashSet<_>>();

    for run in active_runs {
        if let Err(error) = state
            .session_store
            .transition_workflow_run(
                run.id,
                WorkflowRunTransitionRequest {
                    state: WorkflowRunState::Cancelled,
                    output: run.output.clone(),
                    error: None,
                    artifact_refs: run.artifact_refs.clone(),
                    message: Some(
                        "workflow run cancelled because its session was force killed".to_string(),
                    ),
                    data: Some(serde_json::json!({
                        "reason": "session_kill",
                        "session_id": session.id,
                    })),
                },
            )
            .await
        {
            warn!(run_id = %run.id, session_id = %session.id, "failed to transition workflow run during session kill: {error}");
        }
        if let Err(error) = state.workflow_lifecycle.cancel_run(run.id).await {
            warn!(run_id = %run.id, session_id = %session.id, "failed to stop workflow worker during session kill: {error}");
        }
    }

    let active_tasks = state
        .session_store
        .list_automation_tasks_for_owner(&owner)
        .await
        .map_err(map_session_store_error)?
        .into_iter()
        .filter(|task| {
            task.session_id == session.id
                && !task.state.is_terminal()
                && !workflow_task_ids.contains(&task.id)
        })
        .collect::<Vec<_>>();
    for task in active_tasks {
        if let Err(error) = state
            .session_store
            .transition_automation_task(
                task.id,
                AutomationTaskTransitionRequest {
                    state: AutomationTaskState::Cancelled,
                    output: task.output.clone(),
                    error: None,
                    artifact_refs: task.artifact_refs.clone(),
                    event_type: "automation_task.cancelled".to_string(),
                    event_message: "automation task cancelled because its session was force killed"
                        .to_string(),
                    event_data: Some(serde_json::json!({
                        "reason": "session_kill",
                        "session_id": session.id,
                    })),
                },
            )
            .await
        {
            warn!(task_id = %task.id, session_id = %session.id, "failed to transition automation task during session kill: {error}");
        }
    }

    Ok(())
}
