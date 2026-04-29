use super::*;

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
