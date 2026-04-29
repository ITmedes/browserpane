use super::*;

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
