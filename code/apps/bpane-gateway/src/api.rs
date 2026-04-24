use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::Response;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing::info;
use uuid::Uuid;

use crate::auth::{AuthValidator, AuthenticatedPrincipal};
use crate::automation_access_token::{
    SessionAutomationAccessTokenClaims, SessionAutomationAccessTokenManager,
};
use crate::connect_ticket::SessionConnectTicketManager;
use crate::idle_stop::schedule_idle_session_stop;
use crate::recording_artifact_store::{
    FinalizeRecordingArtifactRequest, RecordingArtifactStore, RecordingArtifactStoreError,
};
use crate::recording_lifecycle::{RecordingLifecycleError, RecordingLifecycleManager};
use crate::recording_observability::{RecordingObservability, RecordingObservabilitySnapshot};
use crate::recording_playback::{
    prepare_session_recording_playback, PreparedSessionRecordingPlayback, RecordingPlaybackError,
    SessionRecordingPlaybackManifest, SessionRecordingPlaybackResource,
};
use crate::session_control::{
    CompleteSessionRecordingRequest, CreateSessionRequest, FailSessionRecordingRequest,
    PersistCompletedSessionRecordingRequest, SessionLifecycleState, SessionListResponse,
    SessionOwnerMode, SessionRecordingFormat, SessionRecordingListResponse, SessionRecordingMode,
    SessionRecordingPolicy, SessionRecordingResource, SessionRecordingState,
    SessionRecordingTerminationReason, SessionResource, SessionStore, SessionStoreError,
    SetAutomationDelegateRequest, StoredSession, StoredSessionRecording,
};
use crate::session_hub::SessionTelemetrySnapshot;
use crate::session_manager::{SessionManager, SessionManagerError, SessionRuntime};
use crate::session_registry::SessionRegistry;

/// Shared state for the HTTP API.
struct ApiState {
    registry: Arc<SessionRegistry>,
    auth_validator: Arc<AuthValidator>,
    connect_ticket_manager: Arc<SessionConnectTicketManager>,
    automation_access_token_manager: Arc<SessionAutomationAccessTokenManager>,
    session_store: SessionStore,
    session_manager: Arc<SessionManager>,
    recording_artifact_store: Arc<RecordingArtifactStore>,
    recording_observability: Arc<RecordingObservability>,
    recording_lifecycle: Arc<RecordingLifecycleManager>,
    idle_stop_timeout: std::time::Duration,
    public_gateway_url: String,
    default_owner_mode: SessionOwnerMode,
}

const AUTOMATION_ACCESS_TOKEN_HEADER: &str = "x-bpane-automation-access-token";

