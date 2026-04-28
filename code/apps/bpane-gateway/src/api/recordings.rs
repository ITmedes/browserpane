use axum::routing::{get, post};

use super::*;

pub(super) fn recording_routes() -> Router<Arc<ApiState>> {
    Router::new()
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
}

pub(super) fn recording_operation_routes() -> Router<Arc<ApiState>> {
    Router::new().route(
        "/api/v1/recording/operations",
        get(get_recording_operations),
    )
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
