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

    let config_email = std::process::Command::new("git")
        .args(["config", "user.email", "workflow@test.local"])
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(
        config_email.status.success(),
        "{}",
        String::from_utf8_lossy(&config_email.stderr)
    );

    let config_name = std::process::Command::new("git")
        .args(["config", "user.name", "Workflow Test"])
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(
        config_name.status.success(),
        "{}",
        String::from_utf8_lossy(&config_name.stderr)
    );

    std::fs::create_dir_all(temp.path().join("workflows")).unwrap();
    std::fs::write(
        temp.path().join("workflows/run.ts"),
        "export default async function run() {}\n",
    )
    .unwrap();
    let add = std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(
        add.status.success(),
        "{}",
        String::from_utf8_lossy(&add.stderr)
    );
    let commit = std::process::Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(
        commit.status.success(),
        "{}",
        String::from_utf8_lossy(&commit.stderr)
    );
    let head = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(
        head.status.success(),
        "{}",
        String::from_utf8_lossy(&head.stderr)
    );
    let resolved_commit = String::from_utf8_lossy(&head.stdout).trim().to_string();

    let workflow = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/workflows")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({ "name": "git-backed" }).to_string()))
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
                        "entrypoint": "workflows/run.ts",
                        "source": {
                            "kind": "git",
                            "repository_url": temp.path().to_string_lossy(),
                            "ref": "main",
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
    assert_eq!(version["source"]["ref"], "main");
    assert_eq!(version["source"]["root_path"], "workflows");
    assert_eq!(version["source"]["resolved_commit"], resolved_commit);
}