#[derive(Serialize)]
struct SessionStatus {
    browser_clients: u32,
    viewer_clients: u32,
    recorder_clients: u32,
    max_viewers: u32,
    viewer_slots_remaining: u32,
    exclusive_browser_owner: bool,
    mcp_owner: bool,
    resolution: (u16, u16),
    recording: SessionRecordingStatus,
    playback: SessionRecordingPlaybackResource,
    telemetry: SessionTelemetry,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum SessionRecordingStatusState {
    Disabled,
    Idle,
    Recording,
    Finalizing,
    Ready,
    Failed,
}

#[derive(Serialize)]
struct SessionRecordingStatus {
    configured_mode: SessionRecordingMode,
    format: SessionRecordingFormat,
    retention_sec: Option<u32>,
    state: SessionRecordingStatusState,
    active_recording_id: Option<String>,
    recorder_attached: bool,
    started_at: Option<chrono::DateTime<chrono::Utc>>,
    bytes_written: Option<u64>,
    duration_ms: Option<u64>,
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

#[derive(Serialize)]
struct SessionAutomationAccessInfo {
    endpoint_url: String,
    protocol: String,
    auth_type: String,
    auth_header: String,
    status_path: String,
    mcp_owner_path: String,
    compatibility_mode: String,
}

#[derive(Serialize)]
struct SessionAutomationAccessResponse {
    session_id: Uuid,
    token_type: String,
    token: String,
    expires_at: chrono::DateTime<chrono::Utc>,
    automation: SessionAutomationAccessInfo,
}

async fn create_session(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CreateSessionRequest>,
) -> Result<(StatusCode, Json<SessionResource>), (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    state
        .recording_lifecycle
        .validate_mode(request.recording.mode)
        .map_err(map_recording_lifecycle_error)?;
    let owner_mode = resolve_owner_mode(&state, request.owner_mode)?;
    let stored = state
        .session_store
        .create_session(&principal, request, owner_mode)
        .await
        .map_err(map_session_store_error)?;
    if let Err(error) = state
        .recording_lifecycle
        .ensure_auto_recording(&stored)
        .await
    {
        let _ = state
            .session_store
            .stop_session_for_owner(&principal, stored.id)
            .await;
        state.session_manager.release(stored.id).await;
        state.registry.remove_session(stored.id).await;
        return Err(map_recording_lifecycle_error(error));
    }

    schedule_idle_session_stop(
        stored.id,
        state.idle_stop_timeout,
        state.registry.clone(),
        state.session_store.clone(),
        state.session_manager.clone(),
        state.recording_lifecycle.clone(),
    );

    Ok((
        StatusCode::CREATED,
        Json(session_resource(&state, &stored, None)),
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
        .map(|session| session_resource(&state, &session, None))
        .collect();

    Ok(Json(SessionListResponse { sessions }))
}

async fn get_session(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionResource>, (StatusCode, Json<ErrorResponse>)> {
    let stored = authorize_visible_session_request(&headers, &state, session_id).await?;

    Ok(Json(session_resource(&state, &stored, None)))
}

async fn list_session_recordings(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionRecordingListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let _session = authorize_visible_session_request(&headers, &state, session_id).await?;
    let recordings = state
        .session_store
        .list_recordings_for_session(session_id)
        .await
        .map_err(map_session_store_error)?
        .into_iter()
        .map(|recording| recording.to_resource())
        .collect();

    Ok(Json(SessionRecordingListResponse { recordings }))
}

async fn create_session_recording(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<(StatusCode, Json<SessionRecordingResource>), (StatusCode, Json<ErrorResponse>)> {
    let session = authorize_runtime_session_request(&headers, &state, session_id).await?;
    if session.recording.mode == SessionRecordingMode::Disabled {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!("recording is disabled for session {session_id}"),
            }),
        ));
    }

    let recording = state
        .session_store
        .create_recording_for_session(session_id, session.recording.format, None)
        .await
        .map_err(map_session_store_error)?;

    Ok((StatusCode::CREATED, Json(recording.to_resource())))
}

async fn get_session_recording(
    headers: HeaderMap,
    Path((session_id, recording_id)): Path<(Uuid, Uuid)>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionRecordingResource>, (StatusCode, Json<ErrorResponse>)> {
    let _session = authorize_visible_session_request(&headers, &state, session_id).await?;
    let recording = load_session_recording(&state, session_id, recording_id).await?;
    Ok(Json(recording.to_resource()))
}

async fn stop_session_recording(
    headers: HeaderMap,
    Path((session_id, recording_id)): Path<(Uuid, Uuid)>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionRecordingResource>, (StatusCode, Json<ErrorResponse>)> {
    let _session = authorize_runtime_session_request(&headers, &state, session_id).await?;
    let recording = state
        .session_store
        .stop_recording_for_session(
            session_id,
            recording_id,
            SessionRecordingTerminationReason::ManualStop,
        )
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!(
                        "recording {recording_id} was not found for session {session_id}"
                    ),
                }),
            )
        })?;
    Ok(Json(recording.to_resource()))
}

