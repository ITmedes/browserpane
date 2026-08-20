use super::*;

#[tokio::test]
async fn creates_workflow_definitions_versions_and_workflow_runs_with_default_sessions() {
    let (app, token, state) = test_router_with_state();
    sleep(Duration::from_secs(1)).await;
    let foreign_token = state
        .auth_validator
        .generate_token()
        .expect("hmac auth validator should generate a second dev token");

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
    assert_eq!(version["compatibility"]["state"], "legacy");

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

    let list_versions = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/workflows/{workflow_id}/versions"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_versions.status(), StatusCode::OK);
    let versions = response_json(list_versions).await;
    assert_eq!(versions["versions"].as_array().unwrap().len(), 1);
    assert_eq!(versions["versions"][0]["version"], "v1");

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

    let list_runs = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/workflow-runs")
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_runs.status(), StatusCode::OK);
    let runs_body = response_json(list_runs).await;
    let runs = runs_body["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0]["id"], run_id);
    assert_eq!(runs[0]["session_id"], session_id);
    assert_eq!(
        runs[0]["events_path"],
        format!("/api/v1/workflow-runs/{run_id}/events")
    );
    assert_eq!(
        runs[0]["logs_path"],
        format!("/api/v1/workflow-runs/{run_id}/logs")
    );

    let foreign_list_runs = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/workflow-runs")
                .header("authorization", bearer(&foreign_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(foreign_list_runs.status(), StatusCode::OK);
    let foreign_runs_body = response_json(foreign_list_runs).await;
    assert_eq!(foreign_runs_body["runs"].as_array().unwrap().len(), 0);

    let create_second_run = app
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
                        "session": {
                            "existing_session_id": session_id
                        },
                        "input": {
                            "month": "2026-04",
                            "country_code": "DE"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_second_run.status(), StatusCode::CREATED);
    let second_run = response_json(create_second_run).await;
    let second_run_id = second_run["id"].as_str().unwrap().to_string();

    let ordered_list_runs = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/workflow-runs")
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ordered_list_runs.status(), StatusCode::OK);
    let ordered_runs_body = response_json(ordered_list_runs).await;
    let ordered_runs = ordered_runs_body["runs"].as_array().unwrap();
    assert_eq!(ordered_runs.len(), 2);
    assert_eq!(ordered_runs[0]["id"], second_run_id);
    assert_eq!(ordered_runs[1]["id"], run_id);

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
async fn publishes_supported_package_and_preserves_immutable_git_evidence() {
    let repository = tempdir().unwrap();
    fs::create_dir_all(repository.path().join("workflow/lib")).unwrap();
    fs::write(
        repository.path().join("workflow/run.ts"),
        "import { outcome } from './lib/outcome.ts';\nexport default async function run() { return outcome; }\n",
    )
    .unwrap();
    fs::write(
        repository.path().join("workflow/lib/outcome.ts"),
        "export const outcome = { ok: true };\n",
    )
    .unwrap();
    git(&["init"], repository.path());
    git(
        &["config", "user.email", "test@browserpane.local"],
        repository.path(),
    );
    git(
        &["config", "user.name", "BrowserPane Test"],
        repository.path(),
    );
    git(&["add", "."], repository.path());
    git(&["commit", "-m", "initial package"], repository.path());
    let first_commit = git_head(repository.path());

    let (app, token) = test_router();
    sleep(Duration::from_secs(1)).await;
    let workflow = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/workflows")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({ "name": "supported-package" }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let workflow_id = workflow["id"].as_str().unwrap();
    let publish = |version: &str| {
        json!({
            "version": version,
            "executor": "playwright",
            "entrypoint": "workflow/run.ts",
            "source": {
                "kind": "git",
                "repository_url": repository.path(),
                "ref": "HEAD",
                "root_path": "workflow"
            },
            "input_schema": declared_object_schema(),
            "output_schema": declared_object_schema(),
            "package": supported_package_manifest()
        })
    };

    let first_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/workflows/{workflow_id}/versions"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(publish("v1").to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first_response.status(), StatusCode::CREATED);
    let first = response_json(first_response).await;
    assert_eq!(first["source"]["resolved_commit"], first_commit);
    assert_eq!(first["compatibility"]["state"], "supported");
    assert_eq!(first["package"]["runtime"]["language"], "typescript");
    assert_eq!(
        first["allowed_file_workspace_ids"],
        supported_package_manifest()["requirements"]["allowed_file_workspace_ids"]
    );

    fs::write(
        repository.path().join("workflow/lib/outcome.ts"),
        "export const outcome = { ok: true, revision: 2 };\n",
    )
    .unwrap();
    git(&["add", "."], repository.path());
    git(&["commit", "-m", "second package"], repository.path());
    let second_commit = git_head(repository.path());
    assert_ne!(first_commit, second_commit);

    let second_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/workflows/{workflow_id}/versions"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(publish("v2").to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second_response.status(), StatusCode::CREATED);
    let second = response_json(second_response).await;
    assert_eq!(second["source"]["resolved_commit"], second_commit);

    let original = response_json(
        app.oneshot(
            Request::builder()
                .uri(format!("/api/v1/workflows/{workflow_id}/versions/v1"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap(),
    )
    .await;
    assert_eq!(original["source"]["resolved_commit"], first_commit);
    assert_eq!(original["package"], first["package"]);
}

#[tokio::test]
async fn preserves_legacy_executor_compatibility_and_rejects_invalid_package_metadata() {
    let (app, token) = test_router();
    sleep(Duration::from_secs(1)).await;
    let workflow = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/workflows")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({ "name": "invalid-package" }).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let workflow_id = workflow["id"].as_str().unwrap();

    let unsupported = response_json(
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
                            "executor": "shell",
                            "entrypoint": "workflow/run.ts"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(unsupported["compatibility"]["state"], "unsupported");

    let invalid = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/workflows/{workflow_id}/versions"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "version": "v2",
                        "executor": "playwright",
                        "entrypoint": "workflow/run.mjs",
                        "input_schema": declared_object_schema(),
                        "output_schema": declared_object_schema(),
                        "package": supported_package_manifest()
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    assert!(response_json(invalid).await["error"]
        .as_str()
        .unwrap()
        .contains("TypeScript"));

    let packaged_unsupported = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/workflows/{workflow_id}/versions"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "version": "v3",
                        "executor": "shell",
                        "entrypoint": "workflow/run.ts",
                        "input_schema": declared_object_schema(),
                        "output_schema": declared_object_schema(),
                        "package": supported_package_manifest()
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(packaged_unsupported.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(packaged_unsupported).await["error"],
        "packaged workflow executor must be playwright"
    );
}

fn declared_object_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "additionalProperties": false
    })
}

fn supported_package_manifest() -> Value {
    json!({
        "package_id": "browserpane.test-package.v1",
        "format_version": "browserpane.workflow-package/v1",
        "runtime": {
            "language": "typescript",
            "browserpane_api_version": "v1",
            "node_major_version": 22,
            "playwright_major_version": 1,
            "playwright_minor_version": 59
        },
        "requirements": {
            "default_session": {
                "project_id": "019c888e-934b-7000-8000-000000000001",
                "browser_context": { "mode": "fresh", "context_id": null },
                "network_identity": { "egress_profile_id": null },
                "capabilities": {
                    "browser_input": true,
                    "clipboard": false,
                    "audio": false,
                    "microphone": false,
                    "camera": false,
                    "file_transfer": false,
                    "resize": true
                },
                "recording": {
                    "mode": "disabled",
                    "format": "webm",
                    "retention_sec": null
                },
                "extension_ids": []
            },
            "allowed_credential_binding_ids": [],
            "allowed_extension_ids": [],
            "allowed_file_workspace_ids": ["019c888e-934b-7000-8000-000000000002"]
        },
        "execution": {
            "timeout_ms": 60000,
            "assertions": ["schema-valid-output"],
            "safe_cancellation_points": ["before-submit"],
            "side_effect_checkpoints": ["after-submit"]
        },
        "publication": {
            "reviewer": "browserpane-test-reviewer",
            "reviewed_at": "2026-08-20T12:00:00Z",
            "decision": "approved",
            "fresh_context_replay": true,
            "scenarios": [
                { "kind": "happy_path", "result": "passed" },
                { "kind": "validation", "result": "passed" },
                { "kind": "missing_element", "result": "passed" },
                { "kind": "authentication_challenge", "result": "passed" },
                { "kind": "portal_failure", "result": "passed" },
                { "kind": "runtime_failure", "result": "passed" },
                { "kind": "cancellation", "result": "passed" },
                { "kind": "ambiguous_post_side_effect", "result": "passed" }
            ]
        }
    })
}
