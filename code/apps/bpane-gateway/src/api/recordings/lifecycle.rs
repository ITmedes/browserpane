use super::*;

pub(super) async fn list_session_recordings(
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

pub(super) async fn create_session_recording(
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

pub(super) async fn get_session_recording(
    headers: HeaderMap,
    Path((session_id, recording_id)): Path<(Uuid, Uuid)>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionRecordingResource>, (StatusCode, Json<ErrorResponse>)> {
    let _session = authorize_visible_session_request(&headers, &state, session_id).await?;
    let recording = load_session_recording(&state, session_id, recording_id).await?;
    Ok(Json(recording.to_resource()))
}

pub(super) async fn stop_session_recording(
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

pub(super) async fn complete_session_recording(
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

pub(super) async fn fail_session_recording(
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
