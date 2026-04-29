use crate::workflow_event_delivery::{WorkflowEventDeliveryConfig, WorkflowEventDeliveryManager};

use super::super::*;

#[tokio::test]
async fn workflow_event_subscriptions_preserve_lifecycle_delivery_order() {
    let (app, token, state) = test_router_with_state();
    let receiver =
        TestWebhookReceiver::start(vec![StatusCode::OK, StatusCode::OK, StatusCode::OK]).await;

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
                            "name": "ordered-events",
                            "target_url": receiver.url,
                            "event_types": [
                                "workflow_run.created",
                                "workflow_run.running",
                                "workflow_run.succeeded"
                            ],
                            "signing_secret": "ordered-secret"
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
                        json!({ "name": "ordered-event-workflow" }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let workflow_id = workflow["id"].as_str().unwrap().to_string();

    response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/workflows/{workflow_id}/versions"))
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "version": "v1",
                            "executor": "manual",
                            "entrypoint": "workflows/ordered-events/run.mjs"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;

    let run = response_json(
        app.clone()
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
            .unwrap(),
    )
    .await;
    let run_id = run["id"].as_str().unwrap().to_string();
    let session_id = run["session_id"].as_str().unwrap().to_string();

    let automation_access = response_json(
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
    let automation_token = automation_access["token"].as_str().unwrap().to_string();

    let running_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/workflow-runs/{run_id}/state"))
                .header("x-bpane-automation-access-token", automation_token.as_str())
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "state": "running",
                        "message": "manual executor started"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(running_response.status(), StatusCode::OK);

    let succeeded_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/workflow-runs/{run_id}/state"))
                .header("x-bpane-automation-access-token", automation_token.as_str())
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "state": "succeeded",
                        "message": "manual executor finished"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(succeeded_response.status(), StatusCode::OK);

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
        if receiver.requests().await.len() == 3 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let requests = receiver.requests().await;
    assert_eq!(requests.len(), 3);
    let event_types = requests
        .iter()
        .map(|request| request.body["event_type"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert_eq!(
        event_types,
        vec![
            "workflow_run.created".to_string(),
            "workflow_run.running".to_string(),
            "workflow_run.succeeded".to_string()
        ]
    );

    let deliveries = response_json(
        app.clone()
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
            .unwrap(),
    )
    .await;
    let delivered_event_types = deliveries["deliveries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|delivery| delivery["event_type"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert_eq!(delivered_event_types, event_types);
}
