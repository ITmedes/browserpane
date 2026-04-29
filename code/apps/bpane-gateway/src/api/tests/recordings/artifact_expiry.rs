use super::*;

#[tokio::test]
async fn expired_recording_artifacts_return_gone() {
    let auth_validator = Arc::new(AuthValidator::from_hmac_secret(vec![7; 32]));
    let token = auth_validator.generate_token().unwrap();
    let session_store = SessionStore::in_memory();
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
        session_store: session_store.clone(),
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

    let session = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sessions")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "recording": {
                              "mode": "manual",
                              "format": "webm"
                            }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let session_uuid = Uuid::parse_str(session["id"].as_str().unwrap()).unwrap();

    let recording = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/sessions/{session_uuid}/recordings"))
                    .header("authorization", bearer(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let recording_uuid = Uuid::parse_str(recording["id"].as_str().unwrap()).unwrap();

    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/sessions/{session_uuid}/recordings/{recording_uuid}/stop"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let temp_dir = tempfile::tempdir().unwrap();
    let artifact_path = temp_dir.path().join("expired-segment.webm");
    std::fs::write(&artifact_path, b"expired-segment").unwrap();
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/sessions/{session_uuid}/recordings/{recording_uuid}/complete"
                ))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "source_path": artifact_path.to_string_lossy(),
                        "mime_type": "video/webm",
                        "bytes": 15,
                        "duration_ms": 1500
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    session_store
        .clear_recording_artifact_path(session_uuid, recording_uuid)
        .await
        .unwrap();

    let content_response = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/sessions/{session_uuid}/recordings/{recording_uuid}/content"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(content_response.status(), StatusCode::GONE);
    let body = response_json(content_response).await;
    assert_eq!(
        body["error"],
        format!("recording artifact for {recording_uuid} is no longer available")
    );
}
