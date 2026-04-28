use super::*;

#[tokio::test]
async fn workflow_runs_expose_runtime_hold_and_release_semantics() {
    let (app, token, _) = test_router_with_workflow_lifecycle(WorkflowWorkerConfig {
        docker_bin: std::path::PathBuf::from("/bin/sh"),
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
                    .body(Body::from(
                        json!({
                            "name": "Runtime Hold Workflow"
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

    response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/workflows/{workflow_id}/versions"))
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "version": "v1",
                            "executor": "manual_test",
                            "entrypoint": "workflows/runtime-hold/run.mjs"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;

    let live_hold_run = response_json(
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
                            "session": {
                                "create_session": {}
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
    assert!(
        live_hold_run.get("id").is_some(),
        "unexpected workflow run create response: {live_hold_run}"
    );
    let live_hold_run_id = live_hold_run["id"].as_str().unwrap().to_string();
    let live_hold_session_id = live_hold_run["session_id"].as_str().unwrap().to_string();

    let automation_access = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/api/v1/sessions/{live_hold_session_id}/automation-access"
                    ))
                    .header("authorization", bearer(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let automation_token = automation_access["token"].as_str().unwrap().to_string();

    let running = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/workflow-runs/{live_hold_run_id}/state"))
                .header("x-bpane-automation-access-token", &automation_token)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "state": "running",
                        "message": "executor attached"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(running.status(), StatusCode::OK);

    let awaiting_input = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/workflow-runs/{live_hold_run_id}/state"))
                    .header("x-bpane-automation-access-token", &automation_token)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "state": "awaiting_input",
                            "message": "approval required",
                            "data": {
                                "intervention_request": {
                                    "kind": "approval"
                                },
                                "runtime_hold": {
                                    "mode": "live",
                                    "timeout_sec": 1
                                }
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
    assert!(
        awaiting_input["state"].is_string(),
        "unexpected awaiting_input response: {awaiting_input}"
    );
    assert_eq!(awaiting_input["state"], "awaiting_input");

    let live_runtime = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let current = response_json(
                app.clone()
                    .oneshot(
                        Request::builder()
                            .uri(format!("/api/v1/workflow-runs/{live_hold_run_id}"))
                            .header("authorization", bearer(&token))
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap(),
            )
            .await;
            if current["runtime"]["resume_mode"] == json!("live_runtime") {
                break current;
            }
            sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("live runtime hold should become visible");
    assert_eq!(
        live_runtime["runtime"]["exact_runtime_available"],
        json!(true)
    );
    assert!(live_runtime["runtime"]["hold_until"].is_string());

    let released_live_hold = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let current = response_json(
                app.clone()
                    .oneshot(
                        Request::builder()
                            .uri(format!("/api/v1/workflow-runs/{live_hold_run_id}"))
                            .header("authorization", bearer(&token))
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap(),
            )
            .await;
            if current["runtime"]["released_at"].is_string() {
                break current;
            }
            sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("live-hold workflow run should release after timeout");
    assert_eq!(
        released_live_hold["runtime"]["resume_mode"],
        json!("profile_restart")
    );
    assert_eq!(
        released_live_hold["runtime"]["release_reason"],
        json!("hold_expired")
    );

    let released_session = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/sessions/{live_hold_session_id}"))
                    .header("authorization", bearer(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(released_session["state"], "stopped");

    let immediate_release_run = response_json(
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
                            "session": {
                                "create_session": {}
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
    let immediate_release_run_id = immediate_release_run["id"].as_str().unwrap().to_string();
    let immediate_release_session_id = immediate_release_run["session_id"]
        .as_str()
        .unwrap()
        .to_string();

    let second_automation_access = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/api/v1/sessions/{immediate_release_session_id}/automation-access"
                    ))
                    .header("authorization", bearer(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let second_automation_token = second_automation_access["token"]
        .as_str()
        .unwrap()
        .to_string();

    let second_running = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/workflow-runs/{immediate_release_run_id}/state"
                ))
                .header("x-bpane-automation-access-token", &second_automation_token)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "state": "running",
                        "message": "executor attached"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second_running.status(), StatusCode::OK);

    response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/api/v1/workflow-runs/{immediate_release_run_id}/state"
                    ))
                    .header("x-bpane-automation-access-token", &second_automation_token)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "state": "awaiting_input",
                            "message": "approval required",
                            "data": {
                                "intervention_request": {
                                    "kind": "approval"
                                }
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

    let immediately_released = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let current = response_json(
                app.clone()
                    .oneshot(
                        Request::builder()
                            .uri(format!("/api/v1/workflow-runs/{immediate_release_run_id}"))
                            .header("authorization", bearer(&token))
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap(),
            )
            .await;
            if current["runtime"]["released_at"].is_string() {
                break current;
            }
            sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("awaiting-input run without live hold should release immediately");

    assert_eq!(
        immediately_released["runtime"]["resume_mode"],
        json!("profile_restart")
    );
    assert_eq!(
        immediately_released["runtime"]["release_reason"],
        json!("awaiting_input_no_live_hold")
    );
}
