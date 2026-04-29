use super::*;

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
