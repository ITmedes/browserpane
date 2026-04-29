use super::*;

#[tokio::test]
async fn recording_operations_snapshot_tracks_finalize_playback_and_failures() {
    let auth_validator = Arc::new(AuthValidator::from_hmac_secret(vec![7; 32]));
    let token = auth_validator.generate_token().unwrap();
    let observability = Arc::new(RecordingObservability::default());
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
        recording_observability: observability.clone(),
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
    let session_id = session["id"].as_str().unwrap().to_string();

    let recording = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/sessions/{session_id}/recordings"))
                    .header("authorization", bearer(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let recording_id = recording["id"].as_str().unwrap().to_string();

    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/sessions/{session_id}/recordings/{recording_id}/stop"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let temp_dir = tempfile::tempdir().unwrap();
    let artifact_path = temp_dir.path().join("segment.webm");
    std::fs::write(&artifact_path, b"segment").unwrap();
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/sessions/{session_id}/recordings/{recording_id}/complete"
                ))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "source_path": artifact_path.to_string_lossy(),
                        "mime_type": "video/webm",
                        "bytes": 7,
                        "duration_ms": 700
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let failed_recording = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/sessions/{session_id}/recordings"))
                    .header("authorization", bearer(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let failed_recording_id = failed_recording["id"].as_str().unwrap().to_string();
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/sessions/{session_id}/recordings/{failed_recording_id}/fail"
                ))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "error": "worker exited",
                        "termination_reason": "worker_exit"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/sessions/{session_id}/recording-playback"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/sessions/{session_id}/recording-playback/export"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let snapshot_response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/recording/operations")
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(snapshot_response.status(), StatusCode::OK);
    let snapshot = response_json(snapshot_response).await;
    assert_eq!(snapshot["artifact_finalize_requests_total"], 1);
    assert_eq!(snapshot["artifact_finalize_successes_total"], 1);
    assert_eq!(snapshot["artifact_finalize_failures_total"], 0);
    assert_eq!(snapshot["recording_failures_total"], 1);
    assert_eq!(snapshot["playback_manifest_requests_total"], 0);
    assert_eq!(snapshot["playback_export_requests_total"], 1);
    assert_eq!(snapshot["playback_export_successes_total"], 1);
    assert_eq!(snapshot["playback_export_failures_total"], 0);
    assert!(snapshot["playback_export_bytes_total"].as_u64().unwrap() > 0);
    assert!(snapshot["last_playback_export_at"].is_string());
}
