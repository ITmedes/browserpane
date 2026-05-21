use super::*;

#[tokio::test]
async fn creates_session_from_template_and_filters_catalog() {
    let (app, token) = test_router();

    let create_template_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/session-templates")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "customer-debug-session",
                        "description": "Support debug template",
                        "labels": { "team": "support" },
                        "defaults": {
                            "owner_mode": "collaborative",
                            "idle_timeout_sec": 1800,
                            "labels": {
                                "team": "support",
                                "purpose": "debug"
                            },
                            "integration_context": {
                                "source": "template"
                            },
                            "recording": {
                                "mode": "manual",
                                "format": "webm",
                                "retention_sec": 86400
                            }
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_template_response.status(), StatusCode::CREATED);
    let template = response_json(create_template_response).await;
    let template_id = template["id"].as_str().unwrap().to_string();
    assert_eq!(template["name"], "customer-debug-session");
    assert_eq!(template["version"], 1);
    assert_eq!(template["defaults"]["labels"]["team"], "support");

    let create_session_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "template_id": template_id,
                        "labels": {
                            "case": "INC-1234",
                            "purpose": "case-specific"
                        },
                        "integration_context": {
                            "ticket": "INC-1234"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_session_response.status(), StatusCode::CREATED);
    let session = response_json(create_session_response).await;
    let session_id = session["id"].as_str().unwrap().to_string();
    assert_eq!(session["template_id"], template_id);
    assert_eq!(session["idle_timeout_sec"], 1800);
    assert_eq!(session["labels"]["team"], "support");
    assert_eq!(session["labels"]["purpose"], "case-specific");
    assert_eq!(session["labels"]["case"], "INC-1234");
    assert_eq!(session["integration_context"]["source"], "template");
    assert_eq!(session["integration_context"]["ticket"], "INC-1234");
    assert_eq!(session["recording"]["mode"], "manual");
    assert_eq!(session["recording"]["retention_sec"], 86400);

    let filtered_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/sessions?template_id={template_id}&label.team=support&integration.ticket=INC-1234"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(filtered_response.status(), StatusCode::OK);
    let filtered = response_json(filtered_response).await;
    assert_eq!(filtered["sessions"].as_array().unwrap().len(), 1);
    assert_eq!(filtered["sessions"][0]["id"], session_id);

    let mismatch_response = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/sessions?template_id={template_id}&label.team=sales"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(mismatch_response.status(), StatusCode::OK);
    let mismatch = response_json(mismatch_response).await;
    assert!(mismatch["sessions"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn rejects_invalid_session_template_requests_and_missing_templates() {
    let (app, token) = test_router();

    let unauthorized = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/session-templates")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

    for body in [
        json!({ "name": "" }),
        json!({ "name": "bad-description", "description": "" }),
        json!({ "name": "bad-idle", "defaults": { "idle_timeout_sec": 0 } }),
        json!({ "name": "bad-label", "defaults": { "labels": { "": "value" } } }),
        json!({ "name": "bad-integration", "defaults": { "integration_context": "not-an-object" } }),
        json!({
            "name": "bad-recording",
            "defaults": {
                "recording": {
                    "mode": "manual",
                    "format": "webm",
                    "retention_sec": 0
                }
            }
        }),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/session-templates")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    let missing_template_id = Uuid::now_v7();
    let missing_get = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/session-templates/{missing_template_id}"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_get.status(), StatusCode::NOT_FOUND);

    let missing_update = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/session-templates/{missing_template_id}"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "name": "missing" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_update.status(), StatusCode::NOT_FOUND);

    let create_from_missing_template = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "template_id": missing_template_id.to_string() }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_from_missing_template.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn validates_session_catalog_filters_and_paginates_results() {
    let (app, token) = test_router_with_docker_pool().await;

    for body in [
        json!({
            "labels": { "team": "support", "case": "INC-1" },
            "integration_context": { "ticket": "INC-1" }
        }),
        json!({
            "labels": { "team": "support", "case": "INC-2" },
            "integration_context": { "ticket": "INC-2" }
        }),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sessions")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    let filtered_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/sessions?label.team=support&runtime_state=not_started&state=ready&limit=1&offset=1")
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(filtered_response.status(), StatusCode::OK);
    let filtered = response_json(filtered_response).await;
    assert_eq!(filtered["sessions"].as_array().unwrap().len(), 1);
    assert_eq!(filtered["sessions"][0]["labels"]["team"], "support");

    let integration_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/sessions?integration.ticket=INC-2")
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(integration_response.status(), StatusCode::OK);
    let integration = response_json(integration_response).await;
    assert_eq!(integration["sessions"].as_array().unwrap().len(), 1);
    assert_eq!(integration["sessions"][0]["labels"]["case"], "INC-2");

    for uri in [
        "/api/v1/sessions?unknown=1",
        "/api/v1/sessions?state=bogus",
        "/api/v1/sessions?runtime_state=bogus",
        "/api/v1/sessions?limit=0",
        "/api/v1/sessions?limit=not-a-number",
        "/api/v1/sessions?offset=-1",
        "/api/v1/sessions?label.=support",
        "/api/v1/sessions?integration.=INC-1",
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .header("authorization", bearer(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{uri}");
    }
}

#[tokio::test]
async fn updates_session_template_with_incremented_version() {
    let (app, token) = test_router();

    let created = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/session-templates")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": "template",
                            "defaults": {
                                "labels": { "team": "support" }
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
    let template_id = created["id"].as_str().unwrap();

    let duplicate_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/session-templates")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "template",
                        "defaults": {}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(duplicate_response.status(), StatusCode::CONFLICT);

    let updated_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/session-templates/{template_id}"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "template",
                        "defaults": {
                            "idle_timeout_sec": 600,
                            "labels": { "team": "support", "tier": "gold" }
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(updated_response.status(), StatusCode::OK);
    let updated = response_json(updated_response).await;
    assert_eq!(updated["version"], 2);
    assert_eq!(updated["defaults"]["idle_timeout_sec"], 600);
    assert_eq!(updated["defaults"]["labels"]["tier"], "gold");

    let list_response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/session-templates")
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    let listed = response_json(list_response).await;
    assert_eq!(listed["templates"].as_array().unwrap().len(), 1);
    assert_eq!(listed["templates"][0]["version"], 2);
}