async fn complete_session_recording(
    headers: HeaderMap,
    Path((session_id, recording_id)): Path<(Uuid, Uuid)>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CompleteSessionRecordingRequest>,
) -> Result<Json<SessionRecordingResource>, (StatusCode, Json<ErrorResponse>)> {
    let _session = authorize_visible_session_request(&headers, &state, session_id).await?;
    let recording = load_session_recording(&state, session_id, recording_id).await?;
    let CompleteSessionRecordingRequest {
        source_path,
        mime_type,
        bytes,
        duration_ms,
    } = request;
    state
        .recording_observability
        .record_artifact_finalize_request();
    let stored_artifact = state
        .recording_artifact_store
        .finalize(FinalizeRecordingArtifactRequest {
            session_id,
            recording_id,
            format: recording.format,
            source_path,
        })
        .await
        .map_err(|error| {
            state
                .recording_observability
                .record_artifact_finalize_failure();
            map_recording_artifact_store_error(error)
        })?;
    let recording = state
        .session_store
        .complete_recording_for_session(
            session_id,
            recording_id,
            PersistCompletedSessionRecordingRequest {
                artifact_ref: stored_artifact.artifact_ref.clone(),
                mime_type,
                bytes,
                duration_ms,
            },
        )
        .await
        .map_err(|error| {
            let artifact_store = state.recording_artifact_store.clone();
            let artifact_ref = stored_artifact.artifact_ref.clone();
            tokio::spawn(async move {
                let _ = artifact_store.delete(&artifact_ref).await;
            });
            state
                .recording_observability
                .record_artifact_finalize_failure();
            map_session_store_error(error)
        })?
        .ok_or_else(|| {
            let artifact_store = state.recording_artifact_store.clone();
            let artifact_ref = stored_artifact.artifact_ref.clone();
            tokio::spawn(async move {
                let _ = artifact_store.delete(&artifact_ref).await;
            });
            state
                .recording_observability
                .record_artifact_finalize_failure();
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!(
                        "recording {recording_id} was not found for session {session_id}"
                    ),
                }),
            )
        })?;
    state
        .recording_observability
        .record_artifact_finalize_success();
    Ok(Json(recording.to_resource()))
}

async fn fail_session_recording(
    headers: HeaderMap,
    Path((session_id, recording_id)): Path<(Uuid, Uuid)>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<FailSessionRecordingRequest>,
) -> Result<Json<SessionRecordingResource>, (StatusCode, Json<ErrorResponse>)> {
    let _session = authorize_visible_session_request(&headers, &state, session_id).await?;
    let recording = state
        .session_store
        .fail_recording_for_session(session_id, recording_id, request)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!(
                        "recording {recording_id} was not found for session {session_id}"
                    ),
                }),
            )
        })?;
    state.recording_observability.record_recording_failure();
    Ok(Json(recording.to_resource()))
}

async fn get_session_recording_content(
    headers: HeaderMap,
    Path((session_id, recording_id)): Path<(Uuid, Uuid)>,
    State(state): State<Arc<ApiState>>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let _session = authorize_visible_session_request(&headers, &state, session_id).await?;
    let recording = load_session_recording(&state, session_id, recording_id).await?;
    let artifact_ref = recording.artifact_ref.as_ref().ok_or_else(|| {
        if recording.state.is_terminal() {
            (
                StatusCode::GONE,
                Json(ErrorResponse {
                    error: format!("recording artifact for {recording_id} is no longer available"),
                }),
            )
        } else {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("recording {recording_id} does not have an artifact yet"),
                }),
            )
        }
    })?;
    let bytes = state
        .recording_artifact_store
        .read(artifact_ref)
        .await
        .map_err(|error| match error.io_kind() {
            Some(std::io::ErrorKind::NotFound) => (
                StatusCode::GONE,
                Json(ErrorResponse {
                    error: format!("recording artifact for {recording_id} is no longer available"),
                }),
            ),
            _ => (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: format!("failed to read recording artifact: {error}"),
                }),
            ),
        })?;

    let filename = format!("browserpane-{session_id}-{recording_id}.webm");
    let mime_type = recording
        .mime_type
        .as_deref()
        .unwrap_or(recording_mime_type(recording.format));

    let mut response = Response::new(axum::body::Body::from(bytes.clone()));
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_str(mime_type).map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("failed to encode content type header: {error}"),
                }),
            )
        })?,
    );
    response.headers_mut().insert(
        CONTENT_LENGTH,
        HeaderValue::from_str(&bytes.len().to_string()).map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("failed to encode content length header: {error}"),
                }),
            )
        })?,
    );
    response.headers_mut().insert(
        CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{filename}\"")).map_err(
            |error| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("failed to encode content disposition header: {error}"),
                    }),
                )
            },
        )?,
    );
    Ok(response)
}

