use super::*;

#[tokio::test]
async fn in_memory_store_limits_legacy_runtime_to_one_active_session() {
    let store = SessionStore::in_memory();
    let alpha = principal("alpha");

    store
        .create_session(
            &alpha,
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

    let error = store
        .create_session(
            &alpha,
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
        .unwrap_err();

    assert!(matches!(
        error,
        SessionStoreError::ActiveSessionConflict {
            max_runtime_sessions: 1
        }
    ));
}

#[tokio::test]
async fn in_memory_store_respects_runtime_pool_capacity() {
    let store = SessionStore::in_memory_with_config(SessionManagerProfile {
        runtime_binding: "docker_runtime_pool".to_string(),
        compatibility_mode: "session_runtime_pool".to_string(),
        max_runtime_sessions: 2,
        supports_legacy_global_routes: false,
        supports_session_extensions: true,
    });
    let alpha = principal("alpha");

    for _ in 0..2 {
        store
            .create_session(
                &alpha,
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
    }

    let error = store
        .create_session(
            &alpha,
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
        .unwrap_err();

    assert!(matches!(
        error,
        SessionStoreError::ActiveSessionConflict {
            max_runtime_sessions: 2
        }
    ));
}

#[tokio::test]
async fn reconnect_prep_respects_runtime_pool_capacity() {
    let store = SessionStore::in_memory_with_config(SessionManagerProfile {
        runtime_binding: "docker_runtime_pool".to_string(),
        compatibility_mode: "session_runtime_pool".to_string(),
        max_runtime_sessions: 1,
        supports_legacy_global_routes: false,
        supports_session_extensions: true,
    });
    let owner = principal("owner");

    let ready = store
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
    assert_eq!(ready.state, SessionLifecycleState::Ready);

    let stopped = store
        .stop_session_for_owner(&owner, ready.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stopped.state, SessionLifecycleState::Stopped);

    let replacement = store
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
    assert_eq!(replacement.state, SessionLifecycleState::Ready);

    let error = store
        .prepare_session_for_connect(stopped.id)
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        SessionStoreError::ActiveSessionConflict {
            max_runtime_sessions: 1
        }
    ));
}
