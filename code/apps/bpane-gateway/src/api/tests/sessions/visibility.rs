use super::*;

#[tokio::test]
async fn scopes_session_resources_to_the_authenticated_owner() {
    let auth_validator = Arc::new(AuthValidator::from_hmac_secret(vec![9; 32]));
    let alpha_token = auth_validator.generate_token().unwrap();
    tokio::time::sleep(Duration::from_secs(1)).await;
    let bravo_token = auth_validator.generate_token().unwrap();
    let state = Arc::new(ApiState {
        registry: Arc::new(SessionRegistry::new(10, false)),
        auth_validator,
        connect_ticket_manager: Arc::new(SessionConnectTicketManager::new(
            vec![5; 32],
            Duration::from_secs(300),
        )),
        automation_access_token_manager: Arc::new(SessionAutomationAccessTokenManager::new(
            vec![6; 32],
            Duration::from_secs(300),
        )),
        session_store: SessionStore::in_memory(),
        session_manager: Arc::new(
            SessionManager::new(SessionManagerConfig::StaticSingle {
                agent_socket_path: "/tmp/test.sock".to_string(),
                cdp_endpoint: Some("http://host:9223".to_string()),
                idle_timeout: Duration::from_secs(300),
            })
            .unwrap(),
        ),
        credential_provider: Some(test_credential_provider()),
        recording_artifact_store: test_artifact_store(),
        workspace_file_store: test_workspace_file_store(),
        workflow_source_resolver: test_workflow_source_resolver(),
        recording_observability: Arc::new(RecordingObservability::default()),
        recording_lifecycle: Arc::new(RecordingLifecycleManager::disabled()),
        workflow_lifecycle: Arc::new(WorkflowLifecycleManager::disabled()),
        workflow_observability: Arc::new(WorkflowObservability::default()),
        workflow_log_retention: None,
        workflow_output_retention: None,
        idle_stop_timeout: Duration::from_secs(300),
        public_gateway_url: "https://localhost:4433".to_string(),
        default_owner_mode: SessionOwnerMode::Collaborative,
    });
    let app = build_api_router(state);

    let created = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sessions")
                    .header("authorization", bearer(&alpha_token))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let session_id = created["id"].as_str().unwrap().to_string();

    let lookup = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/sessions/{session_id}"))
                .header("authorization", bearer(&bravo_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(lookup.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn rejects_session_scoped_runtime_routes_for_unknown_or_foreign_sessions_before_runtime_work()
{
    let auth_validator = Arc::new(AuthValidator::from_hmac_secret(vec![11; 32]));
    let alpha_token = auth_validator.generate_token().unwrap();
    tokio::time::sleep(Duration::from_secs(1)).await;
    let bravo_token = auth_validator.generate_token().unwrap();
    let state = Arc::new(ApiState {
        registry: Arc::new(SessionRegistry::new(10, false)),
        auth_validator,
        connect_ticket_manager: Arc::new(SessionConnectTicketManager::new(
            vec![5; 32],
            Duration::from_secs(300),
        )),
        automation_access_token_manager: Arc::new(SessionAutomationAccessTokenManager::new(
            vec![6; 32],
            Duration::from_secs(300),
        )),
        session_store: SessionStore::in_memory(),
        session_manager: Arc::new(
            SessionManager::new(SessionManagerConfig::StaticSingle {
                agent_socket_path: "/tmp/test.sock".to_string(),
                cdp_endpoint: Some("http://host:9223".to_string()),
                idle_timeout: Duration::from_secs(300),
            })
            .unwrap(),
        ),
        credential_provider: Some(test_credential_provider()),
        recording_artifact_store: test_artifact_store(),
        workspace_file_store: test_workspace_file_store(),
        workflow_source_resolver: test_workflow_source_resolver(),
        recording_observability: Arc::new(RecordingObservability::default()),
        recording_lifecycle: Arc::new(RecordingLifecycleManager::disabled()),
        workflow_lifecycle: Arc::new(WorkflowLifecycleManager::disabled()),
        workflow_observability: Arc::new(WorkflowObservability::default()),
        workflow_log_retention: None,
        workflow_output_retention: None,
        idle_stop_timeout: Duration::from_secs(300),
        public_gateway_url: "https://localhost:4433".to_string(),
        default_owner_mode: SessionOwnerMode::Collaborative,
    });
    let app = build_api_router(state);

    let created = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sessions")
                    .header("authorization", bearer(&alpha_token))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let session_id = created["id"].as_str().unwrap().to_string();

    let foreign_status = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/sessions/{session_id}/status"))
                .header("authorization", bearer(&bravo_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(foreign_status.status(), StatusCode::NOT_FOUND);

    let unknown_owner = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/mcp-owner"))
                .header("authorization", bearer(&bravo_token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "width": 1280, "height": 720 }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unknown_owner.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn session_status_reports_stopped_sessions_without_runtime_side_effects() {
    let (app, token, state) = test_router_with_state();

    let created = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sessions")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let session_id = created["id"].as_str().unwrap().to_string();
    let session_uuid = Uuid::parse_str(&session_id).unwrap();

    let delete_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/sessions/{session_id}"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_response.status(), StatusCode::OK);
    assert!(state
        .registry
        .telemetry_snapshot_if_live(session_uuid)
        .await
        .is_none());

    let status_response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/sessions/{session_id}/status"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(status_response.status(), StatusCode::OK);
    let body = response_json(status_response).await;
    assert_eq!(body["state"], "stopped");
    assert_eq!(body["runtime_state"], "stopped");
    assert_eq!(body["presence_state"], "empty");
    assert_eq!(body["connection_counts"]["total_clients"], 0);
    assert_eq!(body["connection_counts"]["interactive_clients"], 0);
    assert_eq!(body["connection_counts"]["automation_clients"], 0);
    assert_eq!(body["stop_eligibility"]["allowed"], true);
    assert_eq!(body["browser_clients"], 0);
    assert_eq!(body["viewer_clients"], 0);
    assert_eq!(body["recorder_clients"], 0);
    assert_eq!(body["mcp_owner"], false);
    assert!(state
        .registry
        .telemetry_snapshot_if_live(session_uuid)
        .await
        .is_none());
}

#[tokio::test]
async fn session_resource_and_status_reads_do_not_create_live_hubs() {
    let (app, token, state) = test_router_with_state();

    let created = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sessions")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let session_id = created["id"].as_str().unwrap().to_string();
    let session_uuid = Uuid::parse_str(&session_id).unwrap();
    assert!(state
        .registry
        .telemetry_snapshot_if_live(session_uuid)
        .await
        .is_none());

    let get_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/sessions/{session_id}"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_response.status(), StatusCode::OK);
    let get_body = response_json(get_response).await;
    assert_eq!(get_body["status"]["runtime_state"], "not_started");
    assert_eq!(get_body["status"]["presence_state"], "empty");
    assert_eq!(get_body["status"]["connection_counts"]["total_clients"], 0);
    assert!(state
        .registry
        .telemetry_snapshot_if_live(session_uuid)
        .await
        .is_none());

    let status_response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/sessions/{session_id}/status"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(status_response.status(), StatusCode::OK);
    let status_body = response_json(status_response).await;
    assert_eq!(status_body["state"], "ready");
    assert_eq!(status_body["runtime_state"], "not_started");
    assert_eq!(status_body["presence_state"], "empty");
    assert_eq!(status_body["connection_counts"]["total_clients"], 0);
    assert_eq!(status_body["browser_clients"], 0);
    assert!(state
        .registry
        .telemetry_snapshot_if_live(session_uuid)
        .await
        .is_none());
}
