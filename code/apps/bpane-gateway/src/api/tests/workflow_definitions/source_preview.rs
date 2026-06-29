use super::*;

#[tokio::test]
async fn workflow_definition_versions_expose_entrypoint_source_preview_to_owner() {
    let (app, token, state) = test_router_with_state();
    sleep(Duration::from_secs(1)).await;
    let foreign_token = state
        .auth_validator
        .generate_token()
        .expect("hmac auth validator should generate a second dev token");
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
    fs::write(
        source_repo.path().join("workflows/run.ts"),
        "import { test } from '@playwright/test';\nexport async function run(): Promise<void> {\n  console.log('preview');\n}\n",
    )
    .unwrap();
    fs::write(
        source_repo.path().join("workflows/helper.ts"),
        "export const helperValue = 42;\n",
    )
    .unwrap();
    fs::write(source_repo.path().join("README.md"), "not in preview\n").unwrap();
    git(&["add", "."], source_repo.path());
    git(&["commit", "-m", "init"], source_repo.path());
    let resolved_commit = git_head(source_repo.path());

    let workflow = create_workflow(&app, &token, "preview-workflow").await;
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
                            "repository_url": source_repo.path().to_string_lossy(),
                            "resolved_commit": resolved_commit,
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

    let files_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/workflows/{workflow_id}/versions/v1/source-files"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(files_response.status(), StatusCode::OK);
    let files = response_json(files_response).await;
    let file_paths = files["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|file| file["path"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert_eq!(files["entrypoint"], "workflows/run.ts");
    assert!(file_paths.contains(&"workflows/run.ts".to_string()));
    assert!(file_paths.contains(&"workflows/helper.ts".to_string()));
    assert!(!file_paths.contains(&"README.md".to_string()));
    let entrypoint_file = files["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["path"] == "workflows/run.ts")
        .unwrap();
    assert_eq!(entrypoint_file["entrypoint"], true);

    let preview_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/workflows/{workflow_id}/versions/v1/source-preview"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(preview_response.status(), StatusCode::OK);
    let preview = response_json(preview_response).await;
    assert_eq!(preview["workflow_definition_id"], workflow_id);
    assert_eq!(preview["workflow_version"], "v1");
    assert_eq!(preview["entrypoint"], "workflows/run.ts");
    assert_eq!(preview["path"], "workflows/run.ts");
    assert_eq!(preview["language"], "typescript");
    assert_eq!(preview["media_type"], "text/typescript; charset=utf-8");
    assert_eq!(preview["truncated"], false);
    assert_eq!(preview["max_bytes"], 65536);
    assert!(preview["content"]
        .as_str()
        .unwrap()
        .contains("export async function run(): Promise<void>"));
    assert_eq!(preview["source"]["resolved_commit"], resolved_commit);

    let helper_preview_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/workflows/{workflow_id}/versions/v1/source-preview?path=workflows/helper.ts"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(helper_preview_response.status(), StatusCode::OK);
    let helper_preview = response_json(helper_preview_response).await;
    assert_eq!(helper_preview["entrypoint"], "workflows/run.ts");
    assert_eq!(helper_preview["path"], "workflows/helper.ts");
    assert!(helper_preview["content"]
        .as_str()
        .unwrap()
        .contains("helperValue"));

    let outside_root_preview = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/workflows/{workflow_id}/versions/v1/source-preview?path=README.md"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(outside_root_preview.status(), StatusCode::BAD_REQUEST);

    let foreign_preview = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/workflows/{workflow_id}/versions/v1/source-preview"
                ))
                .header("authorization", bearer(&foreign_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(foreign_preview.status(), StatusCode::NOT_FOUND);

    let foreign_files = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/workflows/{workflow_id}/versions/v1/source-files"
                ))
                .header("authorization", bearer(&foreign_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(foreign_files.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn workflow_definition_source_preview_requires_source_metadata() {
    let (app, token) = test_router();
    let workflow = create_workflow(&app, &token, "metadata-only-workflow").await;
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
                        "entrypoint": "workflows/run.ts"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_version.status(), StatusCode::CREATED);

    let preview_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/workflows/{workflow_id}/versions/v1/source-preview"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(preview_response.status(), StatusCode::NOT_FOUND);
    let error = response_json(preview_response).await;
    assert!(error["error"]
        .as_str()
        .unwrap()
        .contains("does not have source metadata"));

    let files_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/workflows/{workflow_id}/versions/v1/source-files"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(files_response.status(), StatusCode::NOT_FOUND);
}

async fn create_workflow(app: &Router, token: &str, name: &str) -> Value {
    response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/workflows")
                    .header("authorization", bearer(token))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({ "name": name }).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await
}
