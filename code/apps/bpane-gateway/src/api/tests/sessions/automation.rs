use super::*;

#[tokio::test]
async fn issues_session_automation_access_descriptor() {
    let (app, token) = test_router();

    let created = response_json(
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
    let session_id = created["id"].as_str().unwrap().to_string();

    let issue_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/automation-access"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(issue_response.status(), StatusCode::OK);
    let issued = response_json(issue_response).await;
    assert_eq!(issued["session_id"], session_id);
    assert_eq!(issued["token_type"], "session_automation_access_token");
    assert!(issued["token"].as_str().unwrap().starts_with("v1."));
    assert!(issued["expires_at"].is_string());
    assert_eq!(issued["automation"]["endpoint_url"], "http://host:9223");
    assert_eq!(issued["automation"]["protocol"], "chrome_devtools_protocol");
    assert_eq!(
        issued["automation"]["auth_type"],
        "session_automation_access_token"
    );
    assert_eq!(
        issued["automation"]["auth_header"],
        "x-bpane-automation-access-token"
    );
    assert_eq!(
        issued["automation"]["status_path"],
        format!("/api/v1/sessions/{session_id}/status")
    );
    assert_eq!(
        issued["automation"]["mcp_owner_path"],
        format!("/api/v1/sessions/{session_id}/mcp-owner")
    );
    assert_eq!(
        issued["automation"]["compatibility_mode"],
        "legacy_single_runtime"
    );
}

#[tokio::test]
async fn automation_access_token_can_drive_status_and_mcp_owner_routes() {
    let (app, token, _agent_server) = test_router_with_live_agent().await;

    let created = response_json(
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
    let session_id = created["id"].as_str().unwrap().to_string();

    let issued = response_json(
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
    let automation_token = issued["token"].as_str().unwrap();

    let status_before = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/sessions/{session_id}/status"))
                .header("x-bpane-automation-access-token", automation_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(status_before.status(), StatusCode::OK);
    let status_before_body = response_json(status_before).await;
    assert_eq!(status_before_body["mcp_owner"], false);

    let claim_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/mcp-owner"))
                .header("x-bpane-automation-access-token", automation_token)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "width": 1280, "height": 720 }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(claim_response.status(), StatusCode::OK);

    let status_after_claim = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/sessions/{session_id}/status"))
                .header("x-bpane-automation-access-token", automation_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(status_after_claim.status(), StatusCode::OK);
    let status_after_claim_body = response_json(status_after_claim).await;
    assert_eq!(status_after_claim_body["mcp_owner"], true);

    let clear_response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/sessions/{session_id}/mcp-owner"))
                .header("x-bpane-automation-access-token", automation_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(clear_response.status(), StatusCode::OK);
}

#[tokio::test]
async fn owner_can_set_and_clear_session_automation_delegate() {
    let (app, token) = test_router();

    let created = response_json(
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
    let session_id = created["id"].as_str().unwrap().to_string();

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
                        "issuer": "https://issuer.example",
                        "display_name": "BrowserPane MCP bridge"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delegated.status(), StatusCode::OK);
    let delegated_body = response_json(delegated).await;
    assert_eq!(
        delegated_body["automation_delegate"]["client_id"],
        "bpane-mcp-bridge"
    );

    let cleared = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/sessions/{session_id}/automation-owner"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cleared.status(), StatusCode::OK);
    let cleared_body = response_json(cleared).await;
    assert!(cleared_body["automation_delegate"].is_null());
}