async fn get_session_recording_playback(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionRecordingPlaybackResource>, (StatusCode, Json<ErrorResponse>)> {
    let _session = authorize_visible_session_request(&headers, &state, session_id).await?;
    let playback = load_session_recording_playback(&state, session_id).await?;
    Ok(Json(playback.resource))
}

async fn get_session_recording_playback_manifest(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionRecordingPlaybackManifest>, (StatusCode, Json<ErrorResponse>)> {
    let _session = authorize_visible_session_request(&headers, &state, session_id).await?;
    state
        .recording_observability
        .record_playback_manifest_request();
    let playback = load_session_recording_playback(&state, session_id).await?;
    Ok(Json(playback.manifest))
}

async fn get_session_recording_playback_export(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let _session = authorize_visible_session_request(&headers, &state, session_id).await?;
    state
        .recording_observability
        .record_playback_export_request();
    let playback = load_session_recording_playback(&state, session_id).await?;
    let bytes = playback
        .export_bundle(&state.recording_artifact_store)
        .await
        .map_err(|error| {
            state
                .recording_observability
                .record_playback_export_failure();
            map_recording_playback_error(error)
        })?;
    state
        .recording_observability
        .record_playback_export_success(bytes.len() as u64, Utc::now())
        .await;

    let filename = format!("browserpane-{session_id}-recording-playback.zip");
    let mut response = Response::new(axum::body::Body::from(bytes.clone()));
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/zip"));
    response.headers_mut().insert(
        CONTENT_LENGTH,
        HeaderValue::from_str(&bytes.len().to_string()).map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("failed to encode content length header: {error}"),
                }),
            )
        })?,
    );
    response.headers_mut().insert(
        CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{filename}\"")).map_err(
            |error| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("failed to encode content disposition header: {error}"),
                    }),
                )
            },
        )?,
    );
    Ok(response)
}

async fn get_recording_operations(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<RecordingObservabilitySnapshot>, (StatusCode, Json<ErrorResponse>)> {
    authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    Ok(Json(state.recording_observability.snapshot().await))
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

    Ok(Json(session_resource(&state, &stored, None)))
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

    Ok(Json(session_resource(&state, &stored, None)))
}

async fn issue_session_access_token(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionAccessTokenResponse>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let connectable = prepare_runtime_access_session(&state, &principal, session_id).await?;

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
    let resource = session_resource(&state, &connectable, None);

    Ok(Json(SessionAccessTokenResponse {
        session_id,
        token_type: "session_connect_ticket".to_string(),
        token: issued.token,
        expires_at: issued.expires_at,
        connect: resource.connect,
    }))
}

async fn issue_session_automation_access(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionAutomationAccessResponse>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let connectable = prepare_runtime_access_session(&state, &principal, session_id).await?;
    resolve_runtime(&state, session_id).await?;
    let resource = session_resource(&state, &connectable, None);
    let endpoint_url = resource.runtime.cdp_endpoint.ok_or_else(|| {
        (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!(
                    "session {session_id} does not expose an automation endpoint for the current runtime"
                ),
            }),
        )
    })?;
    let issued = state
        .automation_access_token_manager
        .issue_token(session_id, &principal)
        .map_err(|error| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: format!("failed to issue session automation access token: {error}"),
                }),
            )
        })?;

    Ok(Json(SessionAutomationAccessResponse {
        session_id,
        token_type: "session_automation_access_token".to_string(),
        token: issued.token,
        expires_at: issued.expires_at,
        automation: SessionAutomationAccessInfo {
            endpoint_url,
            protocol: "chrome_devtools_protocol".to_string(),
            auth_type: "session_automation_access_token".to_string(),
            auth_header: AUTOMATION_ACCESS_TOKEN_HEADER.to_string(),
            status_path: format!("/api/v1/sessions/{session_id}/status"),
            mcp_owner_path: format!("/api/v1/sessions/{session_id}/mcp-owner"),
            compatibility_mode: resource.connect.compatibility_mode,
        },
    }))
}

