use super::*;

#[tokio::test]
async fn reports_current_identity_for_authenticated_principal() {
    let (app, token) = test_router();

    let unauthorized = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/identity/me")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/identity/me")
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["issuer"], "bpane-gateway");
    assert_eq!(body["principal_type"], "legacy_dev_token");
    assert!(body["subject"]
        .as_str()
        .unwrap()
        .starts_with("legacy-dev-token:"));
}

#[tokio::test]
async fn reports_access_review_with_project_usage_and_delegations() {
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
                            "name": "support",
                            "description": "Support escalations",
                            "labels": { "team": "support" },
                            "quotas": { "max_active_sessions": 3 }
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
                            "labels": { "purpose": "review-smoke" }
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
                        "issuer": "http://localhost:8091/realms/browserpane",
                        "display_name": "BrowserPane MCP bridge"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delegated.status(), StatusCode::OK);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/identity/access-review")
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;

    assert_eq!(body["principal"]["principal_type"], "legacy_dev_token");
    assert_eq!(body["resource_counts"]["projects"], 1);
    assert_eq!(body["resource_counts"]["sessions"], 1);
    assert_eq!(body["resource_counts"]["active_sessions"], 1);
    assert_eq!(body["resource_counts"]["delegated_principals"], 1);
    assert_eq!(body["projects"].as_array().unwrap().len(), 1);
    assert_eq!(body["projects"][0]["id"], project_id);
    assert_eq!(body["projects"][0]["usage"]["active_sessions"], 1);
    assert_eq!(body["delegated_principals"].as_array().unwrap().len(), 1);
    assert_eq!(
        body["delegated_principals"][0]["client_id"],
        "bpane-mcp-bridge"
    );
    assert_eq!(body["delegated_principals"][0]["session_count"], 1);
    assert_eq!(body["delegated_principals"][0]["active_session_count"], 1);
    assert_eq!(
        body["delegated_principals"][0]["session_ids"][0],
        session_id
    );
}
