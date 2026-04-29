use std::time::Duration;

use chrono::Utc;
use uuid::Uuid;

use super::*;

#[test]
fn event_matching_supports_exact_and_family_patterns() {
    assert!(workflow_event_type_matches(
        &["workflow_run.created".to_string()],
        "workflow_run.created"
    ));
    assert!(!workflow_event_type_matches(
        &["workflow_run.created".to_string()],
        "workflow_run.succeeded"
    ));
    assert!(workflow_event_type_matches(
        &["workflow_run.*".to_string()],
        "workflow_run.awaiting_input"
    ));
    assert!(!workflow_event_type_matches(
        &["workflow_run.*".to_string()],
        "automation_task.running"
    ));
}

#[test]
fn signature_is_stable_for_timestamp_and_body() {
    let signature = sign_workflow_event_delivery("secret", "1714235400", br#"{"ok":true}"#)
        .expect("signature should be created");
    assert_eq!(
        signature,
        "v1=0604501d383ecf7376c043b341360645fc547f7977af54e29fcef2cc4e249289"
    );
}

#[test]
fn backoff_grows_and_caps() {
    assert_eq!(
        backoff_for_attempt(Duration::from_secs(2), 1),
        Duration::from_secs(2)
    );
    assert_eq!(
        backoff_for_attempt(Duration::from_secs(2), 2),
        Duration::from_secs(4)
    );
    assert_eq!(
        backoff_for_attempt(Duration::from_secs(2), 4),
        Duration::from_secs(16)
    );
    assert_eq!(
        backoff_for_attempt(Duration::from_secs(120), 8),
        Duration::from_secs(300)
    );
}

#[test]
fn workflow_event_deliveries_sort_by_lifecycle_order() {
    let created_at = Utc::now();
    let subscription_id = Uuid::now_v7();
    let run_id = Uuid::now_v7();
    let created_event_id = Uuid::parse_str("00000000-0000-7000-8000-000000000001").unwrap();
    let running_event_id = Uuid::parse_str("00000000-0000-7000-8000-000000000002").unwrap();
    let succeeded_event_id = Uuid::parse_str("00000000-0000-7000-8000-000000000003").unwrap();
    let mut deliveries = vec![
        StoredWorkflowEventDelivery {
            id: Uuid::now_v7(),
            subscription_id,
            run_id,
            event_id: succeeded_event_id,
            event_type: "workflow_run.succeeded".to_string(),
            target_url: "http://example.test/hook".to_string(),
            signing_secret: "secret".to_string(),
            payload: serde_json::json!({ "event_type": "workflow_run.succeeded" }),
            state: WorkflowEventDeliveryState::Pending,
            attempt_count: 0,
            next_attempt_at: Some(created_at),
            last_attempt_at: None,
            delivered_at: None,
            last_response_status: None,
            last_error: None,
            created_at,
            updated_at: created_at,
        },
        StoredWorkflowEventDelivery {
            id: Uuid::now_v7(),
            subscription_id,
            run_id,
            event_id: created_event_id,
            event_type: "workflow_run.created".to_string(),
            target_url: "http://example.test/hook".to_string(),
            signing_secret: "secret".to_string(),
            payload: serde_json::json!({ "event_type": "workflow_run.created" }),
            state: WorkflowEventDeliveryState::Pending,
            attempt_count: 0,
            next_attempt_at: Some(created_at),
            last_attempt_at: None,
            delivered_at: None,
            last_response_status: None,
            last_error: None,
            created_at,
            updated_at: created_at,
        },
        StoredWorkflowEventDelivery {
            id: Uuid::now_v7(),
            subscription_id,
            run_id,
            event_id: running_event_id,
            event_type: "workflow_run.running".to_string(),
            target_url: "http://example.test/hook".to_string(),
            signing_secret: "secret".to_string(),
            payload: serde_json::json!({ "event_type": "workflow_run.running" }),
            state: WorkflowEventDeliveryState::Pending,
            attempt_count: 0,
            next_attempt_at: Some(created_at),
            last_attempt_at: None,
            delivered_at: None,
            last_response_status: None,
            last_error: None,
            created_at,
            updated_at: created_at,
        },
    ];

    deliveries.swap(0, 1);

    sort_workflow_event_deliveries(&mut deliveries);

    let event_types = deliveries
        .iter()
        .map(|delivery| delivery.event_type.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        event_types,
        vec![
            "workflow_run.created",
            "workflow_run.running",
            "workflow_run.succeeded"
        ]
    );
}
