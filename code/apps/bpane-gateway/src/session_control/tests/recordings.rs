use super::support::principal;
use super::*;

#[tokio::test]
async fn in_memory_store_creates_and_stops_recording_metadata() {
    let store = SessionStore::in_memory();
    let owner = principal("owner");
    let session = store
        .create_session(
            &owner,
            CreateSessionRequest {
                template_id: None,
                owner_mode: None,
                viewport: None,
                idle_timeout_sec: None,
                labels: HashMap::new(),
                integration_context: None,
                extension_ids: Vec::new(),
                extensions: Vec::new(),
                recording: SessionRecordingPolicy {
                    mode: SessionRecordingMode::Manual,
                    format: SessionRecordingFormat::Webm,
                    retention_sec: None,
                },
            },
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap();

    let created = store
        .create_recording_for_session(session.id, SessionRecordingFormat::Webm, None)
        .await
        .unwrap();
    assert_eq!(created.session_id, session.id);
    assert_eq!(created.previous_recording_id, None);
    assert_eq!(created.state, SessionRecordingState::Recording);
    assert_eq!(created.mime_type.as_deref(), Some("video/webm"));

    let listed = store.list_recordings_for_session(session.id).await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, created.id);

    let stopped = store
        .stop_recording_for_session(
            session.id,
            created.id,
            SessionRecordingTerminationReason::ManualStop,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stopped.state, SessionRecordingState::Finalizing);
    assert_eq!(
        stopped.termination_reason,
        Some(SessionRecordingTerminationReason::ManualStop)
    );

    let latest = store
        .get_latest_recording_for_session(session.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(latest.id, created.id);
    assert_eq!(latest.state, SessionRecordingState::Finalizing);
    assert_eq!(
        latest.termination_reason,
        Some(SessionRecordingTerminationReason::ManualStop)
    );

    let completed = store
        .complete_recording_for_session(
            session.id,
            created.id,
            PersistCompletedSessionRecordingRequest {
                artifact_ref: "local_fs:session/recording.webm".to_string(),
                mime_type: Some("video/webm".to_string()),
                bytes: Some(123),
                duration_ms: Some(456),
            },
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(completed.state, SessionRecordingState::Ready);
    assert_eq!(
        completed.artifact_ref.as_deref(),
        Some("local_fs:session/recording.webm")
    );
    assert_eq!(completed.bytes, Some(123));
    assert_eq!(completed.duration_ms, Some(456));

    let failed = store
        .create_recording_for_session(session.id, SessionRecordingFormat::Webm, Some(created.id))
        .await
        .unwrap();
    let failed = store
        .fail_recording_for_session(
            session.id,
            failed.id,
            FailSessionRecordingRequest {
                error: "boom".to_string(),
                termination_reason: Some(SessionRecordingTerminationReason::WorkerExit),
            },
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(failed.state, SessionRecordingState::Failed);
    assert_eq!(failed.previous_recording_id, Some(created.id));
    assert_eq!(failed.error.as_deref(), Some("boom"));
    assert_eq!(
        failed.termination_reason,
        Some(SessionRecordingTerminationReason::WorkerExit)
    );
}

#[tokio::test]
async fn in_memory_store_lists_and_clears_expired_recording_artifacts() {
    let store = SessionStore::in_memory();
    let owner = principal("owner");
    let session = store
        .create_session(
            &owner,
            CreateSessionRequest {
                template_id: None,
                owner_mode: None,
                viewport: None,
                idle_timeout_sec: None,
                labels: HashMap::new(),
                integration_context: None,
                extension_ids: Vec::new(),
                extensions: Vec::new(),
                recording: SessionRecordingPolicy {
                    mode: SessionRecordingMode::Manual,
                    format: SessionRecordingFormat::Webm,
                    retention_sec: Some(60),
                },
            },
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap();

    let created = store
        .create_recording_for_session(session.id, SessionRecordingFormat::Webm, None)
        .await
        .unwrap();
    let completed = store
        .complete_recording_for_session(
            session.id,
            created.id,
            PersistCompletedSessionRecordingRequest {
                artifact_ref: "local_fs:session/recording.webm".to_string(),
                mime_type: Some("video/webm".to_string()),
                bytes: Some(123),
                duration_ms: Some(456),
            },
        )
        .await
        .unwrap()
        .unwrap();

    let candidates = store
        .list_recording_artifact_retention_candidates(
            completed.completed_at.unwrap() + chrono::Duration::seconds(61),
        )
        .await
        .unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].session_id, session.id);
    assert_eq!(candidates[0].recording_id, created.id);
    assert_eq!(
        candidates[0].artifact_ref,
        "local_fs:session/recording.webm"
    );

    let cleared = store
        .clear_recording_artifact_path(session.id, created.id)
        .await
        .unwrap()
        .unwrap();
    assert!(cleared.artifact_ref.is_none());

    let candidates = store
        .list_recording_artifact_retention_candidates(
            completed.completed_at.unwrap() + chrono::Duration::seconds(61),
        )
        .await
        .unwrap();
    assert!(candidates.is_empty());
}
