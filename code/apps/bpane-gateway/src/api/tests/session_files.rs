use super::*;

async fn create_test_session(app: Router, token: &str) -> String {
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "labels": { "suite": "session-files" } }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    response_json(response).await["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn create_workspace_file(app: Router, token: &str) -> (String, String, Vec<u8>) {
    let create_workspace_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/file-workspaces")
                .header("authorization", bearer(token))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "name": "session-inputs" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_workspace_response.status(), StatusCode::CREATED);
    let workspace_id = response_json(create_workspace_response).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    let file_bytes = b"city,temp\nBerlin,18\n".to_vec();
    let upload_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/file-workspaces/{workspace_id}/files"))
                .header("authorization", bearer(token))
                .header("content-type", "text/csv")
                .header("x-bpane-file-name", "weather.csv")
                .body(Body::from(file_bytes.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(upload_response.status(), StatusCode::CREATED);
    let file_id = response_json(upload_response).await["id"]
        .as_str()
        .unwrap()
        .to_string();
    (workspace_id, file_id, file_bytes)
}

#[tokio::test]
async fn creates_lists_reads_and_removes_session_file_bindings() {
    let (app, token) = test_router();
    let session_id = create_test_session(app.clone(), &token).await;
    let (workspace_id, file_id, file_bytes) = create_workspace_file(app.clone(), &token).await;

    let create_binding_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/file-bindings"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "workspace_id": workspace_id,
                        "file_id": file_id,
                        "mount_path": "inputs/weather.csv",
                        "mode": "read_only",
                        "labels": {
                            "purpose": "manual-test"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_binding_response.status(), StatusCode::CREATED);
    let binding = response_json(create_binding_response).await;
    let binding_id = binding["id"].as_str().unwrap().to_string();
    assert_eq!(binding["session_id"], session_id);
    assert_eq!(binding["workspace_id"], workspace_id);
    assert_eq!(binding["file_id"], file_id);
    assert_eq!(binding["file_name"], "weather.csv");
    assert_eq!(binding["mount_path"], "inputs/weather.csv");
    assert_eq!(binding["mode"], "read_only");
    assert_eq!(binding["state"], "pending");
    assert_eq!(binding["labels"]["purpose"], "manual-test");
    assert_eq!(
        binding["content_path"],
        format!("/api/v1/sessions/{session_id}/file-bindings/{binding_id}/content")
    );

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/sessions/{session_id}/file-bindings"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    let bindings = response_json(list_response).await;
    assert_eq!(bindings["bindings"].as_array().unwrap().len(), 1);
    assert_eq!(bindings["bindings"][0]["id"], binding_id);

    let content_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/sessions/{session_id}/file-bindings/{binding_id}/content"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(content_response.status(), StatusCode::OK);
    assert_eq!(response_bytes(content_response).await, file_bytes);

    let remove_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/v1/sessions/{session_id}/file-bindings/{binding_id}"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(remove_response.status(), StatusCode::OK);
    let removed = response_json(remove_response).await;
    assert_eq!(removed["id"], binding_id);
    assert_eq!(removed["state"], "removed");

    let final_list_response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/sessions/{session_id}/file-bindings"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(final_list_response.status(), StatusCode::OK);
    assert!(response_json(final_list_response).await["bindings"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn automation_access_can_read_session_file_bindings() {
    let (app, token) = test_router();
    let session_id = create_test_session(app.clone(), &token).await;
    let (workspace_id, file_id, _) = create_workspace_file(app.clone(), &token).await;

    let create_binding_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/file-bindings"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "workspace_id": workspace_id,
                        "file_id": file_id,
                        "mount_path": "inputs/weather.csv"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_binding_response.status(), StatusCode::CREATED);

    let automation_response = app
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
    assert_eq!(automation_response.status(), StatusCode::OK);
    let automation_token = response_json(automation_response).await["token"]
        .as_str()
        .unwrap()
        .to_string();

    let list_response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/sessions/{session_id}/file-bindings"))
                .header("x-bpane-automation-access-token", automation_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    assert_eq!(
        response_json(list_response).await["bindings"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}
