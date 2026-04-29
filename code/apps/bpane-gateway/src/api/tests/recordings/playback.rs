use std::io::Read;

use super::*;

#[tokio::test]
async fn playback_manifest_and_export_bundle_follow_ready_segments() {
    let auth_validator = Arc::new(AuthValidator::from_hmac_secret(vec![7; 32]));
    let token = auth_validator.generate_token().unwrap();
    let session_store = SessionStore::in_memory();
    let artifact_store = test_artifact_store();
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
        recording_artifact_store: artifact_store.clone(),
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

    let create_session_response = app
        .clone()
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
        .unwrap();
    let session = response_json(create_session_response).await;
    let session_id = session["id"].as_str().unwrap().to_string();

    let create_first_recording = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/recordings"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let first_recording = response_json(create_first_recording).await;
    let first_recording_id = first_recording["id"].as_str().unwrap().to_string();

    let stop_first_recording = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/sessions/{session_id}/recordings/{first_recording_id}/stop"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stop_first_recording.status(), StatusCode::OK);

    let temp_dir = tempfile::tempdir().unwrap();
    let artifact_path = temp_dir.path().join("segment-1.webm");
    std::fs::write(&artifact_path, b"segment-one").unwrap();
    let complete_first_recording = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/sessions/{session_id}/recordings/{first_recording_id}/complete"
                ))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "source_path": artifact_path.to_string_lossy(),
                        "mime_type": "video/webm",
                        "bytes": 11,
                        "duration_ms": 900
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(complete_first_recording.status(), StatusCode::OK);

    let create_second_recording = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/recordings"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let second_recording = response_json(create_second_recording).await;
    let second_recording_id = second_recording["id"].as_str().unwrap().to_string();

    let fail_second_recording = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/sessions/{session_id}/recordings/{second_recording_id}/fail"
                ))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "error": "recorder worker crashed",
                        "termination_reason": "worker_exit"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(fail_second_recording.status(), StatusCode::OK);

    let playback_response = app
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
    assert_eq!(playback_response.status(), StatusCode::OK);
    let playback = response_json(playback_response).await;
    assert_eq!(playback["state"], "partial");
    assert_eq!(playback["segment_count"], 2);
    assert_eq!(playback["included_segment_count"], 1);
    assert_eq!(playback["failed_segment_count"], 1);
    assert_eq!(playback["active_segment_count"], 0);
    assert_eq!(playback["missing_artifact_segment_count"], 0);

    let manifest_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/sessions/{session_id}/recording-playback/manifest"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(manifest_response.status(), StatusCode::OK);
    let manifest = response_json(manifest_response).await;
    assert_eq!(
        manifest["format_version"],
        "browserpane_recording_playback_v1"
    );
    assert_eq!(manifest["segments"].as_array().unwrap().len(), 1);
    assert_eq!(manifest["omitted_segments"].as_array().unwrap().len(), 1);
    assert_eq!(manifest["omitted_segments"][0]["omitted_reason"], "failed");

    let export_response = app
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
    assert_eq!(export_response.status(), StatusCode::OK);
    assert_eq!(
        export_response
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "application/zip"
    );
    let export_bytes = to_bytes(export_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let cursor = std::io::Cursor::new(export_bytes.to_vec());
    let mut archive = zip::ZipArchive::new(cursor).unwrap();
    let mut manifest_file = archive.by_name("manifest.json").unwrap();
    let mut manifest_bytes = Vec::new();
    manifest_file.read_to_end(&mut manifest_bytes).unwrap();
    drop(manifest_file);
    let manifest_json: Value = serde_json::from_slice(&manifest_bytes).unwrap();
    assert_eq!(manifest_json["segment_count"], 2);
    assert!(archive.by_name("player.html").is_ok());
    let segment_name = manifest_json["segments"][0]["file_name"].as_str().unwrap();
    let mut segment_file = archive.by_name(segment_name).unwrap();
    let mut segment_bytes = Vec::new();
    segment_file.read_to_end(&mut segment_bytes).unwrap();
    assert_eq!(segment_bytes.as_slice(), b"segment-one");
}
