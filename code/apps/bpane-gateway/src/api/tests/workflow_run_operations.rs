use super::*;

#[tokio::test]
async fn workflow_run_owner_can_cancel_queued_run_before_dispatch() {
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
                    .body(Body::from(
                        json!({
                            "name": "queued-cancel"
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
                        "entrypoint": "workflows/queued-cancel/run.mjs",
                        "default_session": {
                            "labels": {
                                "suite": "queued-cancel"
                            }
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_version.status(), StatusCode::CREATED);

    let active_run = response_json(
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
    let active_run_id = active_run["id"].as_str().unwrap().to_string();

    let queued_run = response_json(
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
    let queued_run_id = queued_run["id"].as_str().unwrap().to_string();
    assert_eq!(queued_run["state"], "queued");
    assert_eq!(queued_run["admission"]["state"], "queued");

    let cancel_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/workflow-runs/{queued_run_id}/cancel"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cancel_response.status(), StatusCode::OK);
    let cancelled = response_json(cancel_response).await;
    assert_eq!(cancelled["state"], "cancelled");

    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let first = response_json(
                app.clone()
                    .oneshot(
                        Request::builder()
                            .uri(format!("/api/v1/workflow-runs/{active_run_id}"))
                            .header("authorization", bearer(&token))
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap(),
            )
            .await;
            if first["state"] == "failed" {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("active run should finish");

    tokio::time::sleep(Duration::from_millis(250)).await;

    let stable_cancelled = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/workflow-runs/{queued_run_id}"))
                    .header("authorization", bearer(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(stable_cancelled["state"], "cancelled");

    let queued_events = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/workflow-runs/{queued_run_id}/events"))
                    .header("authorization", bearer(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let event_types = queued_events["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["event_type"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert!(event_types.contains(&"workflow_run.queued".to_string()));
    assert!(event_types.contains(&"workflow_run.cancel_requested".to_string()));
    assert!(event_types.contains(&"workflow_run.cancelled".to_string()));
    assert!(!event_types.contains(&"workflow_run.running".to_string()));
    assert!(!event_types.contains(&"automation_task.running".to_string()));
    assert!(!event_types.contains(&"workflow_run.succeeded".to_string()));

    let capture = fs::read_to_string(&capture_file).unwrap();
    assert!(capture.contains(&active_run_id));
    assert!(!capture.contains(&queued_run_id));
}

#[tokio::test]
async fn workflow_run_owner_can_submit_input_resume_and_reject_interventions() {
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
                            "name": "operator-intervention"
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
                        "entrypoint": "workflows/operator/run.mjs"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_version.status(), StatusCode::CREATED);

    let run = response_json(
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
    let run_id = run["id"].as_str().unwrap().to_string();
    let session_id = run["session_id"].as_str().unwrap().to_string();

    let automation_access = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/sessions/{session_id}/automation-access"))
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
                .uri(format!("/api/v1/workflow-runs/{run_id}/state"))
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

    let first_request_id = Uuid::now_v7();
    let awaiting_input = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/workflow-runs/{run_id}/state"))
                    .header("x-bpane-automation-access-token", &automation_token)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "state": "awaiting_input",
                            "message": "approval required",
                            "data": {
                                "intervention_request": {
                                    "request_id": first_request_id,
                                    "kind": "approval",
                                    "prompt": "Approve payout export",
                                    "details": {
                                        "step": "review"
                                    }
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
    assert_eq!(
        awaiting_input["intervention"]["pending_request"]["request_id"],
        first_request_id.to_string()
    );
    assert_eq!(
        awaiting_input["intervention"]["pending_request"]["kind"],
        "approval"
    );

    let submitted = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/workflow-runs/{run_id}/submit-input"))
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "input": {
                                "approved": true
                            },
                            "comment": "operator approved"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(submitted["state"], "running");
    assert!(submitted["intervention"]["pending_request"].is_null());
    assert_eq!(
        submitted["intervention"]["last_resolution"]["action"],
        "submit_input"
    );
    assert_eq!(
        submitted["intervention"]["last_resolution"]["request_id"],
        first_request_id.to_string()
    );
    assert_eq!(
        submitted["intervention"]["last_resolution"]["input"],
        json!({ "approved": true })
    );

    let second_request_id = Uuid::now_v7();
    let awaiting_resume = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/workflow-runs/{run_id}/state"))
                    .header("x-bpane-automation-access-token", &automation_token)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "state": "awaiting_input",
                            "message": "resume required",
                            "data": {
                                "intervention_request": {
                                    "request_id": second_request_id,
                                    "kind": "confirmation",
                                    "prompt": "Resume the run"
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
    assert_eq!(awaiting_resume["state"], "awaiting_input");
    assert_eq!(
        awaiting_resume["intervention"]["pending_request"]["request_id"],
        second_request_id.to_string()
    );

    let resumed = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/workflow-runs/{run_id}/resume"))
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "comment": "operator resumed"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(resumed["state"], "running");
    assert_eq!(
        resumed["intervention"]["last_resolution"]["action"],
        "resume"
    );
    assert_eq!(
        resumed["intervention"]["last_resolution"]["request_id"],
        second_request_id.to_string()
    );

    let third_request_id = Uuid::now_v7();
    let awaiting_reject = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/workflow-runs/{run_id}/state"))
                    .header("x-bpane-automation-access-token", &automation_token)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "state": "awaiting_input",
                            "message": "approval required again",
                            "data": {
                                "intervention_request": {
                                    "request_id": third_request_id,
                                    "kind": "approval",
                                    "prompt": "Reject this run"
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
    assert_eq!(awaiting_reject["state"], "awaiting_input");

    let rejected = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/workflow-runs/{run_id}/reject"))
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "reason": "operator denied approval"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(rejected["state"], "failed");
    assert_eq!(rejected["error"], "operator denied approval");
    assert_eq!(
        rejected["intervention"]["last_resolution"]["action"],
        "reject"
    );
    assert_eq!(
        rejected["intervention"]["last_resolution"]["request_id"],
        third_request_id.to_string()
    );
    assert_eq!(
        rejected["intervention"]["last_resolution"]["reason"],
        "operator denied approval"
    );

    let events = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/workflow-runs/{run_id}/events"))
                    .header("authorization", bearer(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let event_types = events["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["event_type"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert!(event_types.contains(&"workflow_run.input_submitted".to_string()));
    assert!(event_types.contains(&"workflow_run.resumed".to_string()));
    assert!(event_types.contains(&"workflow_run.rejected".to_string()));
}

#[tokio::test]
async fn workflow_runs_can_be_cancelled_and_surface_task_logs() {
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
                            "name": "demo-workflow"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let workflow_id = workflow["id"].as_str().unwrap();

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
                        "entrypoint": "workflows/demo.ts",
                        "default_session": {}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_version.status(), StatusCode::CREATED);

    let run = response_json(
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
                            "version": "v1"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let run_id = run["id"].as_str().unwrap().to_string();

    let cancel_run = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/workflow-runs/{run_id}/cancel"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cancel_run.status(), StatusCode::OK);
    let cancelled = response_json(cancel_run).await;
    assert_eq!(cancelled["state"], "cancelled");

    let events = app
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
    assert_eq!(events.status(), StatusCode::OK);
    let events_body = response_json(events).await;
    assert_eq!(events_body["events"].as_array().unwrap().len(), 5);
    let event_types = events_body["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["event_type"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert!(event_types.contains(&"workflow_run.created".to_string()));
    assert!(event_types.contains(&"automation_task.created".to_string()));
    assert!(event_types.contains(&"workflow_run.cancel_requested".to_string()));
    assert!(event_types.contains(&"workflow_run.cancelled".to_string()));
    assert!(event_types.contains(&"automation_task.cancelled".to_string()));

    let logs = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/workflow-runs/{run_id}/logs"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(logs.status(), StatusCode::OK);
    let logs_body = response_json(logs).await;
    let logs = logs_body["logs"].as_array().unwrap();
    assert_eq!(logs.len(), 2);
    assert!(logs.iter().all(|log| log["stream"] == "system"));
    assert!(logs
        .iter()
        .any(|log| log["message"] == "workflow run cancelled"));
}
