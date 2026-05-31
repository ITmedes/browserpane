use super::*;

async fn create_project(app: Router, token: &str, name: &str, state: &str) -> Value {
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("authorization", bearer(token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": name,
                        "state": state
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    response_json(response).await
}

async fn create_service_principal(
    app: Router,
    token: &str,
    client_id: &str,
    issuer: &str,
) -> Value {
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/service-principals")
                .header("authorization", bearer(token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "MCP bridge",
                        "client_id": client_id,
                        "issuer": issuer,
                        "state": "active"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    response_json(response).await
}

async fn current_subject(app: Router, token: &str) -> String {
    let current_identity = response_json(
        app.oneshot(
            Request::builder()
                .uri("/api/v1/identity/me")
                .header("authorization", bearer(token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap(),
    )
    .await;
    current_identity["subject"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn owner_can_manage_identity_mappings() {
    let (app, token) = test_router();
    let issuer = "https://issuer.example";
    let project = create_project(app.clone(), &token, "acme-prod-support", "active").await;
    let project_id = project["id"].as_str().unwrap();
    let service_principal =
        create_service_principal(app.clone(), &token, "bpane-mcp-bridge", issuer).await;
    let service_principal_id = service_principal["id"].as_str().unwrap();

    let invalid = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/identity-mappings")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": " ",
                        "kind": "service_principal",
                        "issuer": issuer,
                        "external_id": "bpane-mcp-bridge",
                        "service_principal_id": service_principal_id,
                        "project_id": project_id
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
                .uri("/api/v1/identity-mappings")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "MCP bridge project access",
                        "description": "Allow MCP bridge for Acme support",
                        "kind": "service_principal",
                        "issuer": issuer,
                        "external_id": "bpane-mcp-bridge",
                        "service_principal_id": service_principal_id,
                        "project_id": project_id,
                        "labels": { "customer": "acme" },
                        "scopes": ["session:create", "session:delegate"],
                        "state": "active"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let created_body = response_json(created).await;
    let mapping_id = created_body["id"].as_str().unwrap().to_string();
    assert_eq!(created_body["kind"], "service_principal");
    assert_eq!(created_body["issuer"], issuer);
    assert_eq!(created_body["external_id"], "bpane-mcp-bridge");
    assert_eq!(created_body["service_principal_id"], service_principal_id);
    assert_eq!(created_body["project_id"], project_id);
    assert_eq!(created_body["labels"]["customer"], "acme");
    assert_eq!(created_body["scopes"][0], "session:create");
    assert_eq!(created_body["state"], "active");

    let duplicate = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/identity-mappings")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "Duplicate",
                        "kind": "service_principal",
                        "issuer": issuer,
                        "external_id": "bpane-mcp-bridge",
                        "service_principal_id": service_principal_id,
                        "project_id": project_id
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(duplicate.status(), StatusCode::CONFLICT);

    let listed = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/identity-mappings")
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(listed.status(), StatusCode::OK);
    let listed_body = response_json(listed).await;
    assert_eq!(listed_body["identity_mappings"][0]["id"], mapping_id);

    let fetched = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/identity-mappings/{mapping_id}"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(fetched.status(), StatusCode::OK);
    let fetched_body = response_json(fetched).await;
    assert_eq!(fetched_body["id"], mapping_id);

    let updated = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/identity-mappings/{mapping_id}"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "Disabled MCP bridge project access",
                        "kind": "service_principal",
                        "issuer": issuer,
                        "external_id": "bpane-mcp-bridge",
                        "service_principal_id": service_principal_id,
                        "project_id": project_id,
                        "labels": { "customer": "acme", "review": "pending" },
                        "scopes": ["session:create"],
                        "state": "disabled"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(updated.status(), StatusCode::OK);
    let updated_body = response_json(updated).await;
    assert_eq!(updated_body["id"], mapping_id);
    assert_eq!(updated_body["state"], "disabled");
    assert_eq!(updated_body["labels"]["review"], "pending");
}

#[tokio::test]
async fn identity_mapping_validation_rejects_bad_references_and_access_review_reports_mapping_state(
) {
    let (app, token) = test_router();
    let issuer = "https://issuer.example";
    let project = create_project(app.clone(), &token, "acme-prod-support", "active").await;
    let project_id = project["id"].as_str().unwrap();
    let archived_project =
        create_project(app.clone(), &token, "old-acme-support", "archived").await;
    let archived_project_id = archived_project["id"].as_str().unwrap();
    let service_principal =
        create_service_principal(app.clone(), &token, "bpane-mcp-bridge", issuer).await;
    let service_principal_id = service_principal["id"].as_str().unwrap();
    let current_subject = current_subject(app.clone(), &token).await;

    let missing_project = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/identity-mappings")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "Missing project",
                        "kind": "user",
                        "issuer": "bpane-gateway",
                        "external_id": "demo",
                        "project_id": "018f0000-0000-7000-8000-000000000999"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_project.status(), StatusCode::NOT_FOUND);

    let archived = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/identity-mappings")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "Archived project",
                        "kind": "user",
                        "issuer": "bpane-gateway",
                        "external_id": "demo",
                        "project_id": archived_project_id
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(archived.status(), StatusCode::BAD_REQUEST);

    let mismatch = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/identity-mappings")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "Mismatched service principal",
                        "kind": "service_principal",
                        "issuer": issuer,
                        "external_id": "wrong-client",
                        "service_principal_id": service_principal_id,
                        "project_id": project_id
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(mismatch.status(), StatusCode::BAD_REQUEST);

    let user_mapping = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/identity-mappings")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": "Demo user project access",
                            "kind": "user",
                            "issuer": "bpane-gateway",
                            "external_id": current_subject,
                            "project_id": project_id,
                            "state": "active"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;

    let review = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/identity/access-review")
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(review.status(), StatusCode::OK);
    let review_body = response_json(review).await;
    assert_eq!(review_body["resource_counts"]["identity_mappings"], 1);
    assert_eq!(
        review_body["identity_mappings"][0]["id"],
        user_mapping["id"].as_str().unwrap()
    );
    assert_eq!(
        review_body["identity_mappings"][0]["effective_for_principal"],
        true
    );
    assert!(review_body["unmapped_principal_signals"]
        .as_array()
        .expect("unmapped signals should be an array")
        .is_empty());
}
