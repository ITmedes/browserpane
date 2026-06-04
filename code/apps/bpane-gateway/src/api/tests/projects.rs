use super::*;

#[tokio::test]
async fn manages_projects_and_reports_usage() {
    let (app, token) = test_router_with_docker_pool().await;

    let invalid = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "support",
                        "quotas": { "max_active_sessions": 0 }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);

    let created = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "support",
                        "description": "Support escalations",
                        "labels": { "team": "support" },
                        "quotas": {
                            "max_active_sessions": 1,
                            "max_active_workflow_runs": 2,
                            "max_retained_storage_bytes": 1048576
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let project = response_json(created).await;
    let project_id = project["id"].as_str().unwrap().to_string();
    assert_eq!(project["name"], "support");
    assert_eq!(project["state"], "active");
    assert_eq!(project["usage"]["active_sessions"], 0);
    assert_eq!(project["usage"]["queued_sessions"], 0);
    assert_eq!(project["usage"]["session_creations"], 0);
    assert_eq!(project["usage"]["max_active_sessions"], 1);
    assert_eq!(project["usage"]["runtime_usage_ms"], 0);
    assert_eq!(project["usage"]["egress_rx_bytes"], 0);
    assert_eq!(project["usage"]["egress_tx_bytes"], 0);
    assert_eq!(project["usage"]["egress_total_bytes"], 0);
    assert_eq!(project["usage"]["retained_storage_bytes"], 0);
    assert_eq!(project["usage"]["max_retained_storage_bytes"], 1048576);

    let list = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects")
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);
    let listed = response_json(list).await;
    assert_eq!(listed["projects"].as_array().unwrap().len(), 1);
    assert_eq!(listed["projects"][0]["id"], project_id);

    let updated = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/projects/{project_id}"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "support-escalations",
                        "description": "Support escalations",
                        "labels": { "team": "support", "priority": "high" },
                        "quotas": { "max_active_sessions": 1 },
                        "state": "active"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(updated.status(), StatusCode::OK);
    let updated = response_json(updated).await;
    assert_eq!(updated["name"], "support-escalations");
    assert_eq!(updated["labels"]["priority"], "high");

    let usage = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/projects/{project_id}/usage"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(usage.status(), StatusCode::OK);
    let usage = response_json(usage).await;
    assert_eq!(usage["project_id"], project_id);
    assert_eq!(usage["active_sessions"], 0);
    assert_eq!(usage["queued_sessions"], 0);
    assert_eq!(usage["session_creations"], 0);
    assert_eq!(usage["runtime_usage_ms"], 0);
    assert_eq!(usage["egress_total_bytes"], 0);
    assert_eq!(usage["retained_storage_bytes"], 0);
}

#[tokio::test]
async fn project_usage_counts_recording_bytes_and_rejects_over_quota_completion() {
    let (app, token) = test_router();

    let project = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/projects")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": "recording-storage",
                            "quotas": { "max_retained_storage_bytes": 10 }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let project_id = project["id"].as_str().unwrap().to_string();

    let session = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sessions")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "project_id": project_id,
                            "recording": { "mode": "manual", "format": "webm" }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let session_id = session["id"].as_str().unwrap().to_string();

    let first_recording = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/sessions/{session_id}/recordings"))
                    .header("authorization", bearer(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let first_recording_id = first_recording["id"].as_str().unwrap().to_string();

    let missing_bytes_dir = tempfile::tempdir().unwrap();
    let missing_bytes_path = missing_bytes_dir.path().join("missing-bytes.webm");
    fs::write(&missing_bytes_path, b"abc").unwrap();
    let missing_bytes = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/sessions/{session_id}/recordings/{first_recording_id}/complete"
                ))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "source_path": missing_bytes_path.to_string_lossy(),
                        "mime_type": "video/webm"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_bytes.status(), StatusCode::BAD_REQUEST);
    let missing_bytes = response_json(missing_bytes).await;
    assert!(missing_bytes["error"]
        .as_str()
        .unwrap()
        .contains("retained_storage_byte_count_required"));

    let first_dir = tempfile::tempdir().unwrap();
    let first_path = first_dir.path().join("first.webm");
    fs::write(&first_path, b"12345678").unwrap();
    let completed = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/sessions/{session_id}/recordings/{first_recording_id}/complete"
                ))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "source_path": first_path.to_string_lossy(),
                        "mime_type": "video/webm",
                        "bytes": 8,
                        "duration_ms": 1000
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(completed.status(), StatusCode::OK);

    let usage = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/projects/{project_id}/usage"))
                    .header("authorization", bearer(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(usage["retained_storage_bytes"], 8);
    assert_eq!(usage["max_retained_storage_bytes"], 10);

    let second_recording = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/sessions/{session_id}/recordings"))
                    .header("authorization", bearer(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let second_recording_id = second_recording["id"].as_str().unwrap().to_string();
    let second_dir = tempfile::tempdir().unwrap();
    let second_path = second_dir.path().join("second.webm");
    fs::write(&second_path, b"123").unwrap();
    let rejected = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/sessions/{session_id}/recordings/{second_recording_id}/complete"
                ))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "source_path": second_path.to_string_lossy(),
                        "mime_type": "video/webm",
                        "bytes": 3,
                        "duration_ms": 250
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rejected.status(), StatusCode::CONFLICT);
    let rejected = response_json(rejected).await;
    assert!(rejected["error"]
        .as_str()
        .unwrap()
        .contains("retained_storage_quota_exceeded"));
}

