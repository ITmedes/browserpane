use super::*;

#[tokio::test]
async fn issues_a_short_lived_scoped_admin_event_access_token() {
    let (app, owner_token, state) = test_router_with_state();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/admin/events/access-tokens")
                .header("authorization", bearer(&owner_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.headers()["pragma"], "no-cache");
    let body = response_json(response).await;
    assert_eq!(body["token_type"], "admin_event_access_token");
    assert_eq!(body["websocket"]["endpoint_path"], "/api/v1/admin/events");
    assert_eq!(body["websocket"]["auth_type"], "initial_message");
    assert_eq!(
        body["websocket"]["authentication_message_type"],
        "admin.authenticate"
    );
    assert_eq!(
        body["websocket"]["authenticated_message_type"],
        "admin.authenticated"
    );

    let event_token = body["token"].as_str().unwrap();
    assert!(event_token.starts_with("v2.admin-events."));
    let claims = state
        .admin_event_access_token_manager
        .validate_token(event_token)
        .unwrap();
    assert_eq!(claims.issuer, "bpane-gateway");
    assert!(claims.subject.starts_with("legacy-dev-token:"));
    assert!(state
        .admin_event_access_token_manager
        .validate_token(&owner_token)
        .is_err());
}

#[tokio::test]
async fn rejects_admin_event_token_issuance_without_owner_authentication() {
    let (app, _) = test_router();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/admin/events/access-tokens")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
