use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use super::*;
use crate::auth::AuthenticatedPrincipal;
use crate::automation_tasks::{AutomationTaskSessionSource, PersistAutomationTaskRequest};
use crate::session_control::{
    CreateSessionRequest, SessionOwnerMode, SessionRecordingPolicy, SessionStore,
};
use crate::session_manager::{SessionManager, SessionManagerConfig, SessionManagerProfile};
use crate::session_registry::SessionRegistry;
use crate::workflow::{
    PersistWorkflowDefinitionRequest, PersistWorkflowDefinitionVersionRequest,
    PersistWorkflowRunRequest,
};

mod dispatch;
mod runtime_holds;

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
