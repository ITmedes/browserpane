use std::collections::HashMap;
use std::fs;
use std::io::Cursor;
use std::os::unix::fs::PermissionsExt;
use std::process::Command as StdCommand;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tempfile::tempdir;
use tokio::sync::{oneshot, Mutex};
use tokio::time::sleep;
use tower::ServiceExt;
use zip::ZipArchive;

use super::*;
use crate::auth::AuthValidator;
use crate::automation_access_token::SessionAutomationAccessTokenManager;
use crate::connect_ticket::SessionConnectTicketManager;
use crate::credential_provider::{
    CredentialProvider, CredentialProviderBackend, CredentialProviderError,
    ResolvedCredentialSecret, StoreCredentialSecretRequest, StoredCredentialSecret,
};
use crate::recording_artifact_store::RecordingArtifactStore;
use crate::recording_lifecycle::RecordingLifecycleManager;
use crate::recording_observability::RecordingObservability;
use crate::recording_playback::prepare_session_recording_playback;
use crate::recording_retention::RecordingRetentionManager;
use crate::session_control::{
    SessionRecordingFormat, SessionRecordingMode, SessionRecordingPolicy,
    SessionRecordingState as StoredSessionRecordingState, StoredSessionRecording,
};
use crate::session_manager::{SessionManager, SessionManagerConfig, SessionManagerProfile};
use crate::workflow_lifecycle::{WorkflowLifecycleManager, WorkflowWorkerConfig};
use crate::workflow_observability::WorkflowObservability;
use crate::workflow_source::WorkflowSourceResolver;
use crate::workspace_file_store::WorkspaceFileStore;

mod automation_tasks;
mod credential_bindings;
mod extensions;
mod file_workspaces;
mod recordings;
mod sessions;
mod workflow_events;
mod workflow_run_operations;
mod workflows;

#[derive(Default)]
struct TestCredentialProviderBackend {
    secrets: Mutex<HashMap<String, Value>>,
}

#[async_trait]
impl CredentialProviderBackend for TestCredentialProviderBackend {
    async fn store_secret(
        &self,
        request: StoreCredentialSecretRequest,
    ) -> Result<StoredCredentialSecret, CredentialProviderError> {
        let external_ref = request
            .external_ref
            .unwrap_or_else(|| format!("test/{}", request.binding_id));
        self.secrets
            .lock()
            .await
            .insert(external_ref.clone(), request.payload);
        Ok(StoredCredentialSecret { external_ref })
    }

    async fn resolve_secret(
        &self,
        external_ref: &str,
    ) -> Result<ResolvedCredentialSecret, CredentialProviderError> {
        let payload = self
            .secrets
            .lock()
            .await
            .get(external_ref)
            .cloned()
            .ok_or_else(|| {
                CredentialProviderError::Backend(format!(
                    "test credential secret {external_ref} not found"
                ))
            })?;
        Ok(ResolvedCredentialSecret { payload })
    }
}

fn test_router_with_state() -> (Router, String, Arc<ApiState>) {
    let auth_validator = Arc::new(AuthValidator::from_hmac_secret(vec![7; 32]));
    let token = auth_validator
        .generate_token()
        .expect("hmac auth validator should generate dev token");
    let state = Arc::new(ApiState {
        registry: Arc::new(SessionRegistry::new(10, false)),
        auth_validator,
        connect_ticket_manager: Arc::new(SessionConnectTicketManager::new(
            vec![5; 32],
            Duration::from_secs(300),
        )),
        automation_access_token_manager: Arc::new(SessionAutomationAccessTokenManager::new(
            vec![6; 32],
            Duration::from_secs(300),
        )),
        session_store: SessionStore::in_memory(),
        session_manager: Arc::new(
            SessionManager::new(SessionManagerConfig::StaticSingle {
                agent_socket_path: "/tmp/test.sock".to_string(),
                cdp_endpoint: Some("http://host:9223".to_string()),
                idle_timeout: Duration::from_secs(300),
            })
            .unwrap(),
        ),
        credential_provider: Some(test_credential_provider()),
        recording_artifact_store: test_artifact_store(),
        workspace_file_store: test_workspace_file_store(),
        workflow_source_resolver: test_workflow_source_resolver(),
        recording_observability: Arc::new(RecordingObservability::default()),
        recording_lifecycle: Arc::new(RecordingLifecycleManager::disabled()),
        workflow_lifecycle: Arc::new(WorkflowLifecycleManager::disabled()),
        workflow_observability: Arc::new(WorkflowObservability::default()),
        workflow_log_retention: None,
        workflow_output_retention: None,
        idle_stop_timeout: Duration::from_secs(300),
        public_gateway_url: "https://localhost:4433".to_string(),
        default_owner_mode: SessionOwnerMode::Collaborative,
    });
    (build_api_router(state.clone()), token, state)
}

