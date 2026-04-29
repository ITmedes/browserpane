use std::io::Read;

use super::*;
use crate::session_control::SessionStore;
use crate::session_registry::SessionRegistry;

#[tokio::test]
async fn creates_lists_gets_and_stops_session_recording_metadata() {
    let (app, token) = test_router();

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
    assert_eq!(create_session_response.status(), StatusCode::CREATED);
    let session = response_json(create_session_response).await;
    let session_id = session["id"].as_str().unwrap().to_string();

    let create_recording_response = app
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
    assert_eq!(create_recording_response.status(), StatusCode::CREATED);
    let created_recording = response_json(create_recording_response).await;
    let recording_id = created_recording["id"].as_str().unwrap().to_string();
    assert_eq!(created_recording["session_id"], session_id);
    assert_eq!(created_recording["state"], "recording");
    assert_eq!(created_recording["format"], "webm");
    assert_eq!(created_recording["mime_type"], "video/webm");
    assert!(created_recording["previous_recording_id"].is_null());
    assert!(created_recording["termination_reason"].is_null());
    assert_eq!(
        created_recording["content_path"],
        format!("/api/v1/sessions/{session_id}/recordings/{recording_id}/content")
    );
    assert_eq!(created_recording["artifact_available"], false);

    let list_recordings_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/sessions/{session_id}/recordings"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_recordings_response.status(), StatusCode::OK);
    let recordings = response_json(list_recordings_response).await;
    assert_eq!(recordings["recordings"].as_array().unwrap().len(), 1);
    assert_eq!(recordings["recordings"][0]["id"], recording_id);

    let get_recording_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/sessions/{session_id}/recordings/{recording_id}"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_recording_response.status(), StatusCode::OK);
    let fetched_recording = response_json(get_recording_response).await;
    assert_eq!(fetched_recording["id"], recording_id);
    assert_eq!(fetched_recording["state"], "recording");

    let stop_recording_response = app
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
    assert_eq!(stop_recording_response.status(), StatusCode::OK);
    let stopped_recording = response_json(stop_recording_response).await;
    assert_eq!(stopped_recording["state"], "finalizing");
    assert_eq!(stopped_recording["termination_reason"], "manual_stop");

    let temp_dir = tempfile::tempdir().unwrap();
    let artifact_path = temp_dir.path().join("recording.webm");
    std::fs::write(&artifact_path, b"webm-bytes").unwrap();

    let complete_recording_response = app
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
                      "bytes": 10,
                      "duration_ms": 2500
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(complete_recording_response.status(), StatusCode::OK);
    let completed_recording = response_json(complete_recording_response).await;
    assert_eq!(completed_recording["state"], "ready");
    assert_eq!(completed_recording["artifact_available"], true);
    assert_eq!(completed_recording["bytes"], 10);
    assert_eq!(completed_recording["duration_ms"], 2500);

    let content_response = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/sessions/{session_id}/recordings/{recording_id}/content"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(content_response.status(), StatusCode::OK);
    let content_bytes = to_bytes(content_response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(content_bytes.as_ref(), b"webm-bytes");
}

#[tokio::test]
async fn recording_failure_updates_metadata_state() {
    let (app, token) = test_router();

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

    let create_recording_response = app
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
    let recording = response_json(create_recording_response).await;
    let recording_id = recording["id"].as_str().unwrap().to_string();

    let fail_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/sessions/{session_id}/recordings/{recording_id}/fail"
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
    assert_eq!(fail_response.status(), StatusCode::OK);
    let failed = response_json(fail_response).await;
    assert_eq!(failed["state"], "failed");
    assert_eq!(failed["error"], "recorder worker crashed");
    assert_eq!(failed["termination_reason"], "worker_exit");
}

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
                .uri(format!(
                    "/api/v1/sessions/{session_id}/recording-playback/manifest"
                ))
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

    let operations_response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/recording/operations")
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(operations_response.status(), StatusCode::OK);
    let operations = response_json(operations_response).await;
    assert_eq!(operations["artifact_finalize_requests_total"], 1);
    assert_eq!(operations["artifact_finalize_successes_total"], 1);
    assert_eq!(operations["artifact_finalize_failures_total"], 0);
    assert_eq!(operations["recording_failures_total"], 1);
    assert_eq!(operations["playback_manifest_requests_total"], 1);
    assert_eq!(operations["playback_export_requests_total"], 1);
    assert_eq!(operations["playback_export_successes_total"], 1);
    assert_eq!(operations["playback_export_failures_total"], 0);
    assert!(operations["playback_export_bytes_total"].as_u64().unwrap() > 0);
}

#[tokio::test]
async fn expired_recording_artifacts_return_gone() {
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
                          "format": "webm",
                          "retention_sec": 60
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

    let create_recording_response = app
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
    let recording = response_json(create_recording_response).await;
    let recording_id = recording["id"].as_str().unwrap().to_string();

    let temp_dir = tempfile::tempdir().unwrap();
    let artifact_path = temp_dir.path().join("recording.webm");
    std::fs::write(&artifact_path, b"webm-bytes").unwrap();

    let complete_recording_response = app
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
                      "bytes": 10,
                      "duration_ms": 2500
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(complete_recording_response.status(), StatusCode::OK);

    let recording_uuid = uuid::Uuid::parse_str(&recording_id).unwrap();
    let session_uuid = uuid::Uuid::parse_str(&session_id).unwrap();
    let stored = session_store
        .get_recording_for_session(session_uuid, recording_uuid)
        .await
        .unwrap()
        .unwrap();
    let retention = RecordingRetentionManager::new(
        session_store.clone(),
        artifact_store,
        Arc::new(RecordingObservability::default()),
        Duration::from_secs(60),
    );
    retention
        .run_cleanup_pass(stored.completed_at.unwrap() + chrono::Duration::seconds(61))
        .await
        .unwrap();

    let content_response = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/sessions/{session_id}/recordings/{recording_id}/content"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(content_response.status(), StatusCode::GONE);
    let body = response_json(content_response).await;
    assert!(body["error"]
        .as_str()
        .unwrap()
        .contains("no longer available"));
}
