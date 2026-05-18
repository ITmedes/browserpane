use super::*;

#[tokio::test]
async fn in_memory_store_stops_unused_ready_sessions_and_idle_sessions() {
    let store = SessionStore::in_memory();
    let owner = principal("owner");
    let created = store
        .create_session(
            &owner,
            CreateSessionRequest {
                template_id: None,
                owner_mode: None,
                viewport: None,
                idle_timeout_sec: Some(300),
                labels: HashMap::new(),
                integration_context: None,
                extension_ids: Vec::new(),
                extensions: Vec::new(),
                recording: SessionRecordingPolicy::default(),
            },
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap();

    let stopped_ready = store
        .stop_session_if_idle(created.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stopped_ready.state, SessionLifecycleState::Stopped);

    let created = store
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
                recording: SessionRecordingPolicy::default(),
            },
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap();

    let active = store
        .mark_session_active(created.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(active.state, SessionLifecycleState::Active);

    let idle = store.mark_session_idle(created.id).await.unwrap().unwrap();
    assert_eq!(idle.state, SessionLifecycleState::Idle);

    let stopped = store
        .stop_session_if_idle(created.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stopped.state, SessionLifecycleState::Stopped);

    let after = store
        .mark_session_active(created.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.state, SessionLifecycleState::Stopped);
}

#[tokio::test]
async fn in_memory_store_rejects_stopped_session_reconnect_prep() {
    let store = SessionStore::in_memory();
    let owner = principal("owner");
    let created = store
        .create_session(
            &owner,
            CreateSessionRequest {
                template_id: None,
                owner_mode: None,
                viewport: None,
                idle_timeout_sec: Some(300),
                labels: HashMap::new(),
                integration_context: None,
                extension_ids: Vec::new(),
                extensions: Vec::new(),
                recording: SessionRecordingPolicy::default(),
            },
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap();

    let stopped = store
        .stop_session_if_idle(created.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stopped.state, SessionLifecycleState::Stopped);

    let error = store
        .prepare_session_for_connect(created.id)
        .await
        .unwrap_err();
    assert!(matches!(error, SessionStoreError::Conflict(_)));
}

#[tokio::test]
async fn in_memory_store_can_prepare_a_released_session_for_reconnect() {
    let store = SessionStore::in_memory();
    let owner = principal("owner");
    let created = store
        .create_session(
            &owner,
            CreateSessionRequest {
                template_id: None,
                owner_mode: None,
                viewport: None,
                idle_timeout_sec: Some(300),
                labels: HashMap::new(),
                integration_context: None,
                extension_ids: Vec::new(),
                extensions: Vec::new(),
                recording: SessionRecordingPolicy::default(),
            },
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap();

    let released = store
        .release_session_runtime_for_owner(&owner, created.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(released.state, SessionLifecycleState::Released);
    assert!(released.runtime_released_at.is_some());

    let resumed = store
        .prepare_session_for_connect(created.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(resumed.state, SessionLifecycleState::Ready);
    assert!(resumed.stopped_at.is_none());
    assert!(resumed.runtime_released_at.is_some());
}

#[tokio::test]
async fn in_memory_store_can_restore_runtime_candidate_to_ready_after_runtime_loss() {
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
                recording: SessionRecordingPolicy::default(),
            },
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap();

    let active = store
        .mark_session_active(session.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(active.state, SessionLifecycleState::Active);

    let restored = store
        .mark_session_ready_after_runtime_loss(session.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(restored.state, SessionLifecycleState::Ready);
}
