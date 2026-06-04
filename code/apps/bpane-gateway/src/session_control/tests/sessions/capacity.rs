use super::*;

fn project_policy_session_request(
    project_id: Uuid,
    template_id: Option<&str>,
    egress_profile_id: Option<Uuid>,
) -> CreateSessionRequest {
    CreateSessionRequest {
        project_id: Some(project_id),
        template_id: template_id.map(str::to_string),
        browser_context: None,
        network_identity: egress_profile_id.map(|profile_id| SessionNetworkIdentity {
            egress_profile_id: Some(profile_id),
            ..SessionNetworkIdentity::default()
        }),
        owner_mode: None,
        viewport: None,
        idle_timeout_sec: None,
        labels: HashMap::new(),
        integration_context: None,
        extension_ids: Vec::new(),
        extensions: Vec::new(),
        recording: SessionRecordingPolicy::default(),
    }
}

#[tokio::test]
async fn in_memory_store_limits_legacy_runtime_to_one_active_session() {
    let store = SessionStore::in_memory();
    let alpha = principal("alpha");

    store
        .create_session(
            &alpha,
            CreateSessionRequest {
                project_id: None,
                template_id: None,
                browser_context: None,
                network_identity: None,
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
                project_id: None,
                template_id: None,
                browser_context: None,
                network_identity: None,
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
async fn in_memory_store_enforces_project_template_policy() {
    let store = SessionStore::in_memory_with_config(SessionManagerProfile {
        runtime_binding: "docker_runtime_pool".to_string(),
        compatibility_mode: "session_runtime_pool".to_string(),
        max_runtime_sessions: 4,
        supports_legacy_global_routes: false,
        supports_session_extensions: true,
    });
    let owner = principal("owner");
    let project = store
        .create_project(
            &owner,
            PersistProjectRequest {
                name: "Template policy".to_string(),
                description: None,
                labels: HashMap::new(),
                quotas: ProjectQuotas::default(),
                policy: ProjectPolicy {
                    allowed_session_template_ids: vec!["allowed-template".to_string()],
                    allowed_egress_profile_ids: Vec::new(),
                },
                state: ProjectState::Active,
            },
        )
        .await
        .unwrap();

    let allowed = store
        .create_session(
            &owner,
            project_policy_session_request(project.id, Some("allowed-template"), None),
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap();
    assert_eq!(allowed.project_id, Some(project.id));

    let error = store
        .create_session(
            &owner,
            project_policy_session_request(project.id, Some("other-template"), None),
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap_err();
    assert!(
        matches!(error, SessionStoreError::Conflict(message) if message.contains("session_template_not_allowed"))
    );
}

#[tokio::test]
async fn in_memory_store_enforces_project_egress_policy() {
    let store = SessionStore::in_memory_with_config(SessionManagerProfile {
        runtime_binding: "docker_runtime_pool".to_string(),
        compatibility_mode: "session_runtime_pool".to_string(),
        max_runtime_sessions: 4,
        supports_legacy_global_routes: false,
        supports_session_extensions: true,
    });
    let owner = principal("owner");
    let allowed_profile_id = Uuid::now_v7();
    let project = store
        .create_project(
            &owner,
            PersistProjectRequest {
                name: "Egress policy".to_string(),
                description: None,
                labels: HashMap::new(),
                quotas: ProjectQuotas::default(),
                policy: ProjectPolicy {
                    allowed_session_template_ids: Vec::new(),
                    allowed_egress_profile_ids: vec![allowed_profile_id],
                },
                state: ProjectState::Active,
            },
        )
        .await
        .unwrap();

    let error = store
        .create_session(
            &owner,
            project_policy_session_request(project.id, None, Some(Uuid::now_v7())),
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap_err();
    assert!(
        matches!(error, SessionStoreError::Conflict(message) if message.contains("egress_profile_not_allowed"))
    );

    let allowed = store
        .create_session(
            &owner,
            project_policy_session_request(project.id, None, Some(allowed_profile_id)),
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap();
    assert_eq!(
        allowed.network_identity.egress_profile_id,
        Some(allowed_profile_id)
    );
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
                    project_id: None,
                    template_id: None,
                    browser_context: None,
                    network_identity: None,
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
                project_id: None,
                template_id: None,
                browser_context: None,
                network_identity: None,
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
                project_id: None,
                template_id: None,
                browser_context: None,
                network_identity: None,
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
                project_id: None,
                template_id: None,
                browser_context: None,
                network_identity: None,
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
