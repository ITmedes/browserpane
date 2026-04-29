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
