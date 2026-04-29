use std::sync::Arc;
use std::time::Duration;

use tempfile::tempdir;
use tokio::time::sleep;

use super::*;
use crate::auth::AuthValidator;
use crate::session_access::SessionAutomationAccessTokenManager;
use crate::session_control::SessionLifecycleState;
use crate::workflow::WorkflowRunTransitionRequest;

#[tokio::test]
async fn releases_runtime_immediately_when_awaiting_input_has_no_live_hold() {
    let temp_dir = tempdir().unwrap();
    let capture_file = temp_dir.path().join("capture.txt");
    let script = create_capture_script(&temp_dir, &capture_file);
    let store = SessionStore::in_memory();
    let auth = Arc::new(AuthValidator::from_hmac_secret(vec![9; 32]));
    let automation_access_token_manager = Arc::new(SessionAutomationAccessTokenManager::new(
        vec![7; 32],
        Duration::from_secs(300),
    ));
    let manager = WorkflowLifecycleManager::new(
        Some(test_config(script)),
        auth,
        automation_access_token_manager,
        store.clone(),
        test_session_manager(),
        test_registry(),
    )
    .unwrap();
    let run = create_workflow_run(&store).await;

    store
        .transition_workflow_run(
            run.id,
            WorkflowRunTransitionRequest {
                state: WorkflowRunState::Running,
                output: None,
                error: None,
                artifact_refs: Vec::new(),
                message: Some("executor attached".to_string()),
                data: None,
            },
        )
        .await
        .unwrap();

    store
        .transition_workflow_run(
            run.id,
            WorkflowRunTransitionRequest {
                state: WorkflowRunState::AwaitingInput,
                output: None,
                error: None,
                artifact_refs: Vec::new(),
                message: Some("awaiting operator input".to_string()),
                data: Some(serde_json::json!({
                    "intervention_request": {
                        "kind": "approval"
                    }
                })),
            },
        )
        .await
        .unwrap();
    manager.reconcile_runtime_hold(run.id).await.unwrap();

    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let session = store
                .get_session_by_id(run.session_id)
                .await
                .unwrap()
                .expect("workflow session should exist");
            if session.state == SessionLifecycleState::Stopped {
                break;
            }
            sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("workflow session should be released");

    let events = store.list_workflow_run_events(run.id).await.unwrap();
    assert!(events
        .iter()
        .any(|event| event.event_type == "workflow_run.runtime_released"));
}

#[tokio::test]
async fn keeps_runtime_live_until_hold_timeout_then_releases_it() {
    let temp_dir = tempdir().unwrap();
    let capture_file = temp_dir.path().join("capture.txt");
    let script = create_capture_script(&temp_dir, &capture_file);
    let store = SessionStore::in_memory();
    let auth = Arc::new(AuthValidator::from_hmac_secret(vec![9; 32]));
    let automation_access_token_manager = Arc::new(SessionAutomationAccessTokenManager::new(
        vec![7; 32],
        Duration::from_secs(300),
    ));
    let manager = WorkflowLifecycleManager::new(
        Some(test_config(script)),
        auth,
        automation_access_token_manager,
        store.clone(),
        test_session_manager(),
        test_registry(),
    )
    .unwrap();
    let run = create_workflow_run(&store).await;

    store
        .transition_workflow_run(
            run.id,
            WorkflowRunTransitionRequest {
                state: WorkflowRunState::Running,
                output: None,
                error: None,
                artifact_refs: Vec::new(),
                message: Some("executor attached".to_string()),
                data: None,
            },
        )
        .await
        .unwrap();

    store
        .transition_workflow_run(
            run.id,
            WorkflowRunTransitionRequest {
                state: WorkflowRunState::AwaitingInput,
                output: None,
                error: None,
                artifact_refs: Vec::new(),
                message: Some("awaiting operator input".to_string()),
                data: Some(serde_json::json!({
                    "intervention_request": {
                        "kind": "approval"
                    },
                    "runtime_hold": {
                        "mode": "live",
                        "timeout_sec": 1
                    }
                })),
            },
        )
        .await
        .unwrap();
    manager.reconcile_runtime_hold(run.id).await.unwrap();

    let session = store
        .get_session_by_id(run.session_id)
        .await
        .unwrap()
        .expect("workflow session should exist");
    assert_eq!(session.state, SessionLifecycleState::Ready);

    let events = store.list_workflow_run_events(run.id).await.unwrap();
    assert!(events
        .iter()
        .any(|event| event.event_type == "workflow_run.runtime_held"));

    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let session = store
                .get_session_by_id(run.session_id)
                .await
                .unwrap()
                .expect("workflow session should exist");
            if session.state == SessionLifecycleState::Stopped {
                break;
            }
            sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("workflow session should be released after hold timeout");

    let events = store.list_workflow_run_events(run.id).await.unwrap();
    assert!(events
        .iter()
        .any(|event| event.event_type == "workflow_run.runtime_released"));
}
