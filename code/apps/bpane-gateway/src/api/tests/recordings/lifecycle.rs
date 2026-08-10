use super::*;

#[tokio::test]
async fn updates_session_recording_policy_for_existing_session() {
    let (app, token) = test_router();

    let create_session_response = app
        .clone()
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
        .unwrap();
    assert_eq!(create_session_response.status(), StatusCode::CREATED);
    let created = response_json(create_session_response).await;
    let session_id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["recording"]["mode"], "disabled");

    let update_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/sessions/{session_id}/recording-policy"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "mode": "manual",
                        "format": "webm",
                        "retention_sec": 3600
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update_response.status(), StatusCode::OK);
    let updated = response_json(update_response).await;
    assert_eq!(updated["recording"]["mode"], "manual");
    assert_eq!(updated["recording"]["format"], "webm");
    assert_eq!(updated["recording"]["retention_sec"], 3600);

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

    let disable_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/sessions/{session_id}/recording-policy"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "mode": "disabled",
                        "format": "webm"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(disable_response.status(), StatusCode::OK);
    let disabled = response_json(disable_response).await;
    assert_eq!(disabled["recording"]["mode"], "disabled");
}

#[tokio::test]
async fn rejects_always_recording_policy_when_worker_is_not_configured() {
    let (app, token) = test_router();

    let create_session_response = app
        .clone()
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
        .unwrap();
    assert_eq!(create_session_response.status(), StatusCode::CREATED);
    let created = response_json(create_session_response).await;
    let session_id = created["id"].as_str().unwrap().to_string();

    let update_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/sessions/{session_id}/recording-policy"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "mode": "always",
                        "format": "webm"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update_response.status(), StatusCode::CONFLICT);
    let error = response_json(update_response).await;
    assert!(error["error"]
        .as_str()
        .unwrap()
        .contains("recording mode=always requires a configured recording worker"));

    let load_response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/sessions/{session_id}"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(load_response.status(), StatusCode::OK);
    let loaded = response_json(load_response).await;
    assert_eq!(loaded["recording"]["mode"], "disabled");
}

#[tokio::test]
async fn stores_always_recording_policy_for_stopped_session_without_worker() {
    let (app, token) = test_router();

    let create_session_response = app
        .clone()
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
        .unwrap();
    assert_eq!(create_session_response.status(), StatusCode::CREATED);
    let created = response_json(create_session_response).await;
    let session_id = created["id"].as_str().unwrap().to_string();

    let stop_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/stop"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stop_response.status(), StatusCode::OK);
    let stopped = response_json(stop_response).await;
    assert_eq!(stopped["state"], "stopped");

    let update_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/sessions/{session_id}/recording-policy"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "mode": "always",
                        "format": "webm",
                        "retention_sec": 7200
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update_response.status(), StatusCode::OK);
    let updated = response_json(update_response).await;
    assert_eq!(updated["state"], "stopped");
    assert_eq!(updated["recording"]["mode"], "always");
    assert_eq!(updated["recording"]["format"], "webm");
    assert_eq!(updated["recording"]["retention_sec"], 7200);
}

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
                .header(
                    RECORDING_WORKER_ACCESS_TOKEN_HEADER,
                    recording_worker_access_token(&session_id, &recording_id),
                )
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
async fn automation_access_cannot_finalize_or_fail_session_recordings() {
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

    let automation_access_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/automation-access"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(automation_access_response.status(), StatusCode::OK);
    let automation_access = response_json(automation_access_response).await;
    let automation_token = automation_access["token"].as_str().unwrap().to_string();

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

    let list_recordings_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/sessions/{session_id}/recordings"))
                .header("x-bpane-automation-access-token", automation_token.as_str())
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
                .header("x-bpane-automation-access-token", automation_token.as_str())
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

    let temp_dir = tempfile::tempdir().unwrap();
    let artifact_path = temp_dir.path().join("recording.webm");
    std::fs::write(&artifact_path, b"automation-webm").unwrap();

    let complete_recording_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/sessions/{session_id}/recordings/{recording_id}/complete"
                ))
                .header("x-bpane-automation-access-token", automation_token.as_str())
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                      "source_path": artifact_path.to_string_lossy(),
                      "mime_type": "video/webm",
                      "bytes": 15,
                      "duration_ms": 3000
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        complete_recording_response.status(),
        StatusCode::UNAUTHORIZED
    );
    assert!(artifact_path.exists());

    let fail_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/sessions/{session_id}/recordings/{recording_id}/fail"
                ))
                .header("x-bpane-automation-access-token", automation_token.as_str())
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
    assert_eq!(fail_response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn recording_worker_access_must_match_session_and_recording() {
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

    let stop_response = app
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
    assert_eq!(stop_response.status(), StatusCode::OK);

    let temp_dir = tempfile::tempdir().unwrap();
    let artifact_path = temp_dir.path().join("recording.webm");
    std::fs::write(&artifact_path, b"worker-scoped-webm").unwrap();
    let completion_body = json!({
        "source_path": artifact_path.to_string_lossy(),
        "mime_type": "video/webm",
        "bytes": 18,
        "duration_ms": 2500
    })
    .to_string();

    for worker_token in [
        "not-a-worker-token".to_string(),
        recording_worker_access_token(Uuid::now_v7(), &recording_id),
        recording_worker_access_token(&session_id, Uuid::now_v7()),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/api/v1/sessions/{session_id}/recordings/{recording_id}/complete"
                    ))
                    .header(RECORDING_WORKER_ACCESS_TOKEN_HEADER, worker_token)
                    .header("content-type", "application/json")
                    .body(Body::from(completion_body.clone()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(artifact_path.exists());
    }
}

#[tokio::test]
async fn recording_worker_cannot_complete_an_active_recording() {
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
    let temp_dir = tempfile::tempdir().unwrap();
    let artifact_path = temp_dir.path().join("recording.webm");
    std::fs::write(&artifact_path, b"active-recording").unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/sessions/{session_id}/recordings/{recording_id}/complete"
                ))
                .header(
                    RECORDING_WORKER_ACCESS_TOKEN_HEADER,
                    recording_worker_access_token(&session_id, &recording_id),
                )
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "source_path": artifact_path.to_string_lossy(),
                        "mime_type": "video/webm",
                        "bytes": 16,
                        "duration_ms": 1500
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert!(artifact_path.exists());
}

