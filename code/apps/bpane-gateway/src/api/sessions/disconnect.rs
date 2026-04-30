use tracing::info;

use super::super::*;
use crate::session_hub::SessionTerminationReason;

pub(super) async fn disconnect_session_connection(
    headers: HeaderMap,
    Path((session_id, connection_id)): Path<(Uuid, u64)>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionStatus>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let session = load_owned_session(&state, &principal, session_id).await?;
    let disconnected = state
        .registry
        .disconnect_session_client(
            session_id,
            connection_id,
            SessionTerminationReason::DisconnectedByOwner,
        )
        .await;
    if !disconnected {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("connection {connection_id} was not found for session {session_id}"),
            }),
        ));
    }
    info!(%session_id, connection_id, "disconnected session connection");
    let updated = reconcile_session_after_disconnect(&state, session).await?;
    Ok(Json(
        load_session_status(&state, &updated)
            .await
            .map_err(map_session_store_error)?,
    ))
}

pub(super) async fn disconnect_all_session_connections(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionStatus>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let session = load_owned_session(&state, &principal, session_id).await?;
    let disconnected = state
        .registry
        .disconnect_all_session_clients(session_id, SessionTerminationReason::DisconnectedByOwner)
        .await;
    info!(%session_id, disconnected, "disconnected all live session connections");
    let updated = reconcile_session_after_disconnect(&state, session).await?;
    Ok(Json(
        load_session_status(&state, &updated)
            .await
            .map_err(map_session_store_error)?,
    ))
}

async fn load_owned_session(
    state: &Arc<ApiState>,
    principal: &AuthenticatedPrincipal,
    session_id: Uuid,
) -> Result<StoredSession, (StatusCode, Json<ErrorResponse>)> {
    state
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
        })
}

async fn reconcile_session_after_disconnect(
    state: &Arc<ApiState>,
    session: StoredSession,
) -> Result<StoredSession, (StatusCode, Json<ErrorResponse>)> {
    let snapshot = state
        .registry
        .telemetry_snapshot_if_live(session.id)
        .await
        .unwrap_or_else(|| state.registry.empty_telemetry_snapshot());

    if snapshot.browser_clients == 0 && !snapshot.mcp_owner && session.state.is_runtime_candidate()
    {
        if let Some(idle) = state
            .session_store
            .mark_session_idle(session.id)
            .await
            .map_err(map_session_store_error)?
        {
            state.session_manager.mark_session_idle(session.id).await;
            schedule_idle_session_stop(
                session.id,
                state.idle_stop_timeout,
                state.registry.clone(),
                state.session_store.clone(),
                state.session_manager.clone(),
                state.recording_lifecycle.clone(),
            );
            return Ok(idle);
        }
    }

    state
        .session_store
        .get_session_by_id(session.id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("session {} not found", session.id),
                }),
            )
        })
}
