use super::*;

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
