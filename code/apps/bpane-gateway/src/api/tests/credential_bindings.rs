use super::*;

#[tokio::test]
async fn creates_lists_and_gets_credential_bindings() {
    let (app, token) = test_router();

    let created_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/credential-bindings")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "demo-login",
                        "provider": "vault_kv_v2",
                        "namespace": "smoke",
                        "allowed_origins": ["http://web:8080"],
                        "injection_mode": "form_fill",
                        "secret_payload": {
                            "username": "demo",
                            "password": "demo-demo"
                        },
                        "labels": {
                            "suite": "credential"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(created_response.status(), StatusCode::CREATED);
    let created = response_json(created_response).await;
    let binding_id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["name"], "demo-login");
    assert_eq!(created["provider"], "vault_kv_v2");
    assert_eq!(created["namespace"], "smoke");
    assert_eq!(created["allowed_origins"], json!(["http://web:8080"]));
    assert_eq!(created["injection_mode"], "form_fill");
    assert!(created["external_ref"]
        .as_str()
        .unwrap()
        .starts_with("test/"));
    assert!(created.get("secret_payload").is_none());

    let listed = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/credential-bindings")
                    .header("authorization", bearer(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let bindings = listed["credential_bindings"].as_array().unwrap();
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0]["id"], binding_id);

    let fetched = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/credential-bindings/{binding_id}"))
                    .header("authorization", bearer(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(fetched["id"], binding_id);
    assert!(fetched.get("secret_payload").is_none());
}

#[tokio::test]
async fn credential_binding_project_scope_shapes_resource_and_validates_project_reference() {
    let (app, token) = test_router();
    let project_id = create_credential_test_project(&app, &token, "credential-scope").await;

    let created =
        create_credential_test_binding(&app, &token, "scoped-login", Some(project_id.as_str()))
            .await;
    let binding_id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["project_id"], project_id);
    assert_eq!(created["project"]["id"], project_id);
    assert_eq!(created["project"]["name"], "credential-scope");

    let listed = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/credential-bindings")
                    .header("authorization", bearer(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(listed["credential_bindings"][0]["id"], binding_id);
    assert_eq!(listed["credential_bindings"][0]["project_id"], project_id);
    assert_eq!(
        listed["credential_bindings"][0]["project"]["id"],
        project_id
    );

    let nil_project_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/credential-bindings")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "project_id": Uuid::nil(),
                        "name": "nil-project-login",
                        "provider": "vault_kv_v2",
                        "injection_mode": "form_fill",
                        "secret_payload": { "username": "demo" }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(nil_project_response.status(), StatusCode::BAD_REQUEST);
    let nil_project_error = response_json(nil_project_response).await;
    assert!(nil_project_error["error"]
        .as_str()
        .unwrap()
        .contains("project_id must not be nil"));

    let missing_project_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/credential-bindings")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "project_id": Uuid::now_v7(),
                        "name": "missing-project-login",
                        "provider": "vault_kv_v2",
                        "injection_mode": "form_fill",
                        "secret_payload": { "username": "demo" }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_project_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn credential_binding_project_scope_enforces_workflow_runs_and_egress_sessions() {
    let (app, token) = test_router();
    let project_a_id = create_credential_test_project(&app, &token, "project-a").await;
    let project_b_id = create_credential_test_project(&app, &token, "project-b").await;
    let project_a_binding =
        create_credential_test_binding(&app, &token, "project-a-login", Some(&project_a_id)).await;
    let project_a_binding_id = project_a_binding["id"].as_str().unwrap().to_string();

    let workflow = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/workflows")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({ "name": "scoped-credential-workflow" }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let workflow_id = workflow["id"].as_str().unwrap().to_string();

    let version_response = app
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
                        "allowed_credential_binding_ids": [project_a_binding_id]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(version_response.status(), StatusCode::CREATED);

    let rejected_run_response = app
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
                        "project_id": project_b_id,
                        "session": { "create_session": {} },
                        "credential_binding_ids": [project_a_binding_id]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rejected_run_response.status(), StatusCode::CONFLICT);
    let rejected_run = response_json(rejected_run_response).await;
    assert!(rejected_run["error"]
        .as_str()
        .unwrap()
        .contains("credential_binding_project_scope_mismatch"));

    let egress_profile_id = create_credential_test_egress_profile(
        &app,
        &token,
        "project-a-auth-proxy",
        &project_a_binding_id,
    )
    .await;
    let accepted_session_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "project_id": project_a_id,
                        "network_identity": { "egress_profile_id": egress_profile_id }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(accepted_session_response.status(), StatusCode::CREATED);
    let accepted_session = response_json(accepted_session_response).await;

    let stop_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/sessions/{}/stop",
                    accepted_session["id"].as_str().unwrap()
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stop_response.status(), StatusCode::OK);

    let rejected_session_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "project_id": project_b_id,
                        "network_identity": { "egress_profile_id": egress_profile_id }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rejected_session_response.status(), StatusCode::CONFLICT);
    let rejected_session = response_json(rejected_session_response).await;
    assert!(rejected_session["error"]
        .as_str()
        .unwrap()
        .contains("credential_binding_project_scope_mismatch"));

    let accepted_run_response = app
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
                        "project_id": project_a_id,
                        "session": { "create_session": {} },
                        "credential_binding_ids": [project_a_binding_id]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(accepted_run_response.status(), StatusCode::CREATED);
    let accepted_run = response_json(accepted_run_response).await;
    assert_eq!(accepted_run["project_id"], project_a_id);
    assert_eq!(
        accepted_run["credential_bindings"][0]["project_id"],
        project_a_id
    );
}

#[tokio::test]
async fn workflow_runs_resolve_credential_bindings_via_automation_access() {
    let (app, token) = test_router();

    let created_binding = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/credential-bindings")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": "demo-login",
                            "provider": "vault_kv_v2",
                            "namespace": "smoke",
                            "allowed_origins": ["http://web:8080"],
                            "injection_mode": "form_fill",
                            "secret_payload": {
                                "username": "demo",
                                "password": "demo-demo"
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
    let binding_id = created_binding["id"].as_str().unwrap().to_string();

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
                            "name": "credential-workflow"
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
                        "allowed_credential_binding_ids": [binding_id]
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
                        "credential_binding_ids": [binding_id]
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
    let credential_bindings = run["credential_bindings"].as_array().unwrap();
    assert_eq!(credential_bindings.len(), 1);
    assert_eq!(credential_bindings[0]["id"], binding_id);

    let owner_resolve = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/workflow-runs/{run_id}/credential-bindings/{binding_id}/resolved"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(owner_resolve.status(), StatusCode::UNAUTHORIZED);

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

    let automation_resolve = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/api/v1/workflow-runs/{run_id}/credential-bindings/{binding_id}/resolved"
                    ))
                    .header("x-bpane-automation-access-token", &automation_token)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(automation_resolve["binding"]["id"], binding_id);
    assert_eq!(
        automation_resolve["payload"],
        json!({
            "username": "demo",
            "password": "demo-demo"
        })
    );

    let events = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/workflow-runs/{run_id}/events"))
                    .header("authorization", bearer(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert!(events["events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|event| event["event_type"] == "workflow_run.credential_binding_resolved"));
}

async fn create_credential_test_project(app: &Router, token: &str, name: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("authorization", bearer(token))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "name": name }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    response_json(response).await["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn create_credential_test_binding(
    app: &Router,
    token: &str,
    name: &str,
    project_id: Option<&str>,
) -> Value {
    let mut body = json!({
        "name": name,
        "provider": "vault_kv_v2",
        "namespace": "smoke",
        "allowed_origins": ["http://web:8080"],
        "injection_mode": "form_fill",
        "secret_payload": {
            "username": "demo",
            "password": "demo-demo"
        }
    });
    if let Some(project_id) = project_id {
        body["project_id"] = json!(project_id);
    }
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/credential-bindings")
                .header("authorization", bearer(token))
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    response_json(response).await
}

async fn create_credential_test_egress_profile(
    app: &Router,
    token: &str,
    name: &str,
    credential_binding_id: &str,
) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/egress-profiles")
                .header("authorization", bearer(token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": name,
                        "proxy": {
                            "url": "http://127.0.0.1:3128",
                            "credential_binding_id": credential_binding_id
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    response_json(response).await["id"]
        .as_str()
        .unwrap()
        .to_string()
}
