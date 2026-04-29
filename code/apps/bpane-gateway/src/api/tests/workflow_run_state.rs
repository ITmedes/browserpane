use super::*;

#[tokio::test]
async fn automation_access_token_can_update_workflow_run_state_logs_and_outputs() {
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
                            "name": "stateful-workflow"
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
                        "entrypoint": "workflows/stateful.ts"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_version.status(), StatusCode::CREATED);

    let session = response_json(
        app.clone()
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
            .unwrap(),
    )
    .await;
    let session_id = session["id"].as_str().unwrap().to_string();

    let issued = response_json(
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
    let automation_token = issued["token"].as_str().unwrap().to_string();

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
                                "existing_session_id": session_id
                            },
                            "input": {
                                "month": "2026-03"
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
    let task_id = run["automation_task_id"].as_str().unwrap().to_string();

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
                        "message": "workflow executor attached"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(running.status(), StatusCode::OK);

    let run_log = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/workflow-runs/{run_id}/logs"))
                .header("x-bpane-automation-access-token", &automation_token)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "stream": "system",
                        "message": "workflow bootstrapped"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(run_log.status(), StatusCode::OK);
    let run_log_body = response_json(run_log).await;
    assert_eq!(run_log_body["source"], "run");

    let task_log = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/automation-tasks/{task_id}/logs"))
                .header("x-bpane-automation-access-token", &automation_token)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "stream": "stdout",
                        "message": "opened report page"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(task_log.status(), StatusCode::OK);

    let succeeded = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/workflow-runs/{run_id}/state"))
                .header("x-bpane-automation-access-token", &automation_token)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "state": "succeeded",
                        "output": {
                            "csv_file_id": "file_123"
                        },
                        "artifact_refs": ["artifact://workflow-trace.zip"],
                        "message": "workflow completed"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(succeeded.status(), StatusCode::OK);
    let succeeded_body = response_json(succeeded).await;
    assert_eq!(succeeded_body["state"], "succeeded");
    assert_eq!(succeeded_body["output"]["csv_file_id"], "file_123");
    assert_eq!(
        succeeded_body["artifact_refs"][0],
        "artifact://workflow-trace.zip"
    );

    let fetched = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/workflow-runs/{run_id}"))
                .header("x-bpane-automation-access-token", &automation_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(fetched.status(), StatusCode::OK);
    let fetched_body = response_json(fetched).await;
    assert_eq!(fetched_body["state"], "succeeded");
    assert_eq!(fetched_body["output"]["csv_file_id"], "file_123");

    let events = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/workflow-runs/{run_id}/events"))
                .header("x-bpane-automation-access-token", &automation_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(events.status(), StatusCode::OK);
    let events_body = response_json(events).await;
    let event_types = events_body["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["event_type"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert!(event_types.contains(&"workflow_run.created".to_string()));
    assert!(event_types.contains(&"automation_task.created".to_string()));
    assert!(event_types.contains(&"workflow_run.running".to_string()));
    assert!(event_types.contains(&"automation_task.running".to_string()));
    assert!(event_types.contains(&"workflow_run.succeeded".to_string()));
    assert!(event_types.contains(&"automation_task.succeeded".to_string()));

    let logs = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/workflow-runs/{run_id}/logs"))
                .header("x-bpane-automation-access-token", &automation_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(logs.status(), StatusCode::OK);
    let logs_body = response_json(logs).await;
    let sources = logs_body["logs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|log| log["source"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert!(sources.contains(&"run".to_string()));
    assert!(sources.contains(&"automation_task".to_string()));
}
