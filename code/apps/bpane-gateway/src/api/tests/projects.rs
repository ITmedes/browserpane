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
                            "max_retained_storage_bytes": 1048576,
                            "max_session_creations": 1,
                            "max_session_creations_per_window": 2,
                            "session_creation_window_sec": 3600,
                            "max_runtime_usage_ms": 60000,
                            "max_egress_total_bytes": 10485760
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
    assert_eq!(project["usage"]["max_session_creations"], 1);
    assert_eq!(project["quotas"]["max_session_creations_per_window"], 2);
    assert_eq!(project["quotas"]["session_creation_window_sec"], 3600);
    assert_eq!(project["usage"]["runtime_usage_ms"], 0);
    assert_eq!(project["usage"]["max_runtime_usage_ms"], 60000);
    assert_eq!(project["usage"]["egress_rx_bytes"], 0);
    assert_eq!(project["usage"]["egress_tx_bytes"], 0);
    assert_eq!(project["usage"]["egress_total_bytes"], 0);
    assert_eq!(project["usage"]["max_egress_total_bytes"], 10485760);
    assert_eq!(project["usage"]["retained_storage_bytes"], 0);
    assert_eq!(project["usage"]["max_retained_storage_bytes"], 1048576);
    assert!(project["usage"]["alerts"].as_array().unwrap().is_empty());

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
async fn project_usage_reports_soft_budget_alerts_through_api() {
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
                            "name": "budget-alerts",
                            "quotas": {
                                "max_session_creations": 1,
                                "max_runtime_usage_ms": 60000,
                                "max_egress_total_bytes": 10485760
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
    assert!(project["usage"]["alerts"].as_array().unwrap().is_empty());

    let created_session = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "project_id": project_id
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(created_session.status(), StatusCode::CREATED);

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
    assert_eq!(usage["session_creations"], 1);
    assert_eq!(usage["max_session_creations"], 1);
    assert_eq!(usage["max_runtime_usage_ms"], 60000);
    assert_eq!(usage["max_egress_total_bytes"], 10485760);
    assert_eq!(usage["alerts"].as_array().unwrap().len(), 1);
    assert_eq!(usage["alerts"][0]["metric"], "session_creations");
    assert_eq!(usage["alerts"][0]["state"], "exceeded");
    assert_eq!(usage["alerts"][0]["current_value"], 1);
    assert_eq!(usage["alerts"][0]["limit_value"], 1);
    assert_eq!(usage["alerts"][0]["threshold_percent"], 100);
}

#[tokio::test]
async fn session_egress_usage_reports_roll_up_to_project_usage() {
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
                            "name": "egress-rollup",
                            "quotas": {
                                "max_egress_total_bytes": 100
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
    let session = response_json(
        app.clone()
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
            .unwrap(),
    )
    .await;
    let session_id = session["id"].as_str().unwrap().to_string();

    let no_op = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/egress-usage"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(json!({}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(no_op.status(), StatusCode::BAD_REQUEST);

    let invalid_observer = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/egress-usage"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "rx_bytes_delta": 1,
                        "observer_id": "https://proxy.example"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_observer.status(), StatusCode::BAD_REQUEST);

    let report = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/egress-usage"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "rx_bytes_delta": 40,
                        "tx_bytes_delta": 30,
                        "source_kind": "proxy",
                        "observer_id": "local-squid:3128",
                        "observed_at": "2026-06-05T10:00:00Z"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(report.status(), StatusCode::OK);
    let report = response_json(report).await;
    assert_eq!(report["session_id"], session_id);
    assert_eq!(report["project_id"], project_id);
    assert_eq!(report["egress_rx_bytes"], 40);
    assert_eq!(report["egress_tx_bytes"], 30);
    assert_eq!(report["egress_total_bytes"], 70);
    assert_eq!(report["rx_bytes_delta"], 40);
    assert_eq!(report["tx_bytes_delta"], 30);
    assert_eq!(report["source_kind"], "proxy");
    assert_eq!(report["observer_id"], "local-squid:3128");
    assert_eq!(report["observed_at"], "2026-06-05T10:00:00Z");
    assert!(report["recorded_at"].is_string());

    let second_report = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/egress-usage"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "rx_bytes_delta": 20,
                        "tx_bytes_delta": 20,
                        "source_kind": "tls_interceptor",
                        "observer_id": "mitmproxy"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second_report.status(), StatusCode::OK);
    let second_report = response_json(second_report).await;
    assert_eq!(second_report["egress_rx_bytes"], 60);
    assert_eq!(second_report["egress_tx_bytes"], 50);
    assert_eq!(second_report["source_kind"], "tls_interceptor");

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
    assert_eq!(usage["egress_rx_bytes"], 60);
    assert_eq!(usage["egress_tx_bytes"], 50);
    assert_eq!(usage["egress_total_bytes"], 110);
    assert_eq!(usage["alerts"].as_array().unwrap().len(), 1);
    assert_eq!(usage["alerts"][0]["metric"], "egress_total_bytes");
    assert_eq!(usage["alerts"][0]["state"], "exceeded");
    assert_eq!(usage["alerts"][0]["current_value"], 110);
    assert_eq!(usage["alerts"][0]["limit_value"], 100);
}

