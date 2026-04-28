use super::*;

#[tokio::test]
async fn creates_extensions_and_applies_them_to_docker_sessions() {
    let (app, token) = test_router_with_docker_pool().await;

    let create_extension_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/extensions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "workflow-extension",
                        "description": "Approved workflow extension",
                        "labels": { "suite": "contract" }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_extension_response.status(), StatusCode::CREATED);
    let extension = response_json(create_extension_response).await;
    let extension_id = extension["id"].as_str().unwrap().to_string();

    let create_version_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/extensions/{extension_id}/versions"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "version": "1.0.0",
                        "install_path": "/home/bpane/bpane-test-extension"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_version_response.status(), StatusCode::CREATED);
    let version = response_json(create_version_response).await;

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
                        "labels": { "suite": "contract" },
                        "extension_ids": [extension_id]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_session_response.status(), StatusCode::CREATED);
    let session = response_json(create_session_response).await;
    assert_eq!(session["extensions"].as_array().unwrap().len(), 1);
    assert_eq!(session["extensions"][0]["extension_id"], extension_id);
    assert_eq!(session["extensions"][0]["name"], "workflow-extension");
    assert_eq!(session["extensions"][0]["version"], "1.0.0");
    assert_eq!(
        session["extensions"][0]["extension_version_id"],
        version["id"].as_str().unwrap()
    );
}
