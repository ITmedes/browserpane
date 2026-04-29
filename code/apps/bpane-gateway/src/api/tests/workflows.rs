use super::*;

#[tokio::test]
async fn workflow_run_create_supports_external_correlation_and_safe_idempotent_retry() {
    let (app, token) = test_router();

    let workflow = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/workflows")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": "idempotent-workflow"
                        })
                        .to_string(),
                    ))
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
                        "entrypoint": "workflows/idempotent/run.mjs"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_version.status(), StatusCode::CREATED);

    let request_body = json!({
        "workflow_id": workflow_id,
        "version": "v1",
        "session": {
            "create_session": {}
        },
        "source_system": "camunda-prod",
        "source_reference": "process-instance-123/task-7",
        "client_request_id": "job-123-attempt-1",
        "input": {
            "customer_id": "cust-42"
        },
        "labels": {
            "suite": "contract"
        }
    });

    let first_create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workflow-runs")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(request_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first_create.status(), StatusCode::CREATED);
    let first_run = response_json(first_create).await;
    let run_id = first_run["id"].as_str().unwrap().to_string();
    let session_id = first_run["session_id"].as_str().unwrap().to_string();
    let task_id = first_run["automation_task_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(first_run["source_system"], "camunda-prod");
    assert_eq!(first_run["source_reference"], "process-instance-123/task-7");
    assert_eq!(first_run["client_request_id"], "job-123-attempt-1");

    let second_create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workflow-runs")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(request_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let second_status = second_create.status();
    let second_run = response_json(second_create).await;
    assert_eq!(second_status, StatusCode::OK, "{second_run:#}");
    assert_eq!(second_run["id"], run_id);
    assert_eq!(second_run["session_id"], session_id);
    assert_eq!(second_run["automation_task_id"], task_id);
    assert_eq!(second_run["source_system"], "camunda-prod");
    assert_eq!(
        second_run["source_reference"],
        "process-instance-123/task-7"
    );
    assert_eq!(second_run["client_request_id"], "job-123-attempt-1");

    let run_events = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/workflow-runs/{run_id}/events"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(run_events.status(), StatusCode::OK);
    let events = response_json(run_events).await;
    let created_count = events["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|event| event["event_type"] == "workflow_run.created")
        .count();
    assert_eq!(created_count, 1);
}

#[tokio::test]
async fn workflow_run_create_rejects_conflicting_idempotent_retry() {
    let (app, token) = test_router();

    let workflow = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/workflows")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": "idempotent-conflict"
                        })
                        .to_string(),
                    ))
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
                        "entrypoint": "workflows/idempotent/run.mjs"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_version.status(), StatusCode::CREATED);

    let base_request = json!({
        "workflow_id": workflow_id,
        "version": "v1",
        "session": {
            "create_session": {}
        },
        "source_system": "camunda-prod",
        "source_reference": "task-1",
        "client_request_id": "job-999-attempt-1",
        "input": {
            "customer_id": "cust-42"
        }
    });
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workflow-runs")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(base_request.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);

    let conflicting_request = json!({
        "workflow_id": workflow_id,
        "version": "v1",
        "session": {
            "create_session": {}
        },
        "source_system": "camunda-prod",
        "source_reference": "task-2",
        "client_request_id": "job-999-attempt-1",
        "input": {
            "customer_id": "cust-77"
        }
    });
    let conflicting_create = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workflow-runs")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(conflicting_request.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let conflicting_status = conflicting_create.status();
    let body = response_json(conflicting_create).await;
    assert_eq!(conflicting_status, StatusCode::CONFLICT, "{body:#}");
    assert!(body["error"]
        .as_str()
        .unwrap()
        .contains("client_request_id"));
}

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
