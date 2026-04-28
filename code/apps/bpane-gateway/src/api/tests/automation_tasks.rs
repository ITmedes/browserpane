use super::*;

#[tokio::test]
async fn creates_lists_gets_and_cancels_automation_tasks_for_existing_sessions() {
    let (app, token) = test_router();

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

    let create_task = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/automation-tasks")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "display_name": "Smoke task",
                        "executor": "playwright",
                        "session": {
                            "existing_session_id": session_id
                        },
                        "input": {
                            "step": "open_dashboard"
                        },
                        "labels": {
                            "suite": "contract"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_task.status(), StatusCode::CREATED);
    let task = response_json(create_task).await;
    let task_id = task["id"].as_str().unwrap().to_string();
    assert_eq!(task["display_name"], "Smoke task");
    assert_eq!(task["executor"], "playwright");
    assert_eq!(task["state"], "pending");
    assert_eq!(task["session"]["source"], "existing_session");
    assert_eq!(task["session"]["session_id"], session_id);
    assert_eq!(task["labels"]["suite"], "contract");
    assert_eq!(task["input"]["step"], "open_dashboard");

    let list_tasks = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/automation-tasks")
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_tasks.status(), StatusCode::OK);
    let listed = response_json(list_tasks).await;
    assert_eq!(listed["tasks"].as_array().unwrap().len(), 1);
    assert_eq!(listed["tasks"][0]["id"], task_id);

    let get_task = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/automation-tasks/{task_id}"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_task.status(), StatusCode::OK);

    let initial_events = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/automation-tasks/{task_id}/events"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(initial_events.status(), StatusCode::OK);
    let initial_events_body = response_json(initial_events).await;
    assert_eq!(initial_events_body["events"].as_array().unwrap().len(), 1);
    assert_eq!(
        initial_events_body["events"][0]["event_type"],
        "automation_task.created"
    );

    let cancel_task = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/automation-tasks/{task_id}/cancel"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cancel_task.status(), StatusCode::OK);
    let cancelled = response_json(cancel_task).await;
    assert_eq!(cancelled["state"], "cancelled");
    assert!(cancelled["cancel_requested_at"].is_string());
    assert!(cancelled["completed_at"].is_string());

    let logs = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/automation-tasks/{task_id}/logs"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(logs.status(), StatusCode::OK);
    let logs_body = response_json(logs).await;
    assert_eq!(logs_body["logs"].as_array().unwrap().len(), 1);
    assert_eq!(logs_body["logs"][0]["stream"], "system");

    let events = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/automation-tasks/{task_id}/events"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(events.status(), StatusCode::OK);
    let events_body = response_json(events).await;
    assert_eq!(events_body["events"].as_array().unwrap().len(), 2);
    assert_eq!(
        events_body["events"][1]["event_type"],
        "automation_task.cancelled"
    );
}

#[tokio::test]
async fn automation_tasks_can_create_their_own_session_binding() {
    let (app, token) = test_router();

    let create_task = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/automation-tasks")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "display_name": "Bootstrap task",
                        "executor": "playwright",
                        "session": {
                            "create_session": {
                                "labels": {
                                    "origin": "automation-task"
                                }
                            }
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_task.status(), StatusCode::CREATED);
    let task = response_json(create_task).await;
    let session_id = task["session"]["session_id"].as_str().unwrap().to_string();
    assert_eq!(task["session"]["source"], "created_session");
    assert_eq!(task["state"], "pending");

    let get_session = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/sessions/{session_id}"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_session.status(), StatusCode::OK);
    let session = response_json(get_session).await;
    assert_eq!(session["labels"]["origin"], "automation-task");
}

#[tokio::test]
async fn automation_access_token_can_update_automation_task_state_and_logs() {
    let (app, token) = test_router();

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

    let task = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/automation-tasks")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "display_name": "Executor task",
                            "executor": "playwright",
                            "session": {
                                "existing_session_id": session_id
                            },
                            "input": {
                                "step": "bootstrap"
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
    let task_id = task["id"].as_str().unwrap().to_string();

    let running = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/automation-tasks/{task_id}/state"))
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

    let log_append = app
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
                        "message": "opened dashboard"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(log_append.status(), StatusCode::OK);

    let succeeded = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/automation-tasks/{task_id}/state"))
                .header("x-bpane-automation-access-token", &automation_token)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "state": "succeeded",
                        "output": {
                            "result": "ok"
                        },
                        "artifact_refs": ["artifact://trace.zip"],
                        "message": "executor finished"
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
    assert_eq!(succeeded_body["output"]["result"], "ok");
    assert_eq!(succeeded_body["artifact_refs"][0], "artifact://trace.zip");

    let fetched = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/automation-tasks/{task_id}"))
                .header("x-bpane-automation-access-token", &automation_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(fetched.status(), StatusCode::OK);
    let fetched_body = response_json(fetched).await;
    assert_eq!(fetched_body["state"], "succeeded");
    assert_eq!(fetched_body["output"]["result"], "ok");

    let events = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/automation-tasks/{task_id}/events"))
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
    assert!(event_types.contains(&"automation_task.created".to_string()));
    assert!(event_types.contains(&"automation_task.running".to_string()));
    assert!(event_types.contains(&"automation_task.succeeded".to_string()));

    let logs = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/automation-tasks/{task_id}/logs"))
                .header("x-bpane-automation-access-token", &automation_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(logs.status(), StatusCode::OK);
    let logs_body = response_json(logs).await;
    assert_eq!(logs_body["logs"].as_array().unwrap().len(), 1);
    assert_eq!(logs_body["logs"][0]["stream"], "stdout");
    assert_eq!(logs_body["logs"][0]["message"], "opened dashboard");
}
