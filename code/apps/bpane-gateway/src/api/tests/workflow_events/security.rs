use crate::workflow_event_delivery::{
    WorkflowEventDeliveryConfig, WorkflowEventDeliveryManager, WorkflowEventDestinationPolicy,
};

use super::super::*;

#[tokio::test]
async fn workflow_event_subscription_api_rejects_unsafe_targets() {
    let (app, token) = test_router();
    let cases = [
        ("/events", "must be a valid absolute URL"),
        ("ftp://example.com/events", "must use http or https"),
        (
            "https://user:secret@example.com/events",
            "must not include credentials",
        ),
        (
            "https://example.com/events#fragment",
            "must not include a fragment",
        ),
        (
            "http://example.com/events",
            "must use https unless its exact origin is allowed",
        ),
        (
            "https://127.0.0.1/events",
            "resolves to a non-public address",
        ),
        (
            "https://169.254.169.254/events",
            "resolves to a non-public address",
        ),
        ("https://[::1]/events", "resolves to a non-public address"),
        (
            "https://[::ffff:127.0.0.1]/events",
            "resolves to a non-public address",
        ),
    ];

    for (index, (target_url, expected_error)) in cases.into_iter().enumerate() {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/workflow-event-subscriptions")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": format!("unsafe-target-{index}"),
                            "target_url": target_url,
                            "event_types": ["workflow_run.created"],
                            "signing_secret": "test-secret"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{target_url}");
        let body = response_json(response).await;
        assert!(
            body["error"]
                .as_str()
                .is_some_and(|error| error.contains(expected_error)),
            "unexpected error for {target_url}: {body}"
        );
    }
}

#[tokio::test]
async fn workflow_event_subscription_api_persists_the_canonical_target() {
    let allowed_origin = "http://127.0.0.1:8088".to_string();
    let destination_policy = Arc::new(
        WorkflowEventDestinationPolicy::new(&[allowed_origin]).expect("valid test origin"),
    );
    let (app, token, _state) =
        test_router_with_workflow_event_destination_policy(destination_policy);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workflow-event-subscriptions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "canonical-target",
                        "target_url": "HTTP://127.0.0.1:8088/events?kind=workflow",
                        "event_types": ["workflow_run.created"],
                        "signing_secret": "test-secret"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let subscription = response_json(response).await;
    assert_eq!(
        subscription["target_url"],
        "http://127.0.0.1:8088/events?kind=workflow"
    );
}

#[tokio::test]
async fn dispatch_revalidates_and_fails_a_persisted_unsafe_target_without_retry() {
    let receiver = TestWebhookReceiver::start(vec![StatusCode::OK]).await;
    let persisted_target = receiver.url.replacen("http://", "https://", 1);
    let allowed_origin = reqwest::Url::parse(&persisted_target)
        .unwrap()
        .origin()
        .ascii_serialization();
    let api_policy = Arc::new(WorkflowEventDestinationPolicy::new(&[allowed_origin]).unwrap());
    let (app, token, state) = test_router_with_workflow_event_destination_policy(api_policy);

    let subscription = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/workflow-event-subscriptions")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": "persisted-unsafe-target",
                            "target_url": persisted_target,
                            "event_types": ["workflow_run.created"],
                            "signing_secret": "test-secret"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let subscription_id = subscription["id"].as_str().unwrap().to_string();

    let workflow = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/workflows")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({ "name": "unsafe-event-workflow" }).to_string(),
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
                        "entrypoint": "workflows/security/run.mjs"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(version_response.status(), StatusCode::CREATED);

    let run_response = app
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
                        "session": { "create_session": {} }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(run_response.status(), StatusCode::CREATED);

    let manager = WorkflowEventDeliveryManager::new(
        state.session_store.clone(),
        state.workflow_observability.clone(),
        WorkflowEventDeliveryConfig {
            poll_interval: Duration::from_millis(5),
            request_timeout: Duration::from_secs(2),
            max_attempts: 3,
            batch_size: 8,
            base_backoff: Duration::from_millis(5),
        },
        Arc::new(WorkflowEventDestinationPolicy::default()),
    )
    .unwrap();
    manager.run_dispatch_pass().await.unwrap();

    assert!(receiver.requests().await.is_empty());
    let deliveries_response = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/workflow-event-subscriptions/{subscription_id}/deliveries"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(deliveries_response.status(), StatusCode::OK);
    let deliveries = response_json(deliveries_response).await;
    let delivery = &deliveries["deliveries"][0];
    assert_eq!(delivery["state"], "failed");
    assert_eq!(delivery["attempt_count"], 1);
    assert!(delivery["next_attempt_at"].is_null());
    assert!(delivery["last_error"]
        .as_str()
        .is_some_and(|error| error.contains("non-public address")));
}