fn test_router() -> (Router, String) {
    let (router, token, _) = test_router_with_state();
    (router, token)
}

fn test_router_with_workflow_lifecycle(
    config: WorkflowWorkerConfig,
) -> (Router, String, Arc<ApiState>) {
    let auth_validator = Arc::new(AuthValidator::from_hmac_secret(vec![7; 32]));
    let token = auth_validator
        .generate_token()
        .expect("hmac auth validator should generate dev token");
    let automation_access_token_manager = Arc::new(SessionAutomationAccessTokenManager::new(
        vec![6; 32],
        Duration::from_secs(300),
    ));
    let session_store = SessionStore::in_memory_with_config(SessionManagerProfile {
        runtime_binding: "workflow_test_pool".to_string(),
        compatibility_mode: "session_runtime_pool".to_string(),
        max_runtime_sessions: 4,
        supports_legacy_global_routes: false,
        supports_session_extensions: true,
    });
    let session_manager = Arc::new(
        SessionManager::new(SessionManagerConfig::StaticSingle {
            agent_socket_path: "/tmp/test.sock".to_string(),
            cdp_endpoint: Some("http://host:9223".to_string()),
            idle_timeout: Duration::from_secs(300),
        })
        .unwrap(),
    );
    let registry = Arc::new(SessionRegistry::new(10, false));
    let workflow_lifecycle = Arc::new(
        WorkflowLifecycleManager::new(
            Some(config),
            auth_validator.clone(),
            automation_access_token_manager.clone(),
            session_store.clone(),
            session_manager.clone(),
            registry.clone(),
        )
        .expect("workflow lifecycle test config should be valid"),
    );
    let state = Arc::new(ApiState {
        registry,
        auth_validator,
        connect_ticket_manager: Arc::new(SessionConnectTicketManager::new(
            vec![5; 32],
            Duration::from_secs(300),
        )),
        automation_access_token_manager,
        session_store,
        session_manager,
        credential_provider: Some(test_credential_provider()),
        recording_artifact_store: test_artifact_store(),
        workspace_file_store: test_workspace_file_store(),
        workflow_source_resolver: test_workflow_source_resolver(),
        recording_observability: Arc::new(RecordingObservability::default()),
        recording_lifecycle: Arc::new(RecordingLifecycleManager::disabled()),
        workflow_lifecycle,
        workflow_observability: Arc::new(WorkflowObservability::default()),
        workflow_log_retention: None,
        workflow_output_retention: None,
        idle_stop_timeout: Duration::from_secs(300),
        public_gateway_url: "https://localhost:4433".to_string(),
        default_owner_mode: SessionOwnerMode::Collaborative,
    });
    (build_api_router(state.clone()), token, state)
}

