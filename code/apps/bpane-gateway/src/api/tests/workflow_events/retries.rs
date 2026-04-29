use crate::workflow_event_delivery::{WorkflowEventDeliveryConfig, WorkflowEventDeliveryManager};

use super::super::*;

#[tokio::test]
async fn workflow_event_delivery_retries_retryable_failures_and_then_succeeds() {
    let (app, token, state) = test_router_with_state();
    let receiver = TestWebhookReceiver::start(vec![StatusCode::BAD_GATEWAY, StatusCode::OK]).await;

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
                            "name": "retry-events",
                            "target_url": receiver.url,
                            "event_types": ["workflow_run.created"],
                            "signing_secret": "retry-secret"
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
                    .body(Body::from(json!({ "name": "retry-workflow" }).to_string()))
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
                        "entrypoint": "workflows/retry/run.mjs"
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
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_run.status(), StatusCode::CREATED);

    let manager = WorkflowEventDeliveryManager::new(
        state.session_store.clone(),
        state.workflow_observability.clone(),
        WorkflowEventDeliveryConfig {
            poll_interval: Duration::from_millis(5),
            request_timeout: Duration::from_secs(2),
            max_attempts: 3,
            batch_size: 8,
            base_backoff: Duration::from_millis(1),
        },
    )
    .unwrap();
    manager.reconcile_persisted_state().await.unwrap();
    manager.run_dispatch_pass().await.unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    manager.run_dispatch_pass().await.unwrap();

    for _ in 0..20 {
        if receiver.requests().await.len() == 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let requests = receiver.requests().await;
    assert_eq!(requests.len(), 2);

    let deliveries_response = app
        .clone()
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
    assert_eq!(delivery["state"], "delivered");
    assert_eq!(delivery["attempt_count"], 2);
    assert_eq!(delivery["attempts"].as_array().unwrap().len(), 2);
    assert_eq!(delivery["attempts"][0]["response_status"], 502);
    assert_eq!(delivery["attempts"][1]["response_status"], 200);

    let ops_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/workflow/operations")
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let ops = response_json(ops_response).await;
    assert_eq!(ops["event_delivery_attempts_total"], 2);
    assert_eq!(ops["event_delivery_successes_total"], 1);
    assert_eq!(ops["event_delivery_retries_total"], 1);
    assert_eq!(ops["event_delivery_failures_total"], 0);
}