async fn get_session_status(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionStatus>, (StatusCode, Json<ErrorResponse>)> {
    let session =
        authorize_runtime_session_request_with_automation_access(&headers, &state, session_id)
            .await?;
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
    let recordings = state
        .session_store
        .list_recordings_for_session(session_id)
        .await
        .map_err(map_session_store_error)?;
    let latest_recording = latest_recording(&recordings);
    let playback = prepare_session_recording_playback(session_id, &recordings, Utc::now());

    Ok(Json(session_status_from_snapshot(
        snapshot,
        &session.recording,
        latest_recording,
        playback.resource,
    )))
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

    if should_block_session_stop(
        stored.state,
        state
            .session_manager
            .profile()
            .supports_legacy_global_routes,
        runtime_is_currently_in_use(&state).await,
    ) {
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

    if let Err(error) = state
        .recording_lifecycle
        .request_stop_and_wait(session_id, SessionRecordingTerminationReason::SessionStop)
        .await
    {
        info!(%session_id, "recording finalization before session stop returned: {error}");
    }
    state.session_manager.release(session_id).await;
    state.registry.remove_session(session_id).await;

    Ok(Json(session_resource(&state, &stopped, None)))
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
    let session = state
        .session_store
        .get_session_by_id(session_id)
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
    let recordings = state
        .session_store
        .list_recordings_for_session(session_id)
        .await
        .map_err(map_session_store_error)?;
    let latest_recording = latest_recording(&recordings);
    let playback = prepare_session_recording_playback(session_id, &recordings, Utc::now());

    Ok(Json(session_status_from_snapshot(
        snapshot,
        &session.recording,
        latest_recording,
        playback.resource,
    )))
}

fn session_status_from_snapshot(
    snapshot: SessionTelemetrySnapshot,
    recording_policy: &SessionRecordingPolicy,
    latest_recording: Option<&StoredSessionRecording>,
    playback: SessionRecordingPlaybackResource,
) -> SessionStatus {
    SessionStatus {
        browser_clients: snapshot.browser_clients,
        viewer_clients: snapshot.viewer_clients,
        recorder_clients: snapshot.recorder_clients,
        max_viewers: snapshot.max_viewers,
        viewer_slots_remaining: snapshot.viewer_slots_remaining,
        exclusive_browser_owner: snapshot.exclusive_browser_owner,
        mcp_owner: snapshot.mcp_owner,
        resolution: snapshot.resolution,
        recording: recording_status_from_snapshot(snapshot, recording_policy, latest_recording),
        playback,
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

fn recording_status_from_snapshot(
    snapshot: SessionTelemetrySnapshot,
    recording_policy: &SessionRecordingPolicy,
    latest_recording: Option<&StoredSessionRecording>,
) -> SessionRecordingStatus {
    let active_recording_id = latest_recording
        .filter(|recording| recording.state.is_active())
        .map(|recording| recording.id.to_string());
    let state = if let Some(recording) = latest_recording {
        match recording.state {
            SessionRecordingState::Starting | SessionRecordingState::Recording => {
                SessionRecordingStatusState::Recording
            }
            SessionRecordingState::Finalizing => SessionRecordingStatusState::Finalizing,
            SessionRecordingState::Ready => SessionRecordingStatusState::Ready,
            SessionRecordingState::Failed => SessionRecordingStatusState::Failed,
        }
    } else if recording_policy.mode == SessionRecordingMode::Disabled {
        SessionRecordingStatusState::Disabled
    } else if snapshot.recorder_clients > 0 {
        SessionRecordingStatusState::Recording
    } else {
        SessionRecordingStatusState::Idle
    };

    SessionRecordingStatus {
        configured_mode: recording_policy.mode,
        format: recording_policy.format,
        retention_sec: recording_policy.retention_sec,
        state,
        active_recording_id,
        recorder_attached: snapshot.recorder_clients > 0,
        started_at: latest_recording.map(|recording| recording.started_at),
        bytes_written: latest_recording.and_then(|recording| recording.bytes),
        duration_ms: latest_recording.and_then(|recording| recording.duration_ms),
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
    state.session_manager.mark_session_active(session_id).await;
    let _ = state.session_store.mark_session_active(session_id).await;

    Ok(Json(OkResponse { ok: true }))
}

async fn set_session_mcp_owner(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(req): Json<McpOwnerRequest>,
) -> Result<Json<OkResponse>, (StatusCode, Json<ErrorResponse>)> {
    let _session =
        authorize_runtime_session_request_with_automation_access(&headers, &state, session_id)
            .await?;
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
    state.session_manager.mark_session_active(session_id).await;
    let _ = state.session_store.mark_session_active(session_id).await;

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
        .session_manager
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
        let _ = state.session_store.mark_session_idle(session_id).await;
        state.session_manager.mark_session_idle(session_id).await;
        schedule_idle_session_stop(
            session_id,
            state.idle_stop_timeout,
            state.registry.clone(),
            state.session_store.clone(),
            state.session_manager.clone(),
            state.recording_lifecycle.clone(),
        );
    }

    Ok(Json(OkResponse { ok: true }))
}

async fn clear_session_mcp_owner(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<OkResponse>, (StatusCode, Json<ErrorResponse>)> {
    let _session =
        authorize_runtime_session_request_with_automation_access(&headers, &state, session_id)
            .await?;
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
        let _ = state.session_store.mark_session_idle(session_id).await;
        state.session_manager.mark_session_idle(session_id).await;
        schedule_idle_session_stop(
            session_id,
            state.idle_stop_timeout,
            state.registry.clone(),
            state.session_store.clone(),
            state.session_manager.clone(),
            state.recording_lifecycle.clone(),
        );
    }

    Ok(Json(OkResponse { ok: true }))
}

/// Runs the HTTP API server for MCP bridge communication.
pub async fn run_api_server(
    bind_addr: SocketAddr,
    registry: Arc<SessionRegistry>,
    auth_validator: Arc<AuthValidator>,
    connect_ticket_manager: Arc<SessionConnectTicketManager>,
    automation_access_token_manager: Arc<SessionAutomationAccessTokenManager>,
    session_store: SessionStore,
    session_manager: Arc<SessionManager>,
    recording_artifact_store: Arc<RecordingArtifactStore>,
    recording_observability: Arc<RecordingObservability>,
    recording_lifecycle: Arc<RecordingLifecycleManager>,
    idle_stop_timeout: std::time::Duration,
    public_gateway_url: String,
    default_owner_mode: SessionOwnerMode,
) -> anyhow::Result<()> {
    let state = Arc::new(ApiState {
        registry,
        auth_validator,
        connect_ticket_manager,
        automation_access_token_manager,
        session_store,
        session_manager,
        recording_artifact_store,
        recording_observability,
        recording_lifecycle,
        idle_stop_timeout,
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

fn session_resource(
    state: &ApiState,
    stored: &StoredSession,
    state_override: Option<SessionLifecycleState>,
) -> SessionResource {
    stored.to_resource(
        &state.public_gateway_url,
        state
            .session_manager
            .describe_session_runtime(stored.id)
            .into(),
        state_override,
    )
}

fn extract_bearer_token(headers: &HeaderMap) -> Option<&str> {
    let value = headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    value.strip_prefix("Bearer ")
}

fn extract_automation_access_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(AUTOMATION_ACCESS_TOKEN_HEADER)?
        .to_str()
        .ok()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn build_api_router(state: Arc<ApiState>) -> Router {
    Router::new()
        .route("/api/v1/sessions", post(create_session).get(list_sessions))
        .route(
            "/api/v1/sessions/{session_id}",
            get(get_session).delete(delete_session),
        )
        .route(
            "/api/v1/sessions/{session_id}/recordings",
            post(create_session_recording).get(list_session_recordings),
        )
        .route(
            "/api/v1/sessions/{session_id}/recordings/{recording_id}",
            get(get_session_recording),
        )
        .route(
            "/api/v1/sessions/{session_id}/recordings/{recording_id}/stop",
            post(stop_session_recording),
        )
        .route(
            "/api/v1/sessions/{session_id}/recordings/{recording_id}/complete",
            post(complete_session_recording),
        )
        .route(
            "/api/v1/sessions/{session_id}/recordings/{recording_id}/fail",
            post(fail_session_recording),
        )
        .route(
            "/api/v1/sessions/{session_id}/recordings/{recording_id}/content",
            get(get_session_recording_content),
        )
        .route(
            "/api/v1/sessions/{session_id}/recording-playback",
            get(get_session_recording_playback),
        )
        .route(
            "/api/v1/sessions/{session_id}/recording-playback/manifest",
            get(get_session_recording_playback_manifest),
        )
        .route(
            "/api/v1/sessions/{session_id}/recording-playback/export",
            get(get_session_recording_playback_export),
        )
        .route(
            "/api/v1/sessions/{session_id}/access-tokens",
            post(issue_session_access_token),
        )
        .route(
            "/api/v1/sessions/{session_id}/automation-access",
            post(issue_session_automation_access),
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
        .route(
            "/api/v1/recording/operations",
            get(get_recording_operations),
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
        SessionStoreError::Conflict(_) => (
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

fn map_recording_artifact_store_error(
    error: RecordingArtifactStoreError,
) -> (StatusCode, Json<ErrorResponse>) {
    match error {
        RecordingArtifactStoreError::InvalidSourcePath(_)
        | RecordingArtifactStoreError::InvalidReference(_) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        ),
        RecordingArtifactStoreError::Backend(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        ),
    }
}

fn map_recording_playback_error(
    error: RecordingPlaybackError,
) -> (StatusCode, Json<ErrorResponse>) {
    match error {
        RecordingPlaybackError::Empty => (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        ),
        RecordingPlaybackError::Artifact(RecordingArtifactStoreError::Backend(inner))
            if inner.kind() == std::io::ErrorKind::NotFound =>
        {
            (
                StatusCode::GONE,
                Json(ErrorResponse {
                    error: "a playback segment artifact is no longer available".to_string(),
                }),
            )
        }
        RecordingPlaybackError::Artifact(inner) => map_recording_artifact_store_error(inner),
        RecordingPlaybackError::ManifestEncode(_)
        | RecordingPlaybackError::Io(_)
        | RecordingPlaybackError::Package(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        ),
    }
}

fn map_recording_lifecycle_error(
    error: RecordingLifecycleError,
) -> (StatusCode, Json<ErrorResponse>) {
    match error {
        RecordingLifecycleError::Disabled(_) => (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        ),
        RecordingLifecycleError::InvalidConfiguration(_)
        | RecordingLifecycleError::LaunchFailed(_)
        | RecordingLifecycleError::Store(_) => (
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

    ensure_runtime_candidate_session(session, session_id)
}

async fn authorize_runtime_session_request_with_automation_access(
    headers: &HeaderMap,
    state: &ApiState,
    session_id: Uuid,
) -> Result<StoredSession, (StatusCode, Json<ErrorResponse>)> {
    let session =
        authorize_visible_session_request_with_automation_access(headers, state, session_id)
            .await?;

    ensure_runtime_candidate_session(session, session_id)
}

fn ensure_runtime_candidate_session(
    session: StoredSession,
    session_id: Uuid,
) -> Result<StoredSession, (StatusCode, Json<ErrorResponse>)> {
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

async fn prepare_runtime_access_session(
    state: &ApiState,
    principal: &AuthenticatedPrincipal,
    session_id: Uuid,
) -> Result<StoredSession, (StatusCode, Json<ErrorResponse>)> {
    let stored = state
        .session_store
        .get_session_for_principal(principal, session_id)
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
    let was_stopped = stored.state == SessionLifecycleState::Stopped;

    let connectable = if was_stopped {
        let prepared = state
            .session_store
            .prepare_session_for_connect(session_id)
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
        schedule_idle_session_stop(
            session_id,
            state.idle_stop_timeout,
            state.registry.clone(),
            state.session_store.clone(),
            state.session_manager.clone(),
            state.recording_lifecycle.clone(),
        );
        prepared
    } else {
        stored
    };

    if let Err(error) = state
        .recording_lifecycle
        .ensure_auto_recording(&connectable)
        .await
    {
        if was_stopped {
            let _ = state.session_store.stop_session_if_idle(session_id).await;
            state.session_manager.release(session_id).await;
            state.registry.remove_session(session_id).await;
        }
        return Err(map_recording_lifecycle_error(error));
    }

    if !connectable.state.is_runtime_candidate() {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!(
                    "session {session_id} is not connectable in state {}",
                    connectable.state.as_str()
                ),
            }),
        ));
    }

    Ok(connectable)
}

async fn authorize_visible_session_request_with_automation_access(
    headers: &HeaderMap,
    state: &ApiState,
    session_id: Uuid,
) -> Result<StoredSession, (StatusCode, Json<ErrorResponse>)> {
    if extract_bearer_token(headers).is_some() {
        match authorize_visible_session_request(headers, state, session_id).await {
            Ok(session) => return Ok(session),
            Err(error) if extract_automation_access_token(headers).is_none() => return Err(error),
            Err(_) => {}
        }
    }

    let claims = validate_automation_access_request(headers, state, session_id)?;
    let session = state
        .session_store
        .get_session_by_id(session_id)
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
    if !automation_access_claims_match_session(&claims, &session) {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "automation access token is no longer valid for this session".to_string(),
            }),
        ));
    }

    Ok(session)
}

fn validate_automation_access_request(
    headers: &HeaderMap,
    state: &ApiState,
    session_id: Uuid,
) -> Result<SessionAutomationAccessTokenClaims, (StatusCode, Json<ErrorResponse>)> {
    let token = extract_automation_access_token(headers).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "missing bearer token or session automation access token".to_string(),
            }),
        )
    })?;
    let claims = state
        .automation_access_token_manager
        .validate_token(token)
        .map_err(|error| {
            (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: format!("invalid session automation access token: {error}"),
                }),
            )
        })?;
    if claims.session_id != session_id {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "session automation access token does not match the requested session"
                    .to_string(),
            }),
        ));
    }
    Ok(claims)
}

