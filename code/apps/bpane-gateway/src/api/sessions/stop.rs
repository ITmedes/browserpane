use axum::response::{IntoResponse, Response};
use tracing::info;

use super::super::*;
use crate::session_control::{SessionStopBlockerKind, SessionStopEligibility};

pub(super) async fn stop_session(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    stop_session_for_owner(&state, &principal, session_id).await
}

pub(super) async fn delete_session(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    stop_session_for_owner(&state, &principal, session_id).await
}

pub(super) async fn stop_session_for_owner(
    state: &Arc<ApiState>,
    principal: &AuthenticatedPrincipal,
    session_id: Uuid,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let stored = state
        .session_store
        .get_session_for_owner(principal, session_id)
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
        let resource = session_resource(state, &stored, None)
            .await
            .map_err(map_session_store_error)?;
        return Ok((StatusCode::OK, Json(resource)).into_response());
    }

    let status = session_status_summary(state, &stored)
        .await
        .map_err(map_session_store_error)?;
    if !status.stop_eligibility.allowed {
        let session = session_resource(state, &stored, None)
            .await
            .map_err(map_session_store_error)?;
        return Ok((
            StatusCode::CONFLICT,
            Json(SessionStopConflictResponse {
                error: session_stop_conflict_message(&status.stop_eligibility),
                session,
            }),
        )
            .into_response());
    }

    let stopped = state
        .session_store
        .stop_session_for_owner(principal, session_id)
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

    if let Err(error) = state
        .recording_lifecycle
        .request_stop_and_wait(session_id, SessionRecordingTerminationReason::SessionStop)
        .await
    {
        info!(%session_id, "recording finalization before session stop returned: {error}");
    }
    state.session_manager.release(session_id).await;
    state.registry.remove_session(session_id).await;

    let resource = session_resource(state, &stopped, None)
        .await
        .map_err(map_session_store_error)?;
    Ok((StatusCode::OK, Json(resource)).into_response())
}

fn session_stop_conflict_message(stop_eligibility: &SessionStopEligibility) -> String {
    let blockers = stop_eligibility
        .blockers
        .iter()
        .map(|blocker| match blocker.kind {
            SessionStopBlockerKind::OwnerClients => {
                format!("{} owner client(s)", blocker.count)
            }
            SessionStopBlockerKind::ViewerClients => {
                format!("{} viewer client(s)", blocker.count)
            }
            SessionStopBlockerKind::RecorderClients => {
                format!("{} recorder client(s)", blocker.count)
            }
            SessionStopBlockerKind::AutomationOwner => {
                format!("{} automation owner(s)", blocker.count)
            }
            SessionStopBlockerKind::RecordingActivity => {
                format!("{} active recording operation(s)", blocker.count)
            }
            SessionStopBlockerKind::AutomationTasks => {
                format!("{} active automation task(s)", blocker.count)
            }
            SessionStopBlockerKind::WorkflowRuns => {
                format!("{} active workflow run(s)", blocker.count)
            }
        })
        .collect::<Vec<_>>()
        .join(", ");

    format!("session cannot be stopped while blockers remain: {blockers}")
}
