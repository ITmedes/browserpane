use crate::workflow_event_delivery::{
    sign_workflow_event_delivery, WorkflowEventDeliveryConfig, WorkflowEventDeliveryManager,
};

use super::super::*;

#[tokio::test]
async fn workflow_event_subscriptions_dispatch_signed_run_events_and_expose_diagnostics() {
    let (app, token, state) = test_router_with_state();
    let receiver = TestWebhookReceiver::start(vec![StatusCode::OK]).await;

    let subscription_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workflow-event-subscriptions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "run-events",
                        "target_url": receiver.url,
                        "event_types": ["workflow_run.created"],
                        "signing_secret": "super-secret"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(subscription_response.status(), StatusCode::CREATED);
    let subscription = response_json(subscription_response).await;
    let subscription_id = subscription["id"].as_str().unwrap().to_string();

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/workflow-event-subscriptions")
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    let subscriptions = response_json(list_response).await;
    assert_eq!(subscriptions["subscriptions"].as_array().unwrap().len(), 1);

    let workflow = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/workflows")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({ "name": "evented-workflow" }).to_string(),
                    ))
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
                        "entrypoint": "workflows/evented/run.mjs"
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
                        },
                        "source_system": "test-suite",
                        "source_reference": "run-1"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_run.status(), StatusCode::CREATED);
    let run = response_json(create_run).await;
    let run_id = run["id"].as_str().unwrap().to_string();

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
    )
    .unwrap();
    manager.reconcile_persisted_state().await.unwrap();
    manager.run_dispatch_pass().await.unwrap();

    for _ in 0..20 {
        if receiver.requests().await.len() == 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let requests = receiver.requests().await;
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert_eq!(request.body["run_id"], run_id);
    assert_eq!(request.body["event_type"], "workflow_run.created");
    let timestamp = request
        .headers
        .get("x-bpane-signature-timestamp")
        .unwrap()
        .to_string();
    let signature = request
        .headers
        .get("x-bpane-signature-v1")
        .unwrap()
        .to_string();
    let expected_signature = sign_workflow_event_delivery(
        "super-secret",
        &timestamp,
        &serde_json::to_vec(&request.body).unwrap(),
    )
    .unwrap();
    assert_eq!(signature, expected_signature);

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
    assert_eq!(deliveries["deliveries"].as_array().unwrap().len(), 1);
    let delivery = &deliveries["deliveries"][0];
    assert_eq!(delivery["event_type"], "workflow_run.created");
    assert_eq!(delivery["state"], "delivered");
    assert_eq!(delivery["attempt_count"], 1);
    assert_eq!(delivery["attempts"].as_array().unwrap().len(), 1);
    assert_eq!(delivery["attempts"][0]["response_status"], 200);

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
    assert_eq!(ops_response.status(), StatusCode::OK);
    let ops = response_json(ops_response).await;
    assert_eq!(ops["event_delivery_attempts_total"], 1);
    assert_eq!(ops["event_delivery_successes_total"], 1);

    let delete_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/v1/workflow-event-subscriptions/{subscription_id}"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_response.status(), StatusCode::OK);
}
