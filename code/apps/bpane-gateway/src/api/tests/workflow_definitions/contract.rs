use super::*;

#[tokio::test]
async fn creates_workflow_definitions_versions_and_runs_with_default_sessions() {
    let (app, token) = test_router();

    let create_workflow = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workflows")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "stripe-monthly-export",
                        "description": "Export monthly payout reports",
                        "labels": {
                            "team": "finance"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_workflow.status(), StatusCode::CREATED);
    let workflow = response_json(create_workflow).await;
    let workflow_id = workflow["id"].as_str().unwrap().to_string();
    assert_eq!(workflow["latest_version"], Value::Null);

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
                        "entrypoint": "workflows/stripe/export-payouts.ts",
                        "input_schema": {
                            "type": "object",
                            "required": ["month"]
                        },
                        "output_schema": {
                            "type": "object",
                            "required": ["csv_file_id"]
                        },
                        "default_session": {
                            "labels": {
                                "origin": "workflow-run"
                            }
                        },
                        "allowed_credential_binding_ids": ["cred_stripe_prod"],
                        "allowed_file_workspace_ids": ["ws_finance_reports"]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_version.status(), StatusCode::CREATED);
    let version = response_json(create_version).await;
    assert_eq!(version["version"], "v1");
    assert_eq!(version["executor"], "playwright");

    let get_workflow = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/workflows/{workflow_id}"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_workflow.status(), StatusCode::OK);
    let workflow_body = response_json(get_workflow).await;
    assert_eq!(workflow_body["latest_version"], "v1");

    let get_version = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/workflows/{workflow_id}/versions/v1"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_version.status(), StatusCode::OK);

    let create_run = app
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
                            "month": "2026-03",
                            "country_code": "DE"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_run.status(), StatusCode::CREATED);
    let run = response_json(create_run).await;
    let run_id = run["id"].as_str().unwrap().to_string();
    let session_id = run["session_id"].as_str().unwrap().to_string();
    let task_id = run["automation_task_id"].as_str().unwrap().to_string();
    assert_eq!(run["workflow_definition_id"], workflow_id);
    assert_eq!(run["workflow_version"], "v1");
    assert_eq!(run["state"], "pending");

    let get_run = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/workflow-runs/{run_id}"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_run.status(), StatusCode::OK);
    let run_body = response_json(get_run).await;
    assert_eq!(run_body["automation_task_id"], task_id);
    assert_eq!(run_body["session_id"], session_id);

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
    let events_body = response_json(run_events).await;
    assert!(events_body["events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|event| event["event_type"] == "workflow_run.created"));

    let run_logs = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/workflow-runs/{run_id}/logs"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(run_logs.status(), StatusCode::OK);
    let logs_body = response_json(run_logs).await;
    assert_eq!(logs_body["logs"].as_array().unwrap().len(), 0);

    let get_session = app
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
    assert_eq!(get_session.status(), StatusCode::OK);
    let session = response_json(get_session).await;
    assert_eq!(session["labels"]["origin"], "workflow-run");

    let get_task = app
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
}