fn create_sleep_workflow_worker_script(
    dir: &tempfile::TempDir,
    capture_file: &std::path::Path,
    sleep_seconds: f32,
) -> std::path::PathBuf {
    let script_path = dir.path().join("workflow-worker-test.sh");
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

fn test_artifact_store() -> Arc<RecordingArtifactStore> {
    let root = std::env::temp_dir().join(format!("bpane-artifacts-test-{}", uuid::Uuid::now_v7()));
    Arc::new(RecordingArtifactStore::local_fs(root))
}

fn test_workspace_file_store() -> Arc<WorkspaceFileStore> {
    let root = std::env::temp_dir().join(format!(
        "bpane-workspace-files-test-{}",
        uuid::Uuid::now_v7()
    ));
    Arc::new(WorkspaceFileStore::local_fs(root))
}

fn test_credential_provider() -> Arc<CredentialProvider> {
    Arc::new(CredentialProvider::new(Arc::new(
        TestCredentialProviderBackend::default(),
    )))
}

fn test_workflow_source_resolver() -> Arc<WorkflowSourceResolver> {
    Arc::new(WorkflowSourceResolver::new(std::path::PathBuf::from("git")))
}

#[derive(Debug, Clone)]
struct CapturedWebhookRequest {
    headers: HashMap<String, String>,
    body: Value,
}

#[derive(Clone, Default)]
struct TestWebhookReceiverState {
    requests: Arc<Mutex<Vec<CapturedWebhookRequest>>>,
    statuses: Arc<Mutex<Vec<StatusCode>>>,
}

struct TestWebhookReceiver {
    url: String,
    state: TestWebhookReceiverState,
    shutdown: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<()>,
}

impl TestWebhookReceiver {
    async fn start(statuses: Vec<StatusCode>) -> Self {
        let state = TestWebhookReceiverState {
            requests: Arc::new(Mutex::new(Vec::new())),
            statuses: Arc::new(Mutex::new(statuses)),
        };
        let app = axum::Router::new().route(
            "/events",
            axum::routing::post({
                let state = state.clone();
                move |headers: axum::http::HeaderMap, body: axum::body::Bytes| {
                    let state = state.clone();
                    async move {
                        let body = serde_json::from_slice::<Value>(&body).unwrap();
                        let mut captured_headers = HashMap::new();
                        for name in [
                            "x-bpane-event-id",
                            "x-bpane-event-type",
                            "x-bpane-delivery-id",
                            "x-bpane-subscription-id",
                            "x-bpane-signature-timestamp",
                            "x-bpane-signature-v1",
                        ] {
                            if let Some(value) =
                                headers.get(name).and_then(|value| value.to_str().ok())
                            {
                                captured_headers.insert(name.to_string(), value.to_string());
                            }
                        }
                        state.requests.lock().await.push(CapturedWebhookRequest {
                            headers: captured_headers,
                            body,
                        });
                        let status = {
                            let mut statuses = state.statuses.lock().await;
                            if statuses.is_empty() {
                                StatusCode::OK
                            } else {
                                statuses.remove(0)
                            }
                        };
                        (status, "ok")
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test webhook receiver");
        let address = listener.local_addr().expect("receiver addr");
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = shutdown_rx.await;
                })
                .await
                .expect("run webhook receiver");
        });
        Self {
            url: format!("http://{address}/events"),
            state,
            shutdown: Some(shutdown_tx),
            task,
        }
    }

    async fn requests(&self) -> Vec<CapturedWebhookRequest> {
        self.state.requests.lock().await.clone()
    }
}

impl Drop for TestWebhookReceiver {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        self.task.abort();
    }
}

struct TestAgentServer {
    socket_path: std::path::PathBuf,
    accept_task: tokio::task::JoinHandle<()>,
}

impl TestAgentServer {
    async fn start() -> Self {
        let socket_path = std::path::PathBuf::from(format!(
            "/tmp/bpane-agent-{}.sock",
            uuid::Uuid::now_v7().simple()
        ));
        let _ = std::fs::remove_file(&socket_path);
        let listener = tokio::net::UnixListener::bind(&socket_path).unwrap();
        let accept_task = tokio::spawn(async move {
            let mut connections = Vec::new();
            while let Ok((stream, _)) = listener.accept().await {
                connections.push(stream);
            }
        });

        Self {
            socket_path,
            accept_task,
        }
    }

    fn socket_path(&self) -> String {
        self.socket_path.to_string_lossy().into_owned()
    }
}

impl Drop for TestAgentServer {
    fn drop(&mut self) {
        self.accept_task.abort();
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

async fn test_router_with_live_agent() -> (Router, String, TestAgentServer) {
    let agent_server = TestAgentServer::start().await;
    let auth_validator = Arc::new(AuthValidator::from_hmac_secret(vec![7; 32]));
    let token = auth_validator
        .generate_token()
        .expect("hmac auth validator should generate dev token");
    let state = Arc::new(ApiState {
        registry: Arc::new(SessionRegistry::new(10, false)),
        auth_validator,
        connect_ticket_manager: Arc::new(SessionConnectTicketManager::new(
            vec![5; 32],
            Duration::from_secs(300),
        )),
        automation_access_token_manager: Arc::new(SessionAutomationAccessTokenManager::new(
            vec![6; 32],
            Duration::from_secs(300),
        )),
        session_store: SessionStore::in_memory(),
        session_manager: Arc::new(
            SessionManager::new(SessionManagerConfig::StaticSingle {
                agent_socket_path: agent_server.socket_path(),
                cdp_endpoint: Some("http://host:9223".to_string()),
                idle_timeout: Duration::from_secs(300),
            })
            .unwrap(),
        ),
        credential_provider: Some(test_credential_provider()),
        recording_artifact_store: test_artifact_store(),
        workspace_file_store: test_workspace_file_store(),
        workflow_source_resolver: test_workflow_source_resolver(),
        recording_observability: Arc::new(RecordingObservability::default()),
        recording_lifecycle: Arc::new(RecordingLifecycleManager::disabled()),
        workflow_lifecycle: Arc::new(WorkflowLifecycleManager::disabled()),
        workflow_observability: Arc::new(WorkflowObservability::default()),
        workflow_log_retention: None,
        workflow_output_retention: None,
        idle_stop_timeout: Duration::from_secs(300),
        public_gateway_url: "https://localhost:4433".to_string(),
        default_owner_mode: SessionOwnerMode::Collaborative,
    });
    (build_api_router(state), token, agent_server)
}

async fn test_router_with_docker_pool() -> (Router, String) {
    let auth_validator = Arc::new(AuthValidator::from_hmac_secret(vec![7; 32]));
    let token = auth_validator
        .generate_token()
        .expect("hmac auth validator should generate dev token");
    let session_manager = Arc::new(
        SessionManager::new(SessionManagerConfig::DockerPool(
            crate::session_manager::SessionManagerDockerConfig {
                docker_bin: "docker".to_string(),
                image: "deploy-host".to_string(),
                network: "deploy_bpane-internal".to_string(),
                shared_run_volume: "deploy_agent-socket".to_string(),
                container_name_prefix: "bpane-runtime".to_string(),
                socket_root: "/run/bpane/sessions".to_string(),
                cdp_proxy_port: 9223,
                shm_size: "128m".to_string(),
                start_timeout: Duration::from_secs(30),
                idle_timeout: Duration::from_secs(300),
                max_active_runtimes: 2,
                max_starting_runtimes: 1,
                seccomp_unconfined: true,
                env_file: None,
            },
        ))
        .unwrap(),
    );
    let session_store = SessionStore::in_memory_with_config(session_manager.profile().clone());
    session_manager
        .attach_session_store(session_store.clone())
        .await;
    let state = Arc::new(ApiState {
        registry: Arc::new(SessionRegistry::new(10, false)),
        auth_validator,
        connect_ticket_manager: Arc::new(SessionConnectTicketManager::new(
            vec![5; 32],
            Duration::from_secs(300),
        )),
        automation_access_token_manager: Arc::new(SessionAutomationAccessTokenManager::new(
            vec![6; 32],
            Duration::from_secs(300),
        )),
        session_store,
        session_manager,
        credential_provider: Some(test_credential_provider()),
        recording_artifact_store: test_artifact_store(),
        workspace_file_store: test_workspace_file_store(),
        workflow_source_resolver: test_workflow_source_resolver(),
        recording_observability: Arc::new(RecordingObservability::default()),
        recording_lifecycle: Arc::new(RecordingLifecycleManager::disabled()),
        workflow_lifecycle: Arc::new(WorkflowLifecycleManager::disabled()),
        workflow_observability: Arc::new(WorkflowObservability::default()),
        workflow_log_retention: None,
        workflow_output_retention: None,
        idle_stop_timeout: Duration::from_secs(300),
        public_gateway_url: "https://localhost:4433".to_string(),
        default_owner_mode: SessionOwnerMode::Collaborative,
    });
    (build_api_router(state), token)
}

fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

async fn response_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn response_bytes(response: axum::response::Response) -> Vec<u8> {
    to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec()
}

fn git(args: &[&str], cwd: &std::path::Path) {
    let output = StdCommand::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_head(cwd: &std::path::Path) -> String {
    let output = StdCommand::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_ascii_lowercase()
}

#[tokio::test]
async fn workflow_runs_expose_runtime_hold_and_release_semantics() {
    let (app, token, _) = test_router_with_workflow_lifecycle(WorkflowWorkerConfig {
        docker_bin: std::path::PathBuf::from("/bin/sh"),
        image: "deploy-workflow-worker:test".to_string(),
        max_active_workers: 1,
        network: Some("deploy_bpane-internal".to_string()),
        container_name_prefix: "bpane-workflow".to_string(),
        gateway_api_url: "http://gateway:8932".to_string(),
        work_root: std::path::PathBuf::from("/tmp/bpane-workflows"),
        bearer_token: Some("token".to_string()),
        oidc_token_url: None,
        oidc_client_id: None,
        oidc_client_secret: None,
        oidc_scopes: None,
    });

    let workflow = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/workflows")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": "Runtime Hold Workflow"
                        })
                        .to_string(),
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
                            "executor": "manual_test",
                            "entrypoint": "workflows/runtime-hold/run.mjs"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;

    let live_hold_run = response_json(
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
    assert!(
        live_hold_run.get("id").is_some(),
        "unexpected workflow run create response: {live_hold_run}"
    );
    let live_hold_run_id = live_hold_run["id"].as_str().unwrap().to_string();
    let live_hold_session_id = live_hold_run["session_id"].as_str().unwrap().to_string();

    let automation_access = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/api/v1/sessions/{live_hold_session_id}/automation-access"
                    ))
                    .header("authorization", bearer(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let automation_token = automation_access["token"].as_str().unwrap().to_string();

    let running = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/workflow-runs/{live_hold_run_id}/state"))
                .header("x-bpane-automation-access-token", &automation_token)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "state": "running",
                        "message": "executor attached"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(running.status(), StatusCode::OK);

    let awaiting_input = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/workflow-runs/{live_hold_run_id}/state"))
                    .header("x-bpane-automation-access-token", &automation_token)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "state": "awaiting_input",
                            "message": "approval required",
                            "data": {
                                "intervention_request": {
                                    "kind": "approval"
                                },
                                "runtime_hold": {
                                    "mode": "live",
                                    "timeout_sec": 1
                                }
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
    assert!(
        awaiting_input["state"].is_string(),
        "unexpected awaiting_input response: {awaiting_input}"
    );
    assert_eq!(awaiting_input["state"], "awaiting_input");

    let live_runtime = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let current = response_json(
                app.clone()
                    .oneshot(
                        Request::builder()
                            .uri(format!("/api/v1/workflow-runs/{live_hold_run_id}"))
                            .header("authorization", bearer(&token))
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap(),
            )
            .await;
            if current["runtime"]["resume_mode"] == json!("live_runtime") {
                break current;
            }
            sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("live runtime hold should become visible");
    assert_eq!(
        live_runtime["runtime"]["exact_runtime_available"],
        json!(true)
    );
    assert!(live_runtime["runtime"]["hold_until"].is_string());

    let released_live_hold = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let current = response_json(
                app.clone()
                    .oneshot(
                        Request::builder()
                            .uri(format!("/api/v1/workflow-runs/{live_hold_run_id}"))
                            .header("authorization", bearer(&token))
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap(),
            )
            .await;
            if current["runtime"]["released_at"].is_string() {
                break current;
            }
            sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("live-hold workflow run should release after timeout");
    assert_eq!(
        released_live_hold["runtime"]["resume_mode"],
        json!("profile_restart")
    );
    assert_eq!(
        released_live_hold["runtime"]["release_reason"],
        json!("hold_expired")
    );

    let released_session = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/sessions/{live_hold_session_id}"))
                    .header("authorization", bearer(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(released_session["state"], "stopped");

    let immediate_release_run = response_json(
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
    let immediate_release_run_id = immediate_release_run["id"].as_str().unwrap().to_string();
    let immediate_release_session_id = immediate_release_run["session_id"]
        .as_str()
        .unwrap()
        .to_string();

    let second_automation_access = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/api/v1/sessions/{immediate_release_session_id}/automation-access"
                    ))
                    .header("authorization", bearer(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let second_automation_token = second_automation_access["token"]
        .as_str()
        .unwrap()
        .to_string();

    let second_running = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/workflow-runs/{immediate_release_run_id}/state"
                ))
                .header("x-bpane-automation-access-token", &second_automation_token)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "state": "running",
                        "message": "executor attached"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second_running.status(), StatusCode::OK);

    response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/api/v1/workflow-runs/{immediate_release_run_id}/state"
                    ))
                    .header("x-bpane-automation-access-token", &second_automation_token)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "state": "awaiting_input",
                            "message": "approval required",
                            "data": {
                                "intervention_request": {
                                    "kind": "approval"
                                }
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

    let immediately_released = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let current = response_json(
                app.clone()
                    .oneshot(
                        Request::builder()
                            .uri(format!("/api/v1/workflow-runs/{immediate_release_run_id}"))
                            .header("authorization", bearer(&token))
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap(),
            )
            .await;
            if current["runtime"]["released_at"].is_string() {
                break current;
            }
            sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("awaiting-input run without live hold should release immediately");

    assert_eq!(
        immediately_released["runtime"]["resume_mode"],
        json!("profile_restart")
    );
    assert_eq!(
        immediately_released["runtime"]["release_reason"],
        json!("awaiting_input_no_live_hold")
    );
}

#[tokio::test]
async fn automation_access_token_can_update_workflow_run_state_logs_and_outputs() {
    let (app, token) = test_router();

    let workflow = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/workflows")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": "stateful-workflow"
                        })
                        .to_string(),
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
                        "entrypoint": "workflows/stateful.ts"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_version.status(), StatusCode::CREATED);

    let session = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sessions")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let session_id = session["id"].as_str().unwrap().to_string();

    let issued = response_json(
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
    let automation_token = issued["token"].as_str().unwrap().to_string();

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
                                "existing_session_id": session_id
                            },
                            "input": {
                                "month": "2026-03"
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
    let task_id = run["automation_task_id"].as_str().unwrap().to_string();

    let running = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/workflow-runs/{run_id}/state"))
                .header("x-bpane-automation-access-token", &automation_token)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "state": "running",
                        "message": "workflow executor attached"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(running.status(), StatusCode::OK);

    let run_log = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/workflow-runs/{run_id}/logs"))
                .header("x-bpane-automation-access-token", &automation_token)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "stream": "system",
                        "message": "workflow bootstrapped"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(run_log.status(), StatusCode::OK);
    let run_log_body = response_json(run_log).await;
    assert_eq!(run_log_body["source"], "run");

    let task_log = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/automation-tasks/{task_id}/logs"))
                .header("x-bpane-automation-access-token", &automation_token)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "stream": "stdout",
                        "message": "opened report page"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(task_log.status(), StatusCode::OK);

    let succeeded = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/workflow-runs/{run_id}/state"))
                .header("x-bpane-automation-access-token", &automation_token)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "state": "succeeded",
                        "output": {
                            "csv_file_id": "file_123"
                        },
                        "artifact_refs": ["artifact://workflow-trace.zip"],
                        "message": "workflow completed"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(succeeded.status(), StatusCode::OK);
    let succeeded_body = response_json(succeeded).await;
    assert_eq!(succeeded_body["state"], "succeeded");
    assert_eq!(succeeded_body["output"]["csv_file_id"], "file_123");
    assert_eq!(
        succeeded_body["artifact_refs"][0],
        "artifact://workflow-trace.zip"
    );

    let fetched = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/workflow-runs/{run_id}"))
                .header("x-bpane-automation-access-token", &automation_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(fetched.status(), StatusCode::OK);
    let fetched_body = response_json(fetched).await;
    assert_eq!(fetched_body["state"], "succeeded");
    assert_eq!(fetched_body["output"]["csv_file_id"], "file_123");

    let events = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/workflow-runs/{run_id}/events"))
                .header("x-bpane-automation-access-token", &automation_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(events.status(), StatusCode::OK);
    let events_body = response_json(events).await;
    let event_types = events_body["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["event_type"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert!(event_types.contains(&"workflow_run.created".to_string()));
    assert!(event_types.contains(&"automation_task.created".to_string()));
    assert!(event_types.contains(&"workflow_run.running".to_string()));
    assert!(event_types.contains(&"automation_task.running".to_string()));
    assert!(event_types.contains(&"workflow_run.succeeded".to_string()));
    assert!(event_types.contains(&"automation_task.succeeded".to_string()));

    let logs = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/workflow-runs/{run_id}/logs"))
                .header("x-bpane-automation-access-token", &automation_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(logs.status(), StatusCode::OK);
    let logs_body = response_json(logs).await;
    let sources = logs_body["logs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|log| log["source"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert!(sources.contains(&"run".to_string()));
    assert!(sources.contains(&"automation_task".to_string()));
}
