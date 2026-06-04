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
    assert_eq!(project["usage"]["max_active_sessions"], 1);

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

    let rejected = app
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
    assert_eq!(rejected.status(), StatusCode::CONFLICT);
    let rejected = response_json(rejected).await;
    assert!(rejected["error"]
        .as_str()
        .unwrap()
        .contains("active_session_quota_exceeded"));

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

    let second = app
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
    assert_eq!(second.status(), StatusCode::CREATED);

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
