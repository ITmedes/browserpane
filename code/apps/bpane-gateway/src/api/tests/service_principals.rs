use super::*;

#[tokio::test]
async fn owner_can_create_list_fetch_and_update_service_principals() {
    let (app, token) = test_router();

    let invalid = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/service-principals")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": " ",
                        "client_id": "bpane-mcp-bridge",
                        "issuer": "https://issuer.example"
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
                .uri("/api/v1/service-principals")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "MCP bridge",
                        "description": "Bridge automation identity",
                        "client_id": "bpane-mcp-bridge",
                        "issuer": "https://issuer.example",
                        "labels": { "system": "mcp" },
                        "scopes": ["session:delegate"],
                        "allowed_project_ids": [],
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
    let service_principal_id = created_body["id"].as_str().unwrap().to_string();
    assert_eq!(created_body["name"], "MCP bridge");
    assert_eq!(created_body["client_id"], "bpane-mcp-bridge");
    assert_eq!(created_body["issuer"], "https://issuer.example");
    assert_eq!(created_body["labels"]["system"], "mcp");
    assert_eq!(created_body["scopes"][0], "session:delegate");
    assert_eq!(created_body["state"], "active");
    assert!(created_body["last_seen_at"].is_null());
    assert!(created_body["last_delegated_at"].is_null());

    let duplicate = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/service-principals")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "Duplicate MCP bridge",
                        "client_id": "bpane-mcp-bridge",
                        "issuer": "https://issuer.example"
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
                .uri("/api/v1/service-principals")
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(listed.status(), StatusCode::OK);
    let listed_body = response_json(listed).await;
    assert_eq!(
        listed_body["service_principals"]
            .as_array()
            .expect("service principals should be an array")
            .len(),
        1
    );
    assert_eq!(
        listed_body["service_principals"][0]["id"],
        service_principal_id
    );

    let fetched = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/service-principals/{service_principal_id}"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(fetched.status(), StatusCode::OK);
    let fetched_body = response_json(fetched).await;
    assert_eq!(fetched_body["id"], service_principal_id);

    let updated = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/service-principals/{service_principal_id}"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "Disabled MCP bridge",
                        "client_id": "bpane-mcp-bridge",
                        "issuer": "https://issuer.example",
                        "labels": { "system": "mcp", "state": "review" },
                        "scopes": ["session:delegate", "workflow:run"],
                        "allowed_project_ids": [],
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
    assert_eq!(updated_body["id"], service_principal_id);
    assert_eq!(updated_body["name"], "Disabled MCP bridge");
    assert_eq!(updated_body["labels"]["state"], "review");
    assert_eq!(updated_body["scopes"][1], "workflow:run");
    assert_eq!(updated_body["state"], "disabled");
}

#[tokio::test]
async fn disabled_service_principals_block_delegation_and_access_review_correlates_registry() {
    let (app, token) = test_router();
    let issuer = "https://issuer.example";

    let service_principal = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/service-principals")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": "MCP bridge",
                            "client_id": "bpane-mcp-bridge",
                            "issuer": issuer,
                            "state": "disabled"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let service_principal_id = service_principal["id"].as_str().unwrap().to_string();

    let session = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sessions")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let session_id = session["id"].as_str().unwrap().to_string();

    let blocked = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/automation-owner"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "client_id": "bpane-mcp-bridge",
                        "issuer": issuer,
                        "display_name": "BrowserPane MCP bridge"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(blocked.status(), StatusCode::CONFLICT);
    let blocked_body = response_json(blocked).await;
    assert!(blocked_body["error"].as_str().unwrap().contains("disabled"));

    let enabled = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/service-principals/{service_principal_id}"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "MCP bridge",
                        "client_id": "bpane-mcp-bridge",
                        "issuer": issuer,
                        "state": "active"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(enabled.status(), StatusCode::OK);

    let delegated = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/automation-owner"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "client_id": "bpane-mcp-bridge",
                        "issuer": issuer,
                        "display_name": "BrowserPane MCP bridge"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delegated.status(), StatusCode::OK);

    let fetched = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/service-principals/{service_principal_id}"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(fetched.status(), StatusCode::OK);
    let fetched_body = response_json(fetched).await;
    assert!(fetched_body["last_delegated_at"].is_string());

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
    assert_eq!(review_body["resource_counts"]["service_principals"], 1);
    assert_eq!(
        review_body["service_principals"][0]["id"],
        service_principal_id
    );
    assert_eq!(
        review_body["service_principals"][0]["delegated_session_count"],
        1
    );
    assert_eq!(
        review_body["service_principals"][0]["active_delegated_session_count"],
        1
    );
    assert_eq!(
        review_body["service_principals"][0]["delegated_session_ids"][0],
        session_id
    );
    assert_eq!(review_body["delegated_principals"][0]["registered"], true);
    assert_eq!(
        review_body["delegated_principals"][0]["registered_service_principal_id"],
        service_principal_id
    );
    assert_eq!(review_body["delegated_principals"][0]["state"], "active");
}