#[tokio::test]
async fn project_storage_quota_rejects_workflow_produced_file_uploads() {
    let (app, token) = test_router();

    let project = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/projects")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": "workflow-storage",
                            "quotas": { "max_retained_storage_bytes": 10 }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let project_id = project["id"].as_str().unwrap().to_string();

    let workspace = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/file-workspaces")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({ "name": "workflow-storage-output" }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let workspace_id = workspace["id"].as_str().unwrap().to_string();

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
                            "name": "workflow-storage"
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
                        "entrypoint": "workflows/storage.ts",
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
                        "project_id": project_id,
                        "session": {
                            "create_session": {
                                "project_id": project_id
                            }
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
    assert_eq!(run["project_id"], project_id);

    let first_upload = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/workflow-runs/{run_id}/produced-files"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .header("x-bpane-workflow-workspace-id", &workspace_id)
                .header("x-bpane-file-name", "result.json")
                .body(Body::from(b"12345678".to_vec()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first_upload.status(), StatusCode::CREATED);

    let usage = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/projects/{project_id}/usage"))
                    .header("authorization", bearer(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(usage["retained_storage_bytes"], 8);

    let rejected_upload = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/workflow-runs/{run_id}/produced-files"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .header("x-bpane-workflow-workspace-id", &workspace_id)
                .header("x-bpane-file-name", "too-large.json")
                .body(Body::from(b"123".to_vec()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rejected_upload.status(), StatusCode::CONFLICT);
    let rejected_upload = response_json(rejected_upload).await;
    assert!(rejected_upload["error"]
        .as_str()
        .unwrap()
        .contains("retained_storage_quota_exceeded"));

    let produced_files = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/workflow-runs/{run_id}/produced-files"))
                    .header("authorization", bearer(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(produced_files["files"].as_array().unwrap().len(), 1);

    let workspace_files = response_json(
        app.oneshot(
            Request::builder()
                .uri(format!("/api/v1/file-workspaces/{workspace_id}/files"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap(),
    )
    .await;
    assert_eq!(workspace_files["files"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn workflow_run_rejects_workspace_inputs_from_other_projects() {
    let (app, token) = test_router();

    let project_a = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/projects")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({ "name": "project-a" }).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let project_a_id = project_a["id"].as_str().unwrap().to_string();
    let project_b = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/projects")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({ "name": "project-b" }).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let project_b_id = project_b["id"].as_str().unwrap().to_string();

    let session = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sessions")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({ "project_id": project_a_id }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let session_id = session["id"].as_str().unwrap().to_string();

    let workspace = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/file-workspaces")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "project_id": project_b_id,
                            "name": "project-b-inputs"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let workspace_id = workspace["id"].as_str().unwrap().to_string();
    let input_file = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/file-workspaces/{workspace_id}/files"))
                    .header("authorization", bearer(&token))
                    .header("content-type", "text/plain")
                    .header("x-bpane-file-name", "input.txt")
                    .body(Body::from(b"input".to_vec()))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let file_id = input_file["id"].as_str().unwrap().to_string();

    let workflow = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/workflows")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({ "name": "projected-workflow" }).to_string(),
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
                        "entrypoint": "workflows/projected.ts",
                        "allowed_file_workspace_ids": [workspace_id]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_version.status(), StatusCode::CREATED);

    let rejected_run = app
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
                        "project_id": project_a_id,
                        "session": { "existing_session_id": session_id },
                        "workspace_inputs": [{
                            "workspace_id": workspace_id,
                            "file_id": file_id
                        }]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rejected_run.status(), StatusCode::BAD_REQUEST);
    let rejected_run = response_json(rejected_run).await;
    assert!(rejected_run["error"]
        .as_str()
        .unwrap()
        .contains("without a matching workflow run project_id"));
}

#[tokio::test]
async fn applies_project_admission_to_sessions_and_template_defaults() {
    let (app, token) = test_router_with_docker_pool().await;

    let project = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/projects")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": "tenant-alpha",
                            "quotas": { "max_active_sessions": 1 }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let project_id = project["id"].as_str().unwrap().to_string();

    let template = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/session-templates")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": "tenant-alpha-debug",
                            "defaults": { "project_id": project_id }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let template_id = template["id"].as_str().unwrap().to_string();

    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "template_id": template_id }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::CREATED);
    let first = response_json(first).await;
    let first_session_id = first["id"].as_str().unwrap().to_string();
    assert_eq!(first["project_id"], project_id);
    assert_eq!(first["project"]["id"], project_id);
    assert_eq!(first["admission"]["state"], "allowed");
    assert_eq!(first["admission"]["reason_code"], "project_quota_available");
    assert_eq!(first["admission"]["active_sessions"], 1);

    let status = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/sessions/{first_session_id}/status"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(status.status(), StatusCode::OK);
    let status = response_json(status).await;
    assert_eq!(status["project_id"], project_id);
    assert_eq!(status["project"]["id"], project_id);
    assert_eq!(status["admission"]["state"], "allowed");

    let queued = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "project_id": project_id }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(queued.status(), StatusCode::CREATED);
    let queued = response_json(queued).await;
    let queued_session_id = queued["id"].as_str().unwrap().to_string();
    assert_eq!(queued["state"], "queued");
    assert_eq!(queued["admission"]["state"], "queued");
    assert_eq!(
        queued["admission"]["reason_code"],
        "active_session_quota_exceeded"
    );
    assert_eq!(queued["queue"]["position"], 1);
    assert_eq!(queued["queue"]["active_sessions"], 1);
    assert_eq!(queued["queue"]["queued_sessions"], 1);
    assert_eq!(queued["queue"]["max_active_sessions"], 1);
    assert_eq!(
        queued["queue"]["dispatch_blocker"],
        "project_active_session_quota"
    );
    assert_eq!(queued["queue"]["cancellable"], true);
    assert!(queued["queued_at"].as_str().is_some());
    assert_eq!(queued["queued_at"], queued["queue"]["queued_at"]);

    let queued_token = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/sessions/{queued_session_id}/access-tokens"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(queued_token.status(), StatusCode::CONFLICT);
    let queued_token = response_json(queued_token).await;
    assert!(queued_token["error"]
        .as_str()
        .unwrap()
        .contains("not connectable in state queued"));

    let usage = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/projects/{project_id}/usage"))
                    .header("authorization", bearer(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(usage["active_sessions"], 1);
    assert_eq!(usage["queued_sessions"], 1);
    assert_eq!(usage["session_creations"], 2);
    assert!(usage["runtime_usage_ms"].as_u64().is_some());
    assert_eq!(usage["egress_rx_bytes"], 0);
    assert_eq!(usage["egress_tx_bytes"], 0);
    assert_eq!(usage["egress_total_bytes"], 0);

    let stopped = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/sessions/{first_session_id}"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stopped.status(), StatusCode::OK);

    let promoted = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/sessions/{queued_session_id}"))
                    .header("authorization", bearer(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(promoted["state"], "ready");
    assert_eq!(promoted["admission"]["state"], "allowed");
    assert_eq!(promoted["admission"]["active_sessions"], 1);
    assert!(promoted["queue"].is_null());
    assert!(promoted["queued_at"].is_null());

    let queued_again = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "project_id": project_id }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(queued_again.status(), StatusCode::CREATED);
    let queued_again = response_json(queued_again).await;
    let queued_again_session_id = queued_again["id"].as_str().unwrap().to_string();
    assert_eq!(queued_again["state"], "queued");
    assert_eq!(queued_again["queue"]["position"], 1);

    let queued_tail = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "project_id": project_id }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(queued_tail.status(), StatusCode::CREATED);
    let queued_tail = response_json(queued_tail).await;
    let queued_tail_session_id = queued_tail["id"].as_str().unwrap().to_string();
    assert_eq!(queued_tail["state"], "queued");
    assert_eq!(queued_tail["queue"]["position"], 2);
    assert_eq!(
        queued_tail["queue"]["dispatch_blocker"],
        "earlier_queued_session"
    );

    let cancelled = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/sessions/{queued_again_session_id}/cancel"))
                    .header("authorization", bearer(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(cancelled["state"], "stopped");
    assert!(cancelled["queue"].is_null());
    assert!(cancelled["queued_at"].is_null());

    let still_queued = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/sessions/{queued_tail_session_id}"))
                    .header("authorization", bearer(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(still_queued["state"], "queued");
    assert_eq!(still_queued["queue"]["position"], 1);
    assert_eq!(
        still_queued["queue"]["dispatch_blocker"],
        "project_active_session_quota"
    );

    let cancel_ready = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{queued_session_id}/cancel"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cancel_ready.status(), StatusCode::CONFLICT);

    let archived = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/projects/{project_id}"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "tenant-alpha",
                        "quotas": { "max_active_sessions": 2 },
                        "state": "archived"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(archived.status(), StatusCode::OK);

    let archived_rejected = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "project_id": project_id }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(archived_rejected.status(), StatusCode::CONFLICT);
    let archived_rejected = response_json(archived_rejected).await;
    assert!(archived_rejected["error"]
        .as_str()
        .unwrap()
        .contains("project_archived"));
}

#[tokio::test]
async fn enforces_project_template_and_egress_policy_for_sessions() {
    let (app, token) = test_router_with_docker_pool().await;

    let allowed_template = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/session-templates")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({ "name": "tenant-debug-template" }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let allowed_template_id = allowed_template["id"].as_str().unwrap().to_string();

    let disallowed_template = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/session-templates")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({ "name": "generic-debug-template" }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let disallowed_template_id = disallowed_template["id"].as_str().unwrap().to_string();

    let allowed_profile = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/egress-profiles")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": "approved-egress",
                            "proxy": { "url": "https://proxy.example:8443" }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let allowed_profile_id = allowed_profile["id"].as_str().unwrap().to_string();

    let disallowed_profile = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/egress-profiles")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": "generic-egress",
                            "proxy": { "url": "https://other-proxy.example:8443" }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let disallowed_profile_id = disallowed_profile["id"].as_str().unwrap().to_string();

    let project = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/projects")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": "tenant-policy",
                            "policy": {
                                "allowed_session_template_ids": [allowed_template_id],
                                "allowed_egress_profile_ids": [allowed_profile_id]
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
    let project_id = project["id"].as_str().unwrap().to_string();
    assert_eq!(
        project["policy"]["allowed_session_template_ids"][0],
        allowed_template_id
    );
    assert_eq!(
        project["policy"]["allowed_egress_profile_ids"][0],
        allowed_profile_id
    );

    let allowed = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "project_id": project_id,
                        "template_id": allowed_template_id,
                        "network_identity": { "egress_profile_id": allowed_profile_id }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(allowed.status(), StatusCode::CREATED);

    let rejected_template = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "project_id": project_id,
                        "template_id": disallowed_template_id,
                        "network_identity": { "egress_profile_id": allowed_profile_id }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rejected_template.status(), StatusCode::CONFLICT);
    let rejected_template = response_json(rejected_template).await;
    assert!(rejected_template["error"]
        .as_str()
        .unwrap()
        .contains("session_template_not_allowed"));

    let rejected_egress = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "project_id": project_id,
                        "template_id": allowed_template_id,
                        "network_identity": { "egress_profile_id": disallowed_profile_id }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rejected_egress.status(), StatusCode::CONFLICT);
    let rejected_egress = response_json(rejected_egress).await;
    assert!(rejected_egress["error"]
        .as_str()
        .unwrap()
        .contains("egress_profile_not_allowed"));
}
