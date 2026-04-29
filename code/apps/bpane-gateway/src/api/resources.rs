use super::*;

pub(super) fn session_status_from_snapshot(
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

pub(super) fn recording_status_from_snapshot(
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

pub(super) fn session_resource(
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

pub(super) async fn load_session_recording(
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

pub(super) async fn load_session_recording_playback(
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

pub(super) fn latest_recording(
    recordings: &[StoredSessionRecording],
) -> Option<&StoredSessionRecording> {
    recordings.iter().max_by(|left, right| {
        left.updated_at
            .cmp(&right.updated_at)
            .then_with(|| left.created_at.cmp(&right.created_at))
    })
}

pub(super) async fn build_workflow_run_resource(
    state: &ApiState,
    run: &StoredWorkflowRun,
) -> Result<WorkflowRunResource, (StatusCode, Json<ErrorResponse>)> {
    let recordings = state
        .session_store
        .list_recordings_for_session(run.session_id)
        .await
        .map_err(map_session_store_error)?
        .into_iter()
        .filter(|recording| workflow_run_recording_matches(run, recording, Utc::now()))
        .map(workflow_run_recording_resource)
        .collect::<Vec<_>>();
    let events = workflow_run_event_resources(state, run).await?;
    let admission = derive_workflow_run_admission_resource(run.state, &events);
    let intervention = derive_workflow_run_intervention_resource(run.state, &events);
    let session_state = state
        .session_store
        .get_session_by_id(run.session_id)
        .await
        .map_err(map_session_store_error)?
        .map(|session| session.state);
    let runtime = derive_workflow_run_runtime_resource(run.state, session_state, &events);
    Ok(run.to_resource(
        recordings,
        workflow_run_retention_resource(state, run),
        admission,
        intervention,
        runtime,
    ))
}

pub(super) async fn workflow_run_event_resources(
    state: &ApiState,
    run: &StoredWorkflowRun,
) -> Result<Vec<WorkflowRunEventResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = load_session_owner_principal(state, run.session_id).await?;
    let mut events = state
        .session_store
        .list_workflow_run_events_for_owner(&principal, run.id)
        .await
        .map_err(map_session_store_error)?
        .into_iter()
        .map(|event| event.to_resource())
        .collect::<Vec<WorkflowRunEventResource>>();
    let task_events = state
        .session_store
        .list_automation_task_events_for_owner(&principal, run.automation_task_id)
        .await
        .map_err(map_session_store_error)?
        .into_iter()
        .map(|event| {
            WorkflowRunEventResource::from_automation_task(run.id, run.automation_task_id, &event)
        });
    events.extend(task_events);
    events.sort_by(|left, right| {
        left.created_at
            .cmp(&right.created_at)
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(events)
}

pub(super) async fn workflow_run_intervention_resource(
    state: &ApiState,
    run: &StoredWorkflowRun,
) -> Result<WorkflowRunInterventionResource, (StatusCode, Json<ErrorResponse>)> {
    let events = workflow_run_event_resources(state, run).await?;
    Ok(derive_workflow_run_intervention_resource(
        run.state, &events,
    ))
}

pub(super) async fn load_owner_workflow_run(
    state: &ApiState,
    principal: &AuthenticatedPrincipal,
    run_id: Uuid,
) -> Result<StoredWorkflowRun, (StatusCode, Json<ErrorResponse>)> {
    state
        .session_store
        .get_workflow_run_for_owner(principal, run_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("workflow run {run_id} not found"),
                }),
            )
        })
}

pub(super) fn ensure_run_awaiting_input(
    run: &StoredWorkflowRun,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    if run.state != WorkflowRunState::AwaitingInput {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!("workflow run {} is not awaiting input", run.id),
            }),
        ));
    }
    Ok(())
}

pub(super) fn workflow_run_intervention_resolution_data(
    request_id: Option<Uuid>,
    action: &str,
    input: Option<Value>,
    reason: Option<String>,
    principal: &AuthenticatedPrincipal,
    details: Option<Value>,
) -> Value {
    serde_json::json!({
        "intervention_resolution": {
            "request_id": request_id.map(|value| value.to_string()),
            "action": action,
            "input": input,
            "reason": reason,
            "actor_subject": principal.subject,
            "actor_issuer": principal.issuer,
            "actor_display_name": principal.display_name,
            "details": details
        }
    })
}

pub(super) fn trim_optional_comment(
    comment: Option<String>,
) -> Result<Option<String>, (StatusCode, Json<ErrorResponse>)> {
    match comment {
        Some(comment) if comment.trim().is_empty() => Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "comment must not be empty when provided".to_string(),
            }),
        )),
        Some(comment) => Ok(Some(comment.trim().to_string())),
        None => Ok(None),
    }
}

pub(super) fn workflow_run_recording_matches(
    run: &StoredWorkflowRun,
    recording: &StoredSessionRecording,
    now: chrono::DateTime<chrono::Utc>,
) -> bool {
    let run_started_at = run.started_at.unwrap_or(run.created_at);
    let run_ended_at = run.completed_at.unwrap_or(now);
    let recording_ended_at = recording.completed_at.unwrap_or(now);
    recording.started_at <= run_ended_at && recording_ended_at >= run_started_at
}

pub(super) fn workflow_run_recording_resource(
    recording: StoredSessionRecording,
) -> WorkflowRunRecordingResource {
    WorkflowRunRecordingResource {
        id: recording.id,
        session_id: recording.session_id,
        state: recording.state.as_str().to_string(),
        format: recording.format.as_str().to_string(),
        mime_type: recording.mime_type,
        bytes: recording.bytes,
        duration_ms: recording.duration_ms,
        error: recording.error,
        termination_reason: recording
            .termination_reason
            .map(|reason| reason.as_str().to_string()),
        previous_recording_id: recording.previous_recording_id,
        started_at: recording.started_at,
        completed_at: recording.completed_at,
        content_path: format!(
            "/api/v1/sessions/{}/recordings/{}/content",
            recording.session_id, recording.id
        ),
        created_at: recording.created_at,
        updated_at: recording.updated_at,
    }
}

pub(super) fn workflow_run_retention_resource(
    state: &ApiState,
    run: &StoredWorkflowRun,
) -> WorkflowRunRetentionResource {
    let output_expire_at = run.completed_at.and_then(|completed_at| {
        state
            .workflow_output_retention
            .map(|retention| completed_at + retention)
    });
    let logs_expire_at = run.completed_at.and_then(|completed_at| {
        state
            .workflow_log_retention
            .map(|retention| completed_at + retention)
    });
    WorkflowRunRetentionResource {
        logs_expire_at,
        output_expire_at,
    }
}

pub(super) fn recording_mime_type(format: SessionRecordingFormat) -> &'static str {
    match format {
        SessionRecordingFormat::Webm => "video/webm",
    }
}
