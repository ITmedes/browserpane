use super::support::{principal, service_principal};
use super::*;

#[tokio::test]
async fn in_memory_store_scopes_sessions_to_owner() {
    let store = SessionStore::in_memory();
    let alpha = principal("alpha");
    let bravo = principal("bravo");

    let created = store
        .create_session(
            &alpha,
            CreateSessionRequest {
                template_id: Some("default".to_string()),
                owner_mode: None,
                viewport: Some(SessionViewport {
                    width: 1920,
                    height: 1080,
                }),
                idle_timeout_sec: Some(600),
                labels: HashMap::from([("suite".to_string(), "smoke".to_string())]),
                integration_context: Some(serde_json::json!({ "ticket": "BPANE-6" })),
                extension_ids: Vec::new(),
                extensions: Vec::new(),
                recording: SessionRecordingPolicy {
                    mode: SessionRecordingMode::Manual,
                    format: SessionRecordingFormat::Webm,
                    retention_sec: Some(86_400),
                },
            },
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap();

    let alpha_sessions = store.list_sessions_for_owner(&alpha).await.unwrap();
    assert_eq!(alpha_sessions.len(), 1);
    assert_eq!(alpha_sessions[0].id, created.id);
    assert_eq!(alpha_sessions[0].recording, created.recording);
    assert_eq!(created.recording.mode, SessionRecordingMode::Manual);
    assert_eq!(created.recording.format, SessionRecordingFormat::Webm);
    assert_eq!(created.recording.retention_sec, Some(86_400));

    let bravo_sessions = store.list_sessions_for_owner(&bravo).await.unwrap();
    assert!(bravo_sessions.is_empty());
    assert!(store
        .get_session_for_owner(&bravo, created.id)
        .await
        .unwrap()
        .is_none());
}

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
async fn in_memory_store_allows_delegated_client_to_load_session() {
    let store = SessionStore::in_memory();
    let owner = principal("owner");
    let delegate = service_principal("service-account-id", "bpane-mcp-bridge");

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

    let updated = store
        .set_automation_delegate_for_owner(
            &owner,
            created.id,
            SetAutomationDelegateRequest {
                client_id: "bpane-mcp-bridge".to_string(),
                issuer: None,
                display_name: Some("BrowserPane MCP bridge".to_string()),
            },
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        updated.automation_delegate.as_ref().unwrap().client_id,
        "bpane-mcp-bridge"
    );

    let visible = store
        .get_session_for_principal(&delegate, created.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(visible.id, created.id);
}

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
async fn in_memory_store_can_prepare_a_stopped_session_for_reconnect() {
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

    let resumed = store
        .prepare_session_for_connect(created.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(resumed.state, SessionLifecycleState::Ready);
    assert!(resumed.stopped_at.is_none());
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