#[tokio::test]
async fn project_budget_enforcement_blocks_session_creation_when_enabled() {
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
                            "name": "blocking-budget",
                            "quotas": {
                                "max_session_creations": 1,
                                "max_runtime_usage_ms": 60000,
                                "max_egress_total_bytes": 10485760
                            },
                            "policy": {
                                "usage_budget_enforcement": "block_session_creation"
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
        project["policy"]["usage_budget_enforcement"],
        "block_session_creation"
    );

    let created_session = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "project_id": project_id
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(created_session.status(), StatusCode::CREATED);

    let rejected_session = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "project_id": project_id
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rejected_session.status(), StatusCode::CONFLICT);
    let rejection = String::from_utf8(
        axum::body::to_bytes(rejected_session.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(rejection.contains("session_creation_budget_exceeded"));
    assert!(rejection.contains("1/1"));

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
    assert_eq!(usage["session_creations"], 1);
    assert_eq!(usage["alerts"][0]["metric"], "session_creations");
    assert_eq!(usage["alerts"][0]["state"], "exceeded");
}

#[tokio::test]
async fn project_rate_limit_enforcement_blocks_session_creation_when_enabled() {
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
                            "name": "rate-limited-project",
                            "quotas": {
                                "max_session_creations_per_window": 1,
                                "session_creation_window_sec": 3600
                            },
                            "policy": {
                                "usage_budget_enforcement": "block_session_creation"
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

    let created_session = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "project_id": project_id
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(created_session.status(), StatusCode::CREATED);

    let rejected_session = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "project_id": project_id
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rejected_session.status(), StatusCode::CONFLICT);
    let rejection = String::from_utf8(
        axum::body::to_bytes(rejected_session.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(rejection.contains("session_creation_rate_exceeded"));
    assert!(rejection.contains("1/1"));
    assert!(rejection.contains("3600s"));
}

#[tokio::test]
async fn project_runtime_budget_enforcement_blocks_session_creation_when_enabled() {
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
                            "name": "runtime-budget-project",
                            "quotas": {
                                "max_runtime_usage_ms": 1
                            },
                            "policy": {
                                "usage_budget_enforcement": "block_session_creation"
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

    let created_session = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "project_id": project_id
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(created_session.status(), StatusCode::CREATED);
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;

    let rejected_session = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "project_id": project_id
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rejected_session.status(), StatusCode::CONFLICT);
    let rejection = String::from_utf8(
        axum::body::to_bytes(rejected_session.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(rejection.contains("runtime_usage_budget_exceeded"));
    assert!(rejection.contains("/1 ms"));
}

#[tokio::test]
async fn rejects_zero_project_usage_budget_quotas() {
    let (app, token) = test_router_with_docker_pool().await;

    for quotas in [
        json!({ "max_session_creations": 0 }),
        json!({ "max_session_creations_per_window": 0, "session_creation_window_sec": 3600 }),
        json!({ "max_session_creations_per_window": 1, "session_creation_window_sec": 0 }),
        json!({ "max_session_creations_per_window": 1 }),
        json!({ "session_creation_window_sec": 3600 }),
        json!({ "max_runtime_usage_ms": 0 }),
        json!({ "max_egress_total_bytes": 0 }),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/projects")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": "invalid-budget",
                            "quotas": quotas
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
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
async fn file_workspace_project_policy_allows_only_approved_workflow_inputs() {
    let (app, token) = test_router();

    let allowed_workspace = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/file-workspaces")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({ "name": "allowed-inputs" }).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let allowed_workspace_id = allowed_workspace["id"].as_str().unwrap().to_string();
    let allowed_file = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/api/v1/file-workspaces/{allowed_workspace_id}/files"
                    ))
                    .header("authorization", bearer(&token))
                    .header("content-type", "text/plain")
                    .header("x-bpane-file-name", "allowed.txt")
                    .body(Body::from(b"allowed".to_vec()))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let allowed_file_id = allowed_file["id"].as_str().unwrap().to_string();

    let disallowed_workspace = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/file-workspaces")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({ "name": "disallowed-inputs" }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let disallowed_workspace_id = disallowed_workspace["id"].as_str().unwrap().to_string();
    let disallowed_file = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/api/v1/file-workspaces/{disallowed_workspace_id}/files"
                    ))
                    .header("authorization", bearer(&token))
                    .header("content-type", "text/plain")
                    .header("x-bpane-file-name", "disallowed.txt")
                    .body(Body::from(b"disallowed".to_vec()))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let disallowed_file_id = disallowed_file["id"].as_str().unwrap().to_string();

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
                            "name": "workspace-policy",
                            "policy": {
                                "allowed_file_workspace_ids": [allowed_workspace_id]
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
        project["policy"]["allowed_file_workspace_ids"][0],
        allowed_workspace_id
    );

    let session = response_json(
        app.clone()
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
            .unwrap(),
    )
    .await;
    let session_id = session["id"].as_str().unwrap().to_string();

    let workflow = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/workflows")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({ "name": "workspace-policy" }).to_string(),
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
                        "entrypoint": "workflows/workspace-policy.ts",
                        "allowed_file_workspace_ids": [allowed_workspace_id, disallowed_workspace_id]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_version.status(), StatusCode::CREATED);

    let allowed_run = app
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
                        "session": { "existing_session_id": session_id },
                        "workspace_inputs": [{
                            "workspace_id": allowed_workspace_id,
                            "file_id": allowed_file_id
                        }]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(allowed_run.status(), StatusCode::CREATED);
    let allowed_run = response_json(allowed_run).await;
    assert_eq!(
        allowed_run["workspace_inputs"][0]["workspace_id"],
        allowed_workspace_id
    );

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
                        "project_id": project_id,
                        "session": { "existing_session_id": session_id },
                        "workspace_inputs": [{
                            "workspace_id": disallowed_workspace_id,
                            "file_id": disallowed_file_id
                        }]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rejected_run.status(), StatusCode::CONFLICT);
    let rejected_run = response_json(rejected_run).await;
    assert!(rejected_run["error"]
        .as_str()
        .unwrap()
        .contains("file_workspace_not_allowed"));
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
async fn enforces_project_resource_policy_for_sessions() {
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

    let allowed_extension = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/extensions")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": "approved-project-extension"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let allowed_extension_id = allowed_extension["id"].as_str().unwrap().to_string();
    let allowed_extension_version = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/extensions/{allowed_extension_id}/versions"
                ))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "version": "1.0.0",
                        "install_path": "/home/bpane/project-extension"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(allowed_extension_version.status(), StatusCode::CREATED);

    let disallowed_extension = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/extensions")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": "generic-project-extension"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let disallowed_extension_id = disallowed_extension["id"].as_str().unwrap().to_string();
    let disallowed_extension_version = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/extensions/{disallowed_extension_id}/versions"
                ))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "version": "1.0.0",
                        "install_path": "/home/bpane/generic-extension"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(disallowed_extension_version.status(), StatusCode::CREATED);

    let allowed_context = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/browser-contexts")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": "approved-project-context"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let allowed_context_id = allowed_context["id"].as_str().unwrap().to_string();

    let disallowed_context = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/browser-contexts")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": "generic-project-context"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let disallowed_context_id = disallowed_context["id"].as_str().unwrap().to_string();

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
                                "allowed_egress_profile_ids": [allowed_profile_id],
                                "allowed_extension_ids": [allowed_extension_id],
                                "allowed_browser_context_ids": [allowed_context_id]
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
    assert_eq!(
        project["policy"]["allowed_extension_ids"][0],
        allowed_extension_id
    );
    assert_eq!(
        project["policy"]["allowed_browser_context_ids"][0],
        allowed_context_id
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
                        "network_identity": { "egress_profile_id": allowed_profile_id },
                        "extension_ids": [allowed_extension_id],
                        "browser_context": {
                            "mode": "reusable",
                            "context_id": allowed_context_id
                        }
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

    let rejected_extension = app
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
                        "network_identity": { "egress_profile_id": allowed_profile_id },
                        "extension_ids": [disallowed_extension_id]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rejected_extension.status(), StatusCode::CONFLICT);
    let rejected_extension = response_json(rejected_extension).await;
    assert!(rejected_extension["error"]
        .as_str()
        .unwrap()
        .contains("extension_not_allowed"));

    let rejected_context = app
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
                        "network_identity": { "egress_profile_id": allowed_profile_id },
                        "browser_context": {
                            "mode": "reusable",
                            "context_id": disallowed_context_id
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rejected_context.status(), StatusCode::CONFLICT);
    let rejected_context = response_json(rejected_context).await;
    assert!(rejected_context["error"]
        .as_str()
        .unwrap()
        .contains("browser_context_not_allowed"));
}