fn automation_access_claims_match_session(
    claims: &SessionAutomationAccessTokenClaims,
    session: &StoredSession,
) -> bool {
    if session.owner.subject == claims.subject && session.owner.issuer == claims.issuer {
        return true;
    }

    let Some(delegate) = &session.automation_delegate else {
        return false;
    };
    claims.issuer == delegate.issuer
        && claims.client_id.as_deref() == Some(delegate.client_id.as_str())
}

async fn load_session_recording(
    state: &ApiState,
    session_id: Uuid,
    recording_id: Uuid,
) -> Result<StoredSessionRecording, (StatusCode, Json<ErrorResponse>)> {
    state
        .session_store
        .get_recording_for_session(session_id, recording_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!(
                        "recording {recording_id} was not found for session {session_id}"
                    ),
                }),
            )
        })
}

async fn load_session_recording_playback(
    state: &ApiState,
    session_id: Uuid,
) -> Result<PreparedSessionRecordingPlayback, (StatusCode, Json<ErrorResponse>)> {
    let recordings = state
        .session_store
        .list_recordings_for_session(session_id)
        .await
        .map_err(map_session_store_error)?;
    Ok(prepare_session_recording_playback(
        session_id,
        &recordings,
        Utc::now(),
    ))
}

fn latest_recording(recordings: &[StoredSessionRecording]) -> Option<&StoredSessionRecording> {
    recordings.iter().max_by(|left, right| {
        left.updated_at
            .cmp(&right.updated_at)
            .then_with(|| left.created_at.cmp(&right.created_at))
    })
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

fn should_block_session_stop(
    state: SessionLifecycleState,
    supports_legacy_global_routes: bool,
    runtime_in_use: bool,
) -> bool {
    supports_legacy_global_routes && state.is_runtime_candidate() && runtime_in_use
}

fn recording_mime_type(format: SessionRecordingFormat) -> &'static str {
    match format {
        SessionRecordingFormat::Webm => "video/webm",
    }
}

async fn resolve_runtime(
    state: &ApiState,
    session_id: Uuid,
) -> Result<SessionRuntime, (StatusCode, Json<ErrorResponse>)> {
    state
        .session_manager
        .resolve(session_id)
        .await
        .map_err(map_session_manager_error)
}

async fn resolve_runtime_compat(
    state: &ApiState,
    session_id: Uuid,
) -> Result<SessionRuntime, StatusCode> {
    state
        .session_manager
        .resolve(session_id)
        .await
        .map_err(|_| StatusCode::CONFLICT)
}

fn map_session_manager_error(error: SessionManagerError) -> (StatusCode, Json<ErrorResponse>) {
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
        .session_manager
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
