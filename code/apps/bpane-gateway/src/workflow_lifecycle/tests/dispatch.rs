use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tempfile::tempdir;
use tokio::time::sleep;

use super::*;
use crate::auth::AuthValidator;
use crate::session_access::SessionAutomationAccessTokenManager;
use crate::session_control::PersistedWorkflowRunWorkerAssignment;

#[tokio::test]
async fn launches_worker_and_marks_unfinished_run_failed() {
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
        Some(WorkflowWorkerConfig {
            docker_bin: script,
            image: "deploy-workflow-worker:test".to_string(),
            max_active_workers: 0,
            network: Some("deploy_bpane-internal".to_string()),
            container_name_prefix: "bpane-workflow".to_string(),
            gateway_api_url: "http://gateway:8932".to_string(),
            work_root: PathBuf::from("/tmp/bpane-workflows"),
            bearer_token: Some("token".to_string()),
            oidc_token_url: None,
            oidc_client_id: None,
            oidc_client_secret: None,
            oidc_scopes: None,
        }),
        auth,
        automation_access_token_manager,
        store.clone(),
        test_session_manager(),
        test_registry(),
    )
    .unwrap();
    let run = create_workflow_run(&store).await;

    manager
        .ensure_run_started("playwright", run.id)
        .await
        .unwrap();

    for _ in 0..200 {
        if capture_file.exists() {
            break;
        }
        sleep(Duration::from_millis(20)).await;
    }
    assert!(capture_file.exists());

    let capture = fs::read_to_string(&capture_file).unwrap();
    assert!(capture.contains("run"));
    assert!(capture.contains("BPANE_WORKFLOW_RUN_ID"));
    assert!(capture.contains(&run.id.to_string()));
    assert!(capture.contains("BPANE_SESSION_AUTOMATION_ACCESS_TOKEN"));
    assert!(capture.contains("deploy-workflow-worker:test"));

    let mut latest = None;
    for _ in 0..50 {
        latest = store.get_workflow_run_by_id(run.id).await.unwrap();
        if latest.as_ref().is_some_and(|run| run.state.is_terminal()) {
            break;
        }
        sleep(Duration::from_millis(10)).await;
    }

    let failed = latest.expect("workflow run should exist");
    assert!(matches!(failed.state, WorkflowRunState::Failed));
}

#[tokio::test]
async fn reconcile_fails_stale_run_assignment() {
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
        .upsert_workflow_run_worker_assignment(PersistedWorkflowRunWorkerAssignment {
            run_id: run.id,
            session_id: run.session_id,
            automation_task_id: run.automation_task_id,
            status: WorkflowRunWorkerAssignmentStatus::Running,
            process_id: Some(7777),
            container_name: Some("bpane-workflow-stale".to_string()),
        })
        .await
        .unwrap();

    manager.reconcile_persisted_state().await.unwrap();

    let failed = store.get_workflow_run_by_id(run.id).await.unwrap().unwrap();
    assert!(matches!(failed.state, WorkflowRunState::Failed));
    assert!(store
        .get_workflow_run_worker_assignment(run.id)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn ensure_run_started_reconciles_stale_terminal_task_before_dispatch() {
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
        .cancel_automation_task_for_owner(&test_principal(), run.automation_task_id)
        .await
        .unwrap()
        .unwrap();

    manager
        .ensure_run_started("playwright", run.id)
        .await
        .unwrap();

    let current = store.get_workflow_run_by_id(run.id).await.unwrap().unwrap();
    assert_eq!(current.state, WorkflowRunState::Cancelled);
    assert!(!capture_file.exists());
}

#[tokio::test]
async fn queues_waiting_run_when_worker_capacity_is_exhausted() {
    let temp_dir = tempdir().unwrap();
    let capture_file = temp_dir.path().join("capture.txt");
    let script = create_sleep_capture_script(&temp_dir, &capture_file, 0.3);
    let store = SessionStore::in_memory_with_config(SessionManagerProfile {
        runtime_binding: "workflow_test_pool".to_string(),
        compatibility_mode: "session_runtime_pool".to_string(),
        max_runtime_sessions: 4,
        supports_legacy_global_routes: false,
        supports_session_extensions: true,
    });
    let auth = Arc::new(AuthValidator::from_hmac_secret(vec![9; 32]));
    let automation_access_token_manager = Arc::new(SessionAutomationAccessTokenManager::new(
        vec![7; 32],
        Duration::from_secs(300),
    ));
    let manager = WorkflowLifecycleManager::new(
        Some(WorkflowWorkerConfig {
            max_active_workers: 1,
            ..test_config(script)
        }),
        auth,
        automation_access_token_manager,
        store.clone(),
        test_session_manager(),
        test_registry(),
    )
    .unwrap();
    let first_run = create_workflow_run(&store).await;
    let queued_run = create_workflow_run(&store).await;

    manager
        .ensure_run_started("playwright", first_run.id)
        .await
        .unwrap();
    manager
        .ensure_run_started("playwright", queued_run.id)
        .await
        .unwrap();

    let queued = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let current = store
                .get_workflow_run_by_id(queued_run.id)
                .await
                .unwrap()
                .expect("queued workflow run should exist");
            if current.state == WorkflowRunState::Queued {
                break current;
            }
            sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("queued run should enter queued state");
    assert_eq!(queued.state, WorkflowRunState::Queued);

    let queued_events = store
        .list_workflow_run_events_for_owner(&test_principal(), queued_run.id)
        .await
        .unwrap();
    assert!(queued_events
        .iter()
        .any(|event| event.event_type == "workflow_run.queued"));

    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let current = store
                .get_workflow_run_by_id(first_run.id)
                .await
                .unwrap()
                .expect("first workflow run should exist");
            if current.state.is_terminal() {
                break;
            }
            sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("first workflow run should complete");

    manager.reconcile_waiting_runs().await.unwrap();

    let dispatched = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let current = store
                .get_workflow_run_by_id(queued_run.id)
                .await
                .unwrap()
                .expect("queued workflow run should exist");
            if current.state.is_terminal() {
                break current;
            }
            sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("queued run should eventually dispatch and complete");
    assert!(matches!(dispatched.state, WorkflowRunState::Failed));

    let capture = fs::read_to_string(&capture_file).unwrap();
    assert!(capture.contains(&first_run.id.to_string()));
    assert!(capture.contains(&queued_run.id.to_string()));
}
