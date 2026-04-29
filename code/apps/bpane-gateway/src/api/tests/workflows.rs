use super::*;

#[tokio::test]
async fn workflow_runs_inherit_session_extensions() {
    let (app, token) = test_router_with_docker_pool().await;

    let create_extension_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/extensions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "workflow-extension",
                        "description": "Approved workflow extension",
                        "labels": { "suite": "contract" }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_extension_response.status(), StatusCode::CREATED);
    let extension = response_json(create_extension_response).await;
    let extension_id = extension["id"].as_str().unwrap().to_string();

    let create_version_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/extensions/{extension_id}/versions"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "version": "1.0.0",
                        "install_path": "/home/bpane/bpane-test-extension"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_version_response.status(), StatusCode::CREATED);

    let create_workflow_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workflows")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "extension-smoke",
                        "description": "Workflow extension test",
                        "labels": { "suite": "contract" }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_workflow_response.status(), StatusCode::CREATED);
    let workflow = response_json(create_workflow_response).await;
    let workflow_id = workflow["id"].as_str().unwrap().to_string();

    let create_workflow_version_response = app
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
                        "entrypoint": "workflows/extensions/run.mjs",
                        "default_session": {
                            "extension_ids": [extension_id]
                        },
                        "allowed_extension_ids": [extension_id]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        create_workflow_version_response.status(),
        StatusCode::CREATED
    );

    let create_run_response = app
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
                            "suite": "contract"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_run_response.status(), StatusCode::CREATED);
    let run = response_json(create_run_response).await;
    let session_id = run["session_id"].as_str().unwrap().to_string();
    assert_eq!(run["extensions"].as_array().unwrap().len(), 1);
    assert_eq!(run["extensions"][0]["extension_id"], extension_id);

    let session_response = app
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
    assert_eq!(session_response.status(), StatusCode::OK);
    let session = response_json(session_response).await;
    assert_eq!(session["extensions"].as_array().unwrap().len(), 1);
    assert_eq!(session["extensions"][0]["extension_id"], extension_id);
}

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
                            "month": "2026-03"
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
    assert_eq!(create_run.status(), StatusCode::CREATED);
    let run = response_json(create_run).await;
    let run_id = run["id"].as_str().unwrap().to_string();
    let task_id = run["automation_task_id"].as_str().unwrap().to_string();
    let session_id = run["session_id"].as_str().unwrap().to_string();
    assert_eq!(run["state"], "pending");
    assert_eq!(run["workflow_version"], "v1");
    assert_eq!(run["labels"]["suite"], "contract");
    assert_eq!(run["input"]["month"], "2026-03");

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
    assert_eq!(events_body["events"].as_array().unwrap().len(), 2);
    let event_types = events_body["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["event_type"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert!(event_types.contains(&"workflow_run.created".to_string()));
    assert!(event_types.contains(&"automation_task.created".to_string()));

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

#[tokio::test]
async fn workflow_definition_versions_can_pin_git_source_metadata() {
    let (app, token) = test_router();
    let temp = tempfile::tempdir().unwrap();

    let init = std::process::Command::new("git")
        .args(["init", "--initial-branch=main"])
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );
    for args in [
        vec!["config", "user.email", "workflow@test.local"],
        vec!["config", "user.name", "Workflow Test"],
    ] {
        let output = std::process::Command::new("git")
            .args(&args)
            .current_dir(temp.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    std::fs::create_dir_all(temp.path().join("workflows")).unwrap();
    std::fs::write(
        temp.path().join("workflows").join("report.ts"),
        "export default async function run() {}\n",
    )
    .unwrap();
    for args in [vec!["add", "."], vec!["commit", "-m", "init"]] {
        let output = std::process::Command::new("git")
            .args(&args)
            .current_dir(temp.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let head = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(head.status.success());
    let expected_commit = String::from_utf8_lossy(&head.stdout)
        .trim()
        .to_ascii_lowercase();

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
                        "name": "git-backed-workflow"
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
                        "entrypoint": "workflows/report.ts",
                        "source": {
                            "kind": "git",
                            "repository_url": temp.path().to_string_lossy(),
                            "ref": "HEAD",
                            "root_path": "workflows"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_version.status(), StatusCode::CREATED);
    let version = response_json(create_version).await;
    assert_eq!(version["source"]["kind"], "git");
    assert_eq!(
        version["source"]["repository_url"],
        temp.path().to_string_lossy().to_string()
    );
    assert_eq!(version["source"]["ref"], "HEAD");
    assert_eq!(version["source"]["resolved_commit"], expected_commit);
    assert_eq!(version["source"]["root_path"], "workflows");
}