#[tokio::test]
async fn automation_access_must_match_recording_session() {
    let (app, token) = test_router();

    let first_session_response = app
        .clone()
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
        .unwrap();
    assert_eq!(first_session_response.status(), StatusCode::CREATED);
    let first_session = response_json(first_session_response).await;
    let first_session_id = first_session["id"].as_str().unwrap().to_string();

    let automation_access_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/sessions/{first_session_id}/automation-access"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(automation_access_response.status(), StatusCode::OK);
    let automation_access = response_json(automation_access_response).await;
    let automation_token = automation_access["token"].as_str().unwrap().to_string();
    let other_session_id = uuid::Uuid::now_v7();

    let mismatched_list_response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/sessions/{other_session_id}/recordings"))
                .header("x-bpane-automation-access-token", automation_token.as_str())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(mismatched_list_response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn project_policy_blocks_manual_recording_and_reports_file_capability() {
    let (app, token) = test_router();

    let project_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "recording-policy",
                        "policy": {
                            "allow_browser_uploads": false,
                            "allow_manual_recordings": false
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(project_response.status(), StatusCode::CREATED);
    let project_id = response_json(project_response).await["id"]
        .as_str()
        .unwrap()
        .to_string();

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
                        "project_id": project_id,
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
    assert_eq!(session["capabilities"]["file_transfer"], false);

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
    assert_eq!(create_recording_response.status(), StatusCode::CONFLICT);
    let error = response_json(create_recording_response).await;
    assert!(error["error"]
        .as_str()
        .unwrap()
        .contains("manual_recording_not_allowed"));

    let stop_session_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/stop"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stop_session_response.status(), StatusCode::OK);

    let binding_policy_project_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "binding-policy",
                        "policy": {
                            "allow_session_file_bindings": false
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        binding_policy_project_response.status(),
        StatusCode::CREATED
    );
    let binding_policy_project_id = response_json(binding_policy_project_response).await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let binding_policy_session_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "project_id": binding_policy_project_id
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        binding_policy_session_response.status(),
        StatusCode::CREATED
    );
    let binding_policy_session = response_json(binding_policy_session_response).await;
    assert_eq!(
        binding_policy_session["capabilities"]["file_transfer"],
        true
    );
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
                .header(
                    RECORDING_WORKER_ACCESS_TOKEN_HEADER,
                    recording_worker_access_token(&session_id, &recording_id),
                )
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
