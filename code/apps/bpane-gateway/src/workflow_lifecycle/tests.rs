use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tempfile::tempdir;
use tokio::time::sleep;

use super::*;
use crate::auth::{AuthValidator, AuthenticatedPrincipal};
use crate::automation_access_token::SessionAutomationAccessTokenManager;
use crate::automation_task::{AutomationTaskSessionSource, PersistAutomationTaskRequest};
use crate::session_control::{
    CreateSessionRequest, SessionOwnerMode, SessionRecordingPolicy, SessionStore,
};
use crate::session_manager::{SessionManager, SessionManagerConfig, SessionManagerProfile};
use crate::session_registry::SessionRegistry;
use crate::workflow::{
    PersistWorkflowDefinitionRequest, PersistWorkflowDefinitionVersionRequest,
    PersistWorkflowRunRequest,
};

fn test_principal() -> AuthenticatedPrincipal {
    AuthenticatedPrincipal {
        subject: "owner".to_string(),
        issuer: "issuer".to_string(),
        display_name: Some("Owner".to_string()),
        client_id: None,
    }
}

fn test_config(script: PathBuf) -> WorkflowWorkerConfig {
    WorkflowWorkerConfig {
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
    }
}

fn test_session_manager() -> Arc<SessionManager> {
    Arc::new(
        SessionManager::new(SessionManagerConfig::StaticSingle {
            agent_socket_path: "/tmp/bpane-workflow-lifecycle.sock".to_string(),
            cdp_endpoint: Some("http://host:9223".to_string()),
            idle_timeout: Duration::from_secs(300),
        })
        .unwrap(),
    )
}

fn test_registry() -> Arc<SessionRegistry> {
    Arc::new(SessionRegistry::new(10, false))
}

async fn create_workflow_run(store: &SessionStore) -> crate::workflow::StoredWorkflowRun {
    let principal = test_principal();
    let session = store
        .create_session(
            &principal,
            CreateSessionRequest {
                template_id: None,
                owner_mode: None,
                viewport: None,
                idle_timeout_sec: None,
                labels: HashMap::new(),
                integration_context: None,
                extension_ids: Vec::new(),
                extensions: Vec::new(),
                recording: SessionRecordingPolicy::default(),
            },
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap();
    let task = store
        .create_automation_task(
            &principal,
            PersistAutomationTaskRequest {
                display_name: Some("Workflow Task".to_string()),
                executor: "playwright".to_string(),
                session_id: session.id,
                session_source: AutomationTaskSessionSource::CreatedSession,
                input: None,
                labels: HashMap::new(),
            },
        )
        .await
        .unwrap();
    let workflow = store
        .create_workflow_definition(
            &principal,
            PersistWorkflowDefinitionRequest {
                name: "Smoke Workflow".to_string(),
                description: None,
                labels: HashMap::new(),
            },
        )
        .await
        .unwrap();
    let version = store
        .create_workflow_definition_version(
            &principal,
            PersistWorkflowDefinitionVersionRequest {
                workflow_definition_id: workflow.id,
                version: "v1".to_string(),
                executor: "playwright".to_string(),
                entrypoint: "workflows/smoke/run.mjs".to_string(),
                source: None,
                input_schema: None,
                output_schema: None,
                default_session: None,
                allowed_credential_binding_ids: Vec::new(),
                allowed_extension_ids: Vec::new(),
                allowed_file_workspace_ids: Vec::new(),
            },
        )
        .await
        .unwrap();
    store
        .create_workflow_run(
            &principal,
            PersistWorkflowRunRequest {
                workflow_definition_id: workflow.id,
                workflow_definition_version_id: version.id,
                workflow_version: version.version.clone(),
                session_id: session.id,
                automation_task_id: task.id,
                source_system: None,
                source_reference: None,
                client_request_id: None,
                create_request_fingerprint: None,
                source_snapshot: None,
                extensions: Vec::new(),
                credential_bindings: Vec::new(),
                workspace_inputs: Vec::new(),
                input: None,
                labels: HashMap::new(),
            },
        )
        .await
        .unwrap()
        .run
}

fn create_capture_script(dir: &tempfile::TempDir, capture_file: &std::path::Path) -> PathBuf {
    let script_path = dir.path().join("capture-docker.sh");
    fs::write(
        &script_path,
        format!(
            r#"#!/bin/sh
printf '%s\n' "$@" >> "{}"
"#,
            capture_file.display()
        ),
    )
    .unwrap();
    let mut permissions = fs::metadata(&script_path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script_path, permissions).unwrap();
    script_path
}

fn create_sleep_capture_script(
    dir: &tempfile::TempDir,
    capture_file: &std::path::Path,
    sleep_seconds: f32,
) -> PathBuf {
    let script_path = dir.path().join("sleep-capture-docker.sh");
    fs::write(
        &script_path,
        format!(
            r#"#!/bin/sh
printf '%s\n' "$@" >> "{}"
sleep {}
"#,
            capture_file.display(),
            sleep_seconds,
        ),
    )
    .unwrap();
    let mut permissions = fs::metadata(&script_path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script_path, permissions).unwrap();
    script_path
}

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
