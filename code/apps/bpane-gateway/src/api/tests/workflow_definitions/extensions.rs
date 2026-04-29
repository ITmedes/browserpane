use super::*;

#[tokio::test]
async fn workflow_runs_inherit_session_extensions() {
    let (app, token) = test_router_with_docker_pool().await;

    let create_extension_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/extensions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "workflow-extension",
                        "description": "Approved workflow extension",
                        "labels": { "suite": "contract" }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_extension_response.status(), StatusCode::CREATED);
    let extension = response_json(create_extension_response).await;
    let extension_id = extension["id"].as_str().unwrap().to_string();

    let create_version_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/extensions/{extension_id}/versions"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "version": "1.0.0",
                        "install_path": "/home/bpane/bpane-test-extension"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_version_response.status(), StatusCode::CREATED);

    let create_workflow_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workflows")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "extension-smoke",
                        "description": "Workflow extension test",
                        "labels": { "suite": "contract" }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_workflow_response.status(), StatusCode::CREATED);
    let workflow = response_json(create_workflow_response).await;
    let workflow_id = workflow["id"].as_str().unwrap().to_string();

    let create_workflow_version_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/workflows/{workflow_id}/versions"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "version": "v1",
                        "executor": "playwright",
                        "entrypoint": "workflows/extensions/run.mjs",
                        "default_session": {
                            "extension_ids": [extension_id]
                        },
                        "allowed_extension_ids": [extension_id]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        create_workflow_version_response.status(),
        StatusCode::CREATED
    );

    let create_run_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workflow-runs")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "workflow_id": workflow_id,
                        "version": "v1",
                        "input": {
                            "suite": "contract"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_run_response.status(), StatusCode::CREATED);
    let run = response_json(create_run_response).await;
    let session_id = run["session_id"].as_str().unwrap().to_string();
    assert_eq!(run["extensions"].as_array().unwrap().len(), 1);
    assert_eq!(run["extensions"][0]["extension_id"], extension_id);

    let session_response = app
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
    assert_eq!(session_response.status(), StatusCode::OK);
    let session = response_json(session_response).await;
    assert_eq!(session["extensions"].as_array().unwrap().len(), 1);
    assert_eq!(session["extensions"][0]["extension_id"], extension_id);
}
