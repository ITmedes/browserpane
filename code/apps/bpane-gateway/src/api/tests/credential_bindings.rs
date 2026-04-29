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
