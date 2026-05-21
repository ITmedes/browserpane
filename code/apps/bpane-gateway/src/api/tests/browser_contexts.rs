use super::*;

#[tokio::test]
async fn manages_browser_context_catalog_and_reusable_session_binding() {
    let (app, token) = test_router();

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/browser-contexts")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "support-profile",
                        "description": "Support engineer profile",
                        "labels": { "team": "support" }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let context = response_json(create_response).await;
    let context_id = context["id"].as_str().unwrap().to_string();
    assert_eq!(context["name"], "support-profile");
    assert_eq!(context["persistence_mode"], "reusable");
    assert_eq!(context["state"], "ready");
    assert!(context["last_used_at"].is_null());

    let duplicate_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/browser-contexts")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "name": "support-profile" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(duplicate_response.status(), StatusCode::CONFLICT);

    let get_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/browser-contexts/{context_id}"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_response.status(), StatusCode::OK);
    let fetched = response_json(get_response).await;
    assert_eq!(fetched["id"], context_id);

    let session_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "browser_context": {
                            "mode": "reusable",
                            "context_id": context_id
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(session_response.status(), StatusCode::CREATED);
    let session = response_json(session_response).await;
    assert_eq!(session["browser_context"]["mode"], "reusable");
    assert_eq!(session["browser_context"]["context_id"], context_id);

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/browser-contexts")
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    let list = response_json(list_response).await;
    assert_eq!(list["contexts"].as_array().unwrap().len(), 1);
    assert_eq!(list["contexts"][0]["id"], context_id);
    assert!(!list["contexts"][0]["last_used_at"].is_null());

    let delete_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/browser-contexts/{context_id}"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_response.status(), StatusCode::OK);
    let deleted = response_json(delete_response).await;
    assert_eq!(deleted["state"], "deleted");
    assert!(!deleted["deleted_at"].is_null());

    let deleted_context_session = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "browser_context": {
                            "mode": "reusable",
                            "context_id": context_id
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(deleted_context_session.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn rejects_invalid_browser_context_requests_and_bindings() {
    let (app, token) = test_router();

    let unauthorized = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/browser-contexts")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

    for body in [
        json!({ "name": "" }),
        json!({ "name": "bad-description", "description": "" }),
        json!({ "name": "bad-label", "labels": { "": "value" } }),
        json!({ "name": "bad-label-value", "labels": { "team": "" } }),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/browser-contexts")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    let missing_context_id = Uuid::now_v7();
    let missing_get = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/browser-contexts/{missing_context_id}"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_get.status(), StatusCode::NOT_FOUND);

    for body in [
        json!({ "browser_context": { "mode": "reusable" } }),
        json!({
            "browser_context": {
                "mode": "fresh",
                "context_id": Uuid::now_v7().to_string()
            }
        }),
        json!({
            "browser_context": {
                "mode": "reusable",
                "context_id": missing_context_id.to_string()
            }
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
        assert!(
            matches!(
                response.status(),
                StatusCode::BAD_REQUEST | StatusCode::NOT_FOUND
            ),
            "unexpected status: {}",
            response.status()
        );
    }
}
