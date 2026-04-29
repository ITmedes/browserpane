use super::*;

#[tokio::test]
async fn workflow_runs_expose_source_snapshot_content_to_owner_and_automation_access() {
    let (app, token) = test_router();
    let source_repo = tempdir().unwrap();
    git(&["init", "--initial-branch=main"], source_repo.path());
    git(
        &["config", "user.email", "workflow@test.local"],
        source_repo.path(),
    );
    git(
        &["config", "user.name", "Workflow Test"],
        source_repo.path(),
    );
    fs::create_dir_all(source_repo.path().join("workflows")).unwrap();
    fs::write(source_repo.path().join("README.md"), "root\n").unwrap();
    fs::write(
        source_repo.path().join("workflows/demo.ts"),
        "export default async function demo() {}\n",
    )
    .unwrap();
    fs::write(source_repo.path().join("workflows/helper.txt"), "helper\n").unwrap();
    git(&["add", "."], source_repo.path());
    git(&["commit", "-m", "init"], source_repo.path());
    let resolved_commit = git_head(source_repo.path());

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
                            "name": "snapshot-workflow"
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
                        "entrypoint": "workflows/demo.ts",
                        "source": {
                            "kind": "git",
                            "repository_url": source_repo.path().to_string_lossy(),
                            "resolved_commit": resolved_commit.clone(),
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

    let create_run = response_json(
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
    let run_id = create_run["id"].as_str().unwrap().to_string();
    let session_id = create_run["session_id"].as_str().unwrap().to_string();
    let source_snapshot = create_run["source_snapshot"].clone();
    assert_eq!(source_snapshot["entrypoint"], "workflows/demo.ts");
    assert_eq!(
        source_snapshot["source"]["resolved_commit"],
        resolved_commit
    );
    let content_path = source_snapshot["content_path"]
        .as_str()
        .unwrap()
        .to_string();

    let owner_download = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&content_path)
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(owner_download.status(), StatusCode::OK);
    let owner_bytes = response_bytes(owner_download).await;
    let mut owner_zip = ZipArchive::new(Cursor::new(owner_bytes.clone())).unwrap();
    let owner_names = (0..owner_zip.len())
        .map(|index| owner_zip.by_index(index).unwrap().name().to_string())
        .collect::<Vec<_>>();
    assert!(owner_names.contains(&"workflows/demo.ts".to_string()));
    assert!(owner_names.contains(&"workflows/helper.txt".to_string()));
    assert!(!owner_names.contains(&"README.md".to_string()));

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
    let automation_download = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/workflow-runs/{run_id}/source-snapshot/content"
                ))
                .header("x-bpane-automation-access-token", &automation_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(automation_download.status(), StatusCode::OK);
    let automation_bytes = response_bytes(automation_download).await;
    assert_eq!(automation_bytes, owner_bytes);
}

#[tokio::test]
async fn workflow_runs_expose_workspace_input_content_to_owner_and_automation_access() {
    let (app, token) = test_router();

    let workspace = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/file-workspaces")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({ "name": "workflow-inputs" }).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let workspace_id = workspace["id"].as_str().unwrap().to_string();

    let file_bytes = b"month,total\n2026-03,42\n".to_vec();
    let upload_file = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/file-workspaces/{workspace_id}/files"))
                .header("authorization", bearer(&token))
                .header("content-type", "text/csv")
                .header("x-bpane-file-name", "monthly-report.csv")
                .body(Body::from(file_bytes.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(upload_file.status(), StatusCode::CREATED);
    let file = response_json(upload_file).await;
    let file_id = file["id"].as_str().unwrap().to_string();

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
                            "name": "workspace-input-workflow"
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
                        "entrypoint": "workflows/demo.ts",
                        "allowed_file_workspace_ids": [workspace_id]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_version.status(), StatusCode::CREATED);

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
                        "session": {
                            "create_session": {}
                        },
                        "workspace_inputs": [
                            {
                                "workspace_id": workspace_id,
                                "file_id": file_id,
                                "mount_path": "inputs/monthly-report.csv"
                            }
                        ]
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
    let workspace_inputs = run["workspace_inputs"].as_array().unwrap();
    assert_eq!(workspace_inputs.len(), 1);
    let workspace_input = &workspace_inputs[0];
    assert_eq!(workspace_input["workspace_id"], workspace_id);
    assert_eq!(workspace_input["file_id"], file_id);
    assert_eq!(workspace_input["mount_path"], "inputs/monthly-report.csv");

    let content_path = workspace_input["content_path"]
        .as_str()
        .unwrap()
        .to_string();
    let owner_download = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&content_path)
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(owner_download.status(), StatusCode::OK);
    let owner_bytes = response_bytes(owner_download).await;
    assert_eq!(owner_bytes, file_bytes);

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
    let automation_download = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/workflow-runs/{run_id}/workspace-inputs/{}/content",
                    workspace_input["id"].as_str().unwrap()
                ))
                .header("x-bpane-automation-access-token", &automation_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(automation_download.status(), StatusCode::OK);
    let automation_bytes = response_bytes(automation_download).await;
    assert_eq!(automation_bytes, owner_bytes);
}
