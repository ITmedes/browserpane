use super::super::*;

#[tokio::test]
async fn workflow_run_create_exposes_queued_admission_when_worker_capacity_is_exhausted() {
    let temp_dir = tempdir().unwrap();
    let capture_file = temp_dir.path().join("workflow-worker-capture.txt");
    let script = create_sleep_workflow_worker_script(&temp_dir, &capture_file, 0.3);
    let (app, token, _state) = test_router_with_workflow_lifecycle(WorkflowWorkerConfig {
        docker_bin: script,
        image: "deploy-workflow-worker:test".to_string(),
        max_active_workers: 1,
        network: Some("deploy_bpane-internal".to_string()),
        container_name_prefix: "bpane-workflow".to_string(),
        gateway_api_url: "http://gateway:8932".to_string(),
        work_root: std::path::PathBuf::from("/tmp/bpane-workflows"),
        bearer_token: Some("token".to_string()),
        oidc_token_url: None,
        oidc_client_id: None,
        oidc_client_secret: None,
        oidc_scopes: None,
    });

    let workflow = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/workflows")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({ "name": "queued-workflow" }).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let workflow_id = workflow["id"].as_str().unwrap().to_string();

    let create_version = app
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
                        "entrypoint": "workflows/queued/run.mjs"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_version.status(), StatusCode::CREATED);

    let first_run = response_json(
        app.clone()
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
                            "session": { "create_session": {} }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let first_run_state = first_run["state"].as_str().unwrap();
    assert!(
        matches!(first_run_state, "pending" | "starting" | "running"),
        "unexpected first run state: {first_run_state}"
    );

    let second_response = app
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
                        "session": { "create_session": {} }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second_response.status(), StatusCode::CREATED);
    let second_run = response_json(second_response).await;
    assert_eq!(second_run["state"], "queued");
    assert_eq!(second_run["admission"]["state"], "queued");
    assert_eq!(
        second_run["admission"]["reason"],
        "workflow_worker_capacity"
    );
}
