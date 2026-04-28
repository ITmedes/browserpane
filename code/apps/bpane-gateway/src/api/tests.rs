use std::collections::HashMap;
use std::fs;
use std::io::Cursor;
use std::process::Command as StdCommand;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tempfile::tempdir;
use tokio::sync::Mutex;
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
use crate::session_manager::{SessionManager, SessionManagerConfig};
use crate::workflow_lifecycle::WorkflowLifecycleManager;
use crate::workflow_observability::WorkflowObservability;
use crate::workflow_source::WorkflowSourceResolver;
use crate::workspace_file_store::WorkspaceFileStore;

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

fn test_router() -> (Router, String) {
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
    (build_api_router(state), token)
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
            loop {
                match listener.accept().await {
                    Ok((stream, _)) => connections.push(stream),
                    Err(_) => break,
                }
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
async fn rejects_v1_session_routes_without_bearer_auth() {
    let (app, _) = test_router();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/sessions")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[test]
fn blocking_session_stop_only_applies_to_legacy_runtime_backends() {
    assert!(should_block_session_stop(
        SessionLifecycleState::Ready,
        true,
        true,
    ));
    assert!(!should_block_session_stop(
        SessionLifecycleState::Ready,
        false,
        true,
    ));
    assert!(!should_block_session_stop(
        SessionLifecycleState::Stopped,
        true,
        true,
    ));
}

#[test]
fn session_status_maps_recorder_clients() {
    let latest_recording = StoredSessionRecording {
        id: uuid::Uuid::now_v7(),
        session_id: uuid::Uuid::now_v7(),
        previous_recording_id: None,
        state: StoredSessionRecordingState::Recording,
        format: SessionRecordingFormat::Webm,
        mime_type: Some("video/webm".to_string()),
        bytes: Some(4096),
        duration_ms: Some(1200),
        error: None,
        termination_reason: None,
        artifact_ref: None,
        started_at: chrono::Utc::now(),
        completed_at: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    let playback =
        prepare_session_recording_playback(latest_recording.session_id, &[], chrono::Utc::now());
    let status = session_status_from_snapshot(
        SessionTelemetrySnapshot {
            browser_clients: 3,
            viewer_clients: 1,
            recorder_clients: 1,
            max_viewers: 10,
            viewer_slots_remaining: 9,
            exclusive_browser_owner: false,
            mcp_owner: false,
            resolution: (1280, 720),
            joins_accepted: 4,
            joins_rejected_viewer_cap: 0,
            last_join_latency_ms: 12,
            average_join_latency_ms: 9.5,
            max_join_latency_ms: 15,
            full_refresh_requests: 1,
            full_refresh_tiles_requested: 30,
            last_full_refresh_tiles: 30,
            max_full_refresh_tiles: 30,
            egress_send_stream_lock_acquires_total: 10,
            egress_send_stream_lock_wait_us_total: 20,
            egress_send_stream_lock_wait_us_average: 2.0,
            egress_send_stream_lock_wait_us_max: 6,
            egress_lagged_receives_total: 0,
            egress_lagged_frames_total: 0,
        },
        &SessionRecordingPolicy {
            mode: SessionRecordingMode::Manual,
            format: SessionRecordingFormat::Webm,
            retention_sec: Some(86_400),
        },
        Some(&latest_recording),
        playback.resource,
    );

    assert_eq!(status.browser_clients, 3);
    assert_eq!(status.viewer_clients, 1);
    assert_eq!(status.recorder_clients, 1);
    assert_eq!(status.viewer_slots_remaining, 9);
    assert_eq!(
        status.recording.configured_mode,
        SessionRecordingMode::Manual
    );
    assert_eq!(status.recording.format, SessionRecordingFormat::Webm);
    assert_eq!(status.recording.retention_sec, Some(86_400));
    assert!(matches!(
        status.recording.state,
        SessionRecordingStatusState::Recording
    ));
    assert!(status.recording.recorder_attached);
    assert!(status.recording.active_recording_id.is_some());
    assert_eq!(status.recording.bytes_written, Some(4096));
    assert_eq!(status.recording.duration_ms, Some(1200));
}

#[tokio::test]
async fn creates_lists_gets_and_stops_a_session_resource() {
    let (app, token) = test_router();

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "template_id": "default",
                        "viewport": { "width": 1440, "height": 900 },
                        "idle_timeout_sec": 900,
                        "labels": { "suite": "contract" },
                        "integration_context": { "ticket": "BPANE-6" },
                        "recording": {
                          "mode": "manual",
                          "format": "webm",
                          "retention_sec": 86400
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(create_response.status(), StatusCode::CREATED);
    let created = response_json(create_response).await;
    let session_id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["state"], "ready");
    assert_eq!(created["owner_mode"], "collaborative");
    assert_eq!(created["idle_timeout_sec"], 900);
    assert_eq!(created["template_id"], "default");
    assert!(created["automation_delegate"].is_null());
    assert_eq!(created["viewport"]["width"], 1440);
    assert_eq!(created["viewport"]["height"], 900);
    assert_eq!(created["capabilities"]["browser_input"], true);
    assert_eq!(created["capabilities"]["clipboard"], true);
    assert_eq!(created["capabilities"]["audio"], true);
    assert_eq!(created["capabilities"]["microphone"], true);
    assert_eq!(created["capabilities"]["camera"], true);
    assert_eq!(created["capabilities"]["file_transfer"], true);
    assert_eq!(created["capabilities"]["resize"], true);
    assert!(created["owner"]["subject"].is_string());
    assert!(created["owner"]["issuer"].is_string());
    assert_eq!(created["labels"]["suite"], "contract");
    assert_eq!(created["integration_context"]["ticket"], "BPANE-6");
    assert_eq!(created["recording"]["mode"], "manual");
    assert_eq!(created["recording"]["format"], "webm");
    assert_eq!(created["recording"]["retention_sec"], 86400);
    assert_eq!(created["connect"]["gateway_url"], "https://localhost:4433");
    assert_eq!(created["connect"]["transport_path"], "/session");
    assert_eq!(created["connect"]["auth_type"], "session_connect_ticket");
    assert_eq!(
        created["connect"]["ticket_path"],
        format!("/api/v1/sessions/{session_id}/access-tokens")
    );
    assert_eq!(
        created["connect"]["compatibility_mode"],
        "legacy_single_runtime"
    );
    assert_eq!(created["runtime"]["binding"], "legacy_single_session");
    assert_eq!(
        created["runtime"]["compatibility_mode"],
        "legacy_single_runtime"
    );
    assert_eq!(created["runtime"]["cdp_endpoint"], "http://host:9223");
    assert!(created["created_at"].is_string());
    assert!(created["updated_at"].is_string());
    assert!(created["stopped_at"].is_null());

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    let listed = response_json(list_response).await;
    assert_eq!(listed["sessions"].as_array().unwrap().len(), 1);
    assert_eq!(listed["sessions"][0]["id"], session_id);

    let get_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/sessions/{session_id}"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_response.status(), StatusCode::OK);
    let fetched = response_json(get_response).await;
    assert_eq!(fetched["id"], session_id);
    assert_eq!(fetched["labels"]["suite"], "contract");
    assert_eq!(fetched["recording"]["mode"], "manual");

    let issue_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/access-tokens"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(issue_response.status(), StatusCode::OK);
    let issued = response_json(issue_response).await;
    assert_eq!(issued["session_id"], session_id);
    assert_eq!(issued["token_type"], "session_connect_ticket");
    assert!(issued["token"].as_str().unwrap().starts_with("v1."));
    assert!(issued["expires_at"].is_string());
    assert_eq!(issued["connect"]["gateway_url"], "https://localhost:4433");
    assert_eq!(issued["connect"]["transport_path"], "/session");
    assert_eq!(issued["connect"]["auth_type"], "session_connect_ticket");
    assert_eq!(
        issued["connect"]["ticket_path"],
        format!("/api/v1/sessions/{session_id}/access-tokens")
    );
    assert_eq!(
        issued["connect"]["compatibility_mode"],
        "legacy_single_runtime"
    );

    let delete_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/sessions/{session_id}"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_response.status(), StatusCode::OK);
    let stopped = response_json(delete_response).await;
    assert_eq!(stopped["id"], session_id);
    assert_eq!(stopped["state"], "stopped");
    assert!(stopped["stopped_at"].is_string());
}

#[tokio::test]
async fn rejects_extension_bound_sessions_on_legacy_runtime_backends() {
    let (app, token) = test_router();

    let create_extension_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/extensions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "adblock",
                        "description": "Policy-approved extension",
                        "labels": { "suite": "contract" }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_extension_response.status(), StatusCode::CREATED);
    let extension = response_json(create_extension_response).await;
    let extension_id = extension["id"].as_str().unwrap();

    let create_version_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/extensions/{extension_id}/versions"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "version": "1.0.0",
                        "install_path": "/home/bpane/bpane-test-extension"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_version_response.status(), StatusCode::CREATED);

    let create_session_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "extension_ids": [extension_id]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_session_response.status(), StatusCode::CONFLICT);
    let error = response_json(create_session_response).await;
    assert_eq!(
        error["error"],
        "the current runtime backend does not support session extensions"
    );
}

#[tokio::test]
async fn creates_extensions_and_applies_them_to_docker_sessions() {
    let (app, token) = test_router_with_docker_pool().await;

    let create_extension_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/extensions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "workflow-extension",
                        "description": "Approved workflow extension",
                        "labels": { "suite": "contract" }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_extension_response.status(), StatusCode::CREATED);
    let extension = response_json(create_extension_response).await;
    let extension_id = extension["id"].as_str().unwrap().to_string();

    let create_version_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/extensions/{extension_id}/versions"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "version": "1.0.0",
                        "install_path": "/home/bpane/bpane-test-extension"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_version_response.status(), StatusCode::CREATED);
    let version = response_json(create_version_response).await;

    let create_session_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "labels": { "suite": "contract" },
                        "extension_ids": [extension_id]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_session_response.status(), StatusCode::CREATED);
    let session = response_json(create_session_response).await;
    assert_eq!(session["extensions"].as_array().unwrap().len(), 1);
    assert_eq!(session["extensions"][0]["extension_id"], extension_id);
    assert_eq!(session["extensions"][0]["name"], "workflow-extension");
    assert_eq!(session["extensions"][0]["version"], "1.0.0");
    assert_eq!(
        session["extensions"][0]["extension_version_id"],
        version["id"].as_str().unwrap()
    );
}

#[tokio::test]
async fn workflow_runs_inherit_session_extensions() {
    let (app, token) = test_router_with_docker_pool().await;

    let create_extension_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/extensions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "workflow-extension",
                        "description": "Approved workflow extension",
                        "labels": { "suite": "contract" }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_extension_response.status(), StatusCode::CREATED);
    let extension = response_json(create_extension_response).await;
    let extension_id = extension["id"].as_str().unwrap().to_string();

    let create_version_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/extensions/{extension_id}/versions"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "version": "1.0.0",
                        "install_path": "/home/bpane/bpane-test-extension"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_version_response.status(), StatusCode::CREATED);

    let create_workflow_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workflows")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "extension-smoke",
                        "description": "Workflow extension test",
                        "labels": { "suite": "contract" }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_workflow_response.status(), StatusCode::CREATED);
    let workflow = response_json(create_workflow_response).await;
    let workflow_id = workflow["id"].as_str().unwrap().to_string();

    let create_workflow_version_response = app
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
                        "entrypoint": "workflows/extensions/run.mjs",
                        "default_session": {
                            "extension_ids": [extension_id]
                        },
                        "allowed_extension_ids": [extension_id]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_workflow_version_response.status(), StatusCode::CREATED);

    let create_run_response = app
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
                        "version": "v1"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_run_response.status(), StatusCode::CREATED);
    let run = response_json(create_run_response).await;
    assert_eq!(run["extensions"].as_array().unwrap().len(), 1);
    assert_eq!(run["extensions"][0]["extension_id"], extension_id);
    assert_eq!(run["extensions"][0]["name"], "workflow-extension");
}

#[tokio::test]
async fn creates_lists_uploads_downloads_and_deletes_file_workspace_content() {
    let (app, token) = test_router();

    let create_workspace_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/file-workspaces")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "finance-reports",
                        "description": "Shared workflow outputs",
                        "labels": {
                            "suite": "contract"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_workspace_response.status(), StatusCode::CREATED);
    let workspace = response_json(create_workspace_response).await;
    let workspace_id = workspace["id"].as_str().unwrap().to_string();
    assert_eq!(workspace["name"], "finance-reports");
    assert_eq!(workspace["description"], "Shared workflow outputs");
    assert_eq!(workspace["labels"]["suite"], "contract");
    assert_eq!(
        workspace["files_path"],
        format!("/api/v1/file-workspaces/{workspace_id}/files")
    );

    let list_workspaces_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/file-workspaces")
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_workspaces_response.status(), StatusCode::OK);
    let workspaces = response_json(list_workspaces_response).await;
    assert_eq!(workspaces["workspaces"].as_array().unwrap().len(), 1);
    assert_eq!(workspaces["workspaces"][0]["id"], workspace_id);

    let get_workspace_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/file-workspaces/{workspace_id}"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_workspace_response.status(), StatusCode::OK);
    let fetched_workspace = response_json(get_workspace_response).await;
    assert_eq!(fetched_workspace["id"], workspace_id);

    let file_bytes = b"alpha,beta\n1,2\n";
    let file_hash = hex::encode(Sha256::digest(file_bytes));
    let provenance = json!({
        "source_kind": "git_materialized",
        "repo_path": "workflows/exports/report.csv",
        "commit": "abc123def456"
    });
    let upload_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/file-workspaces/{workspace_id}/files"))
                .header("authorization", bearer(&token))
                .header("content-type", "text/csv")
                .header("x-bpane-file-name", "report.csv")
                .header("x-bpane-file-provenance", provenance.to_string())
                .body(Body::from(file_bytes.to_vec()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(upload_response.status(), StatusCode::CREATED);
    let uploaded = response_json(upload_response).await;
    let file_id = uploaded["id"].as_str().unwrap().to_string();
    assert_eq!(uploaded["workspace_id"], workspace_id);
    assert_eq!(uploaded["name"], "report.csv");
    assert_eq!(uploaded["media_type"], "text/csv");
    assert_eq!(uploaded["byte_count"], file_bytes.len());
    assert_eq!(uploaded["sha256_hex"], file_hash);
    assert_eq!(uploaded["provenance"], provenance);
    assert_eq!(
        uploaded["content_path"],
        format!("/api/v1/file-workspaces/{workspace_id}/files/{file_id}/content")
    );

    let list_files_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/file-workspaces/{workspace_id}/files"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_files_response.status(), StatusCode::OK);
    let files = response_json(list_files_response).await;
    assert_eq!(files["files"].as_array().unwrap().len(), 1);
    assert_eq!(files["files"][0]["id"], file_id);

    let get_file_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/file-workspaces/{workspace_id}/files/{file_id}"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_file_response.status(), StatusCode::OK);
    let fetched_file = response_json(get_file_response).await;
    assert_eq!(fetched_file["id"], file_id);
    assert_eq!(fetched_file["sha256_hex"], file_hash);

    let content_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/file-workspaces/{workspace_id}/files/{file_id}/content"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(content_response.status(), StatusCode::OK);
    assert_eq!(
        content_response
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "text/csv"
    );
    let downloaded = to_bytes(content_response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(downloaded.as_ref(), file_bytes);

    let delete_file_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/v1/file-workspaces/{workspace_id}/files/{file_id}"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_file_response.status(), StatusCode::OK);
    let deleted = response_json(delete_file_response).await;
    assert_eq!(deleted["id"], file_id);

    let final_list_response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/file-workspaces/{workspace_id}/files"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(final_list_response.status(), StatusCode::OK);
    let final_files = response_json(final_list_response).await;
    assert!(final_files["files"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn rejects_always_mode_when_recording_worker_is_not_configured() {
    let (app, token) = test_router();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "recording": {
                          "mode": "always",
                          "format": "webm"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);
    let payload = response_json(response).await;
    assert_eq!(
        payload["error"],
        "recording mode=always requires a configured recording worker"
    );
}

#[tokio::test]
async fn issues_session_automation_access_descriptor() {
    let (app, token) = test_router();

    let created = response_json(
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
    let session_id = created["id"].as_str().unwrap().to_string();

    let issue_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/automation-access"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(issue_response.status(), StatusCode::OK);
    let issued = response_json(issue_response).await;
    assert_eq!(issued["session_id"], session_id);
    assert_eq!(issued["token_type"], "session_automation_access_token");
    assert!(issued["token"].as_str().unwrap().starts_with("v1."));
    assert!(issued["expires_at"].is_string());
    assert_eq!(issued["automation"]["endpoint_url"], "http://host:9223");
    assert_eq!(issued["automation"]["protocol"], "chrome_devtools_protocol");
    assert_eq!(
        issued["automation"]["auth_type"],
        "session_automation_access_token"
    );
    assert_eq!(
        issued["automation"]["auth_header"],
        "x-bpane-automation-access-token"
    );
    assert_eq!(
        issued["automation"]["status_path"],
        format!("/api/v1/sessions/{session_id}/status")
    );
    assert_eq!(
        issued["automation"]["mcp_owner_path"],
        format!("/api/v1/sessions/{session_id}/mcp-owner")
    );
    assert_eq!(
        issued["automation"]["compatibility_mode"],
        "legacy_single_runtime"
    );
}

#[tokio::test]
async fn automation_access_token_can_drive_status_and_mcp_owner_routes() {
    let (app, token, _agent_server) = test_router_with_live_agent().await;

    let created = response_json(
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
    let session_id = created["id"].as_str().unwrap().to_string();

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
    let automation_token = issued["token"].as_str().unwrap();

    let status_before = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/sessions/{session_id}/status"))
                .header("x-bpane-automation-access-token", automation_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(status_before.status(), StatusCode::OK);
    let status_before_body = response_json(status_before).await;
    assert_eq!(status_before_body["mcp_owner"], false);

    let claim_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/mcp-owner"))
                .header("x-bpane-automation-access-token", automation_token)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "width": 1280, "height": 720 }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(claim_response.status(), StatusCode::OK);

    let status_after_claim = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/sessions/{session_id}/status"))
                .header("x-bpane-automation-access-token", automation_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(status_after_claim.status(), StatusCode::OK);
    let status_after_claim_body = response_json(status_after_claim).await;
    assert_eq!(status_after_claim_body["mcp_owner"], true);

    let clear_response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/sessions/{session_id}/mcp-owner"))
                .header("x-bpane-automation-access-token", automation_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(clear_response.status(), StatusCode::OK);
}

#[tokio::test]
async fn creates_lists_gets_and_stops_session_recording_metadata() {
    let (app, token) = test_router();

    let create_session_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "recording": {
                          "mode": "manual",
                          "format": "webm"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_session_response.status(), StatusCode::CREATED);
    let session = response_json(create_session_response).await;
    let session_id = session["id"].as_str().unwrap().to_string();

    let create_recording_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/recordings"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_recording_response.status(), StatusCode::CREATED);
    let created_recording = response_json(create_recording_response).await;
    let recording_id = created_recording["id"].as_str().unwrap().to_string();
    assert_eq!(created_recording["session_id"], session_id);
    assert_eq!(created_recording["state"], "recording");
    assert_eq!(created_recording["format"], "webm");
    assert_eq!(created_recording["mime_type"], "video/webm");
    assert!(created_recording["previous_recording_id"].is_null());
    assert!(created_recording["termination_reason"].is_null());
    assert_eq!(
        created_recording["content_path"],
        format!("/api/v1/sessions/{session_id}/recordings/{recording_id}/content")
    );
    assert_eq!(created_recording["artifact_available"], false);

    let list_recordings_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/sessions/{session_id}/recordings"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_recordings_response.status(), StatusCode::OK);
    let recordings = response_json(list_recordings_response).await;
    assert_eq!(recordings["recordings"].as_array().unwrap().len(), 1);
    assert_eq!(recordings["recordings"][0]["id"], recording_id);

    let get_recording_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/sessions/{session_id}/recordings/{recording_id}"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_recording_response.status(), StatusCode::OK);
    let fetched_recording = response_json(get_recording_response).await;
    assert_eq!(fetched_recording["id"], recording_id);
    assert_eq!(fetched_recording["state"], "recording");

    let stop_recording_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/sessions/{session_id}/recordings/{recording_id}/stop"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stop_recording_response.status(), StatusCode::OK);
    let stopped_recording = response_json(stop_recording_response).await;
    assert_eq!(stopped_recording["state"], "finalizing");
    assert_eq!(stopped_recording["termination_reason"], "manual_stop");

    let temp_dir = tempfile::tempdir().unwrap();
    let artifact_path = temp_dir.path().join("recording.webm");
    std::fs::write(&artifact_path, b"webm-bytes").unwrap();

    let complete_recording_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/sessions/{session_id}/recordings/{recording_id}/complete"
                ))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                      "source_path": artifact_path.to_string_lossy(),
                      "mime_type": "video/webm",
                      "bytes": 10,
                      "duration_ms": 2500
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(complete_recording_response.status(), StatusCode::OK);
    let completed_recording = response_json(complete_recording_response).await;
    assert_eq!(completed_recording["state"], "ready");
    assert_eq!(completed_recording["artifact_available"], true);
    assert_eq!(completed_recording["bytes"], 10);
    assert_eq!(completed_recording["duration_ms"], 2500);

    let content_response = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/sessions/{session_id}/recordings/{recording_id}/content"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(content_response.status(), StatusCode::OK);
    let content_bytes = to_bytes(content_response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(content_bytes.as_ref(), b"webm-bytes");
}

#[tokio::test]
async fn recording_failure_updates_metadata_state() {
    let (app, token) = test_router();

    let create_session_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "recording": {
                          "mode": "manual",
                          "format": "webm"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let session = response_json(create_session_response).await;
    let session_id = session["id"].as_str().unwrap().to_string();

    let create_recording_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/recordings"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let recording = response_json(create_recording_response).await;
    let recording_id = recording["id"].as_str().unwrap().to_string();

    let fail_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/sessions/{session_id}/recordings/{recording_id}/fail"
                ))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                      "error": "recorder worker crashed",
                      "termination_reason": "worker_exit"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(fail_response.status(), StatusCode::OK);
    let failed = response_json(fail_response).await;
    assert_eq!(failed["state"], "failed");
    assert_eq!(failed["error"], "recorder worker crashed");
    assert_eq!(failed["termination_reason"], "worker_exit");
}

#[tokio::test]
async fn playback_manifest_and_export_bundle_follow_ready_segments() {
    let auth_validator = Arc::new(AuthValidator::from_hmac_secret(vec![7; 32]));
    let token = auth_validator.generate_token().unwrap();
    let session_store = SessionStore::in_memory();
    let artifact_store = test_artifact_store();
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
        session_store: session_store.clone(),
        session_manager: Arc::new(
            SessionManager::new(SessionManagerConfig::StaticSingle {
                agent_socket_path: "/tmp/test.sock".to_string(),
                cdp_endpoint: Some("http://host:9223".to_string()),
                idle_timeout: Duration::from_secs(300),
            })
            .unwrap(),
        ),
        credential_provider: Some(test_credential_provider()),
        recording_artifact_store: artifact_store.clone(),
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
    let app = build_api_router(state);

    let create_session_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "recording": {
                          "mode": "manual",
                          "format": "webm"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let session = response_json(create_session_response).await;
    let session_id = session["id"].as_str().unwrap().to_string();

    let create_first_recording = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/recordings"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let first_recording = response_json(create_first_recording).await;
    let first_recording_id = first_recording["id"].as_str().unwrap().to_string();

    let stop_first_recording = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/sessions/{session_id}/recordings/{first_recording_id}/stop"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stop_first_recording.status(), StatusCode::OK);

    let temp_dir = tempfile::tempdir().unwrap();
    let artifact_path = temp_dir.path().join("segment-1.webm");
    std::fs::write(&artifact_path, b"segment-one").unwrap();
    let complete_first_recording = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/sessions/{session_id}/recordings/{first_recording_id}/complete"
                ))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "source_path": artifact_path.to_string_lossy(),
                        "mime_type": "video/webm",
                        "bytes": 11,
                        "duration_ms": 900
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(complete_first_recording.status(), StatusCode::OK);

    let create_second_recording = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/recordings"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let second_recording = response_json(create_second_recording).await;
    let second_recording_id = second_recording["id"].as_str().unwrap().to_string();

    let fail_second_recording = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/sessions/{session_id}/recordings/{second_recording_id}/fail"
                ))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "error": "recorder worker crashed",
                        "termination_reason": "worker_exit"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(fail_second_recording.status(), StatusCode::OK);

    let playback_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/sessions/{session_id}/recording-playback"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(playback_response.status(), StatusCode::OK);
    let playback = response_json(playback_response).await;
    assert_eq!(playback["state"], "partial");
    assert_eq!(playback["segment_count"], 2);
    assert_eq!(playback["included_segment_count"], 1);
    assert_eq!(playback["failed_segment_count"], 1);
    assert_eq!(playback["active_segment_count"], 0);
    assert_eq!(playback["missing_artifact_segment_count"], 0);

    let manifest_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/sessions/{session_id}/recording-playback/manifest"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(manifest_response.status(), StatusCode::OK);
    let manifest = response_json(manifest_response).await;
    assert_eq!(
        manifest["format_version"],
        "browserpane_recording_playback_v1"
    );
    assert_eq!(manifest["segments"].as_array().unwrap().len(), 1);
    assert_eq!(manifest["omitted_segments"].as_array().unwrap().len(), 1);
    assert_eq!(manifest["omitted_segments"][0]["omitted_reason"], "failed");

    let export_response = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/sessions/{session_id}/recording-playback/export"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(export_response.status(), StatusCode::OK);
    assert_eq!(
        export_response
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "application/zip"
    );
    let export_bytes = to_bytes(export_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let cursor = std::io::Cursor::new(export_bytes.to_vec());
    let mut archive = zip::ZipArchive::new(cursor).unwrap();
    let mut manifest_file = archive.by_name("manifest.json").unwrap();
    let mut manifest_bytes = Vec::new();
    use std::io::Read;
    manifest_file.read_to_end(&mut manifest_bytes).unwrap();
    drop(manifest_file);
    let manifest_json: Value = serde_json::from_slice(&manifest_bytes).unwrap();
    assert_eq!(manifest_json["segment_count"], 2);
    assert!(archive.by_name("player.html").is_ok());
    let segment_name = manifest_json["segments"][0]["file_name"].as_str().unwrap();
    let mut segment_file = archive.by_name(segment_name).unwrap();
    let mut segment_bytes = Vec::new();
    segment_file.read_to_end(&mut segment_bytes).unwrap();
    assert_eq!(segment_bytes.as_slice(), b"segment-one");
}

#[tokio::test]
async fn recording_operations_snapshot_tracks_finalize_playback_and_failures() {
    let auth_validator = Arc::new(AuthValidator::from_hmac_secret(vec![7; 32]));
    let token = auth_validator.generate_token().unwrap();
    let observability = Arc::new(RecordingObservability::default());
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
        recording_observability: observability.clone(),
        recording_lifecycle: Arc::new(RecordingLifecycleManager::disabled()),
        workflow_lifecycle: Arc::new(WorkflowLifecycleManager::disabled()),
        workflow_observability: Arc::new(WorkflowObservability::default()),
        workflow_log_retention: None,
        workflow_output_retention: None,
        idle_stop_timeout: Duration::from_secs(300),
        public_gateway_url: "https://localhost:4433".to_string(),
        default_owner_mode: SessionOwnerMode::Collaborative,
    });
    let app = build_api_router(state);

    let session = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sessions")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "recording": {
                              "mode": "manual",
                              "format": "webm"
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
    let session_id = session["id"].as_str().unwrap().to_string();

    let recording = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/sessions/{session_id}/recordings"))
                    .header("authorization", bearer(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let recording_id = recording["id"].as_str().unwrap().to_string();

    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/sessions/{session_id}/recordings/{recording_id}/stop"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let temp_dir = tempfile::tempdir().unwrap();
    let artifact_path = temp_dir.path().join("segment.webm");
    std::fs::write(&artifact_path, b"segment").unwrap();
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/sessions/{session_id}/recordings/{recording_id}/complete"
                ))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "source_path": artifact_path.to_string_lossy(),
                        "mime_type": "video/webm",
                        "bytes": 7,
                        "duration_ms": 700
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let failed_recording = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/sessions/{session_id}/recordings"))
                    .header("authorization", bearer(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let failed_recording_id = failed_recording["id"].as_str().unwrap().to_string();
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/sessions/{session_id}/recordings/{failed_recording_id}/fail"
                ))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "error": "worker exited",
                        "termination_reason": "worker_exit"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/sessions/{session_id}/recording-playback/manifest"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/sessions/{session_id}/recording-playback/export"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let operations_response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/recording/operations")
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(operations_response.status(), StatusCode::OK);
    let operations = response_json(operations_response).await;
    assert_eq!(operations["artifact_finalize_requests_total"], 1);
    assert_eq!(operations["artifact_finalize_successes_total"], 1);
    assert_eq!(operations["artifact_finalize_failures_total"], 0);
    assert_eq!(operations["recording_failures_total"], 1);
    assert_eq!(operations["playback_manifest_requests_total"], 1);
    assert_eq!(operations["playback_export_requests_total"], 1);
    assert_eq!(operations["playback_export_successes_total"], 1);
    assert_eq!(operations["playback_export_failures_total"], 0);
    assert!(operations["playback_export_bytes_total"].as_u64().unwrap() > 0);
}

#[tokio::test]
async fn expired_recording_artifacts_return_gone() {
    let auth_validator = Arc::new(AuthValidator::from_hmac_secret(vec![7; 32]));
    let token = auth_validator.generate_token().unwrap();
    let session_store = SessionStore::in_memory();
    let artifact_store = test_artifact_store();
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
        session_store: session_store.clone(),
        session_manager: Arc::new(
            SessionManager::new(SessionManagerConfig::StaticSingle {
                agent_socket_path: "/tmp/test.sock".to_string(),
                cdp_endpoint: Some("http://host:9223".to_string()),
                idle_timeout: Duration::from_secs(300),
            })
            .unwrap(),
        ),
        credential_provider: Some(test_credential_provider()),
        recording_artifact_store: artifact_store.clone(),
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
    let app = build_api_router(state);

    let create_session_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "recording": {
                          "mode": "manual",
                          "format": "webm",
                          "retention_sec": 60
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let session = response_json(create_session_response).await;
    let session_id = session["id"].as_str().unwrap().to_string();

    let create_recording_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/recordings"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let recording = response_json(create_recording_response).await;
    let recording_id = recording["id"].as_str().unwrap().to_string();

    let temp_dir = tempfile::tempdir().unwrap();
    let artifact_path = temp_dir.path().join("recording.webm");
    std::fs::write(&artifact_path, b"webm-bytes").unwrap();

    let complete_recording_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/sessions/{session_id}/recordings/{recording_id}/complete"
                ))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                      "source_path": artifact_path.to_string_lossy(),
                      "mime_type": "video/webm",
                      "bytes": 10,
                      "duration_ms": 2500
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(complete_recording_response.status(), StatusCode::OK);

    let recording_uuid = uuid::Uuid::parse_str(&recording_id).unwrap();
    let session_uuid = uuid::Uuid::parse_str(&session_id).unwrap();
    let stored = session_store
        .get_recording_for_session(session_uuid, recording_uuid)
        .await
        .unwrap()
        .unwrap();
    let retention = RecordingRetentionManager::new(
        session_store.clone(),
        artifact_store,
        Arc::new(RecordingObservability::default()),
        Duration::from_secs(60),
    );
    retention
        .run_cleanup_pass(stored.completed_at.unwrap() + chrono::Duration::seconds(61))
        .await
        .unwrap();

    let content_response = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/sessions/{session_id}/recordings/{recording_id}/content"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(content_response.status(), StatusCode::GONE);
    let body = response_json(content_response).await;
    assert!(body["error"]
        .as_str()
        .unwrap()
        .contains("no longer available"));
}

#[tokio::test]
async fn stopped_session_can_issue_a_new_connect_ticket_and_resume() {
    let (app, token) = test_router();

    let created = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sessions")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({ "idle_timeout_sec": 300 }).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let session_id = created["id"].as_str().unwrap().to_string();

    let delete_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/sessions/{session_id}"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_response.status(), StatusCode::OK);

    let issue_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/access-tokens"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(issue_response.status(), StatusCode::OK);
    let issued = response_json(issue_response).await;
    assert_eq!(issued["session_id"], session_id);
    assert_eq!(issued["token_type"], "session_connect_ticket");

    let get_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/sessions/{session_id}"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_response.status(), StatusCode::OK);
    let fetched = response_json(get_response).await;
    assert_eq!(fetched["state"], "ready");
    assert!(fetched["stopped_at"].is_null());
}

#[tokio::test]
async fn rejects_second_active_session_on_legacy_runtime() {
    let (app, token) = test_router();
    let request_body = json!({
        "viewport": { "width": 1280, "height": 720 }
    })
    .to_string();

    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(request_body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::CREATED);

    let second = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(request_body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(second.status(), StatusCode::CONFLICT);
    let body = response_json(second).await;
    assert!(body["error"]
        .as_str()
        .unwrap()
        .contains("only supports 1 active runtime-backed session"));
}

#[tokio::test]
async fn scopes_session_resources_to_the_authenticated_owner() {
    let auth_validator = Arc::new(AuthValidator::from_hmac_secret(vec![9; 32]));
    let alpha_token = auth_validator.generate_token().unwrap();
    tokio::time::sleep(Duration::from_secs(1)).await;
    let bravo_token = auth_validator.generate_token().unwrap();
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
    let app = build_api_router(state);

    let created = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sessions")
                    .header("authorization", bearer(&alpha_token))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let session_id = created["id"].as_str().unwrap().to_string();

    let lookup = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/sessions/{session_id}"))
                .header("authorization", bearer(&bravo_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(lookup.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn rejects_session_scoped_runtime_routes_for_unknown_or_foreign_sessions_before_runtime_work()
{
    let auth_validator = Arc::new(AuthValidator::from_hmac_secret(vec![11; 32]));
    let alpha_token = auth_validator.generate_token().unwrap();
    tokio::time::sleep(Duration::from_secs(1)).await;
    let bravo_token = auth_validator.generate_token().unwrap();
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
    let app = build_api_router(state);

    let created = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sessions")
                    .header("authorization", bearer(&alpha_token))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let session_id = created["id"].as_str().unwrap().to_string();

    let foreign_status = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/sessions/{session_id}/status"))
                .header("authorization", bearer(&bravo_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(foreign_status.status(), StatusCode::NOT_FOUND);

    let unknown_owner = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/mcp-owner"))
                .header("authorization", bearer(&bravo_token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "width": 1280, "height": 720 }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unknown_owner.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn rejects_session_scoped_runtime_routes_for_stopped_sessions() {
    let (app, token) = test_router();

    let created = response_json(
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
    let session_id = created["id"].as_str().unwrap().to_string();

    let delete_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/sessions/{session_id}"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_response.status(), StatusCode::OK);

    let status_response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/sessions/{session_id}/status"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(status_response.status(), StatusCode::CONFLICT);
    let body = response_json(status_response).await;
    assert!(body["error"]
        .as_str()
        .unwrap()
        .contains("runtime-compatible state"));
}

#[tokio::test]
async fn owner_can_set_and_clear_session_automation_delegate() {
    let (app, token) = test_router();

    let created = response_json(
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
    let session_id = created["id"].as_str().unwrap().to_string();

    let delegated = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/automation-owner"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "client_id": "bpane-mcp-bridge",
                        "issuer": "https://issuer.example",
                        "display_name": "BrowserPane MCP bridge"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delegated.status(), StatusCode::OK);
    let delegated_body = response_json(delegated).await;
    assert_eq!(
        delegated_body["automation_delegate"]["client_id"],
        "bpane-mcp-bridge"
    );

    let cleared = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/sessions/{session_id}/automation-owner"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cleared.status(), StatusCode::OK);
    let cleared_body = response_json(cleared).await;
    assert!(cleared_body["automation_delegate"].is_null());
}

#[tokio::test]
async fn creates_lists_gets_and_cancels_automation_tasks_for_existing_sessions() {
    let (app, token) = test_router();

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

    let create_task = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/automation-tasks")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "display_name": "Smoke task",
                        "executor": "playwright",
                        "session": {
                            "existing_session_id": session_id
                        },
                        "input": {
                            "step": "open_dashboard"
                        },
                        "labels": {
                            "suite": "contract"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_task.status(), StatusCode::CREATED);
    let task = response_json(create_task).await;
    let task_id = task["id"].as_str().unwrap().to_string();
    assert_eq!(task["display_name"], "Smoke task");
    assert_eq!(task["executor"], "playwright");
    assert_eq!(task["state"], "pending");
    assert_eq!(task["session"]["source"], "existing_session");
    assert_eq!(task["session"]["session_id"], session_id);
    assert_eq!(task["labels"]["suite"], "contract");
    assert_eq!(task["input"]["step"], "open_dashboard");

    let list_tasks = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/automation-tasks")
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_tasks.status(), StatusCode::OK);
    let listed = response_json(list_tasks).await;
    assert_eq!(listed["tasks"].as_array().unwrap().len(), 1);
    assert_eq!(listed["tasks"][0]["id"], task_id);

    let get_task = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/automation-tasks/{task_id}"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_task.status(), StatusCode::OK);

    let initial_events = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/automation-tasks/{task_id}/events"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(initial_events.status(), StatusCode::OK);
    let initial_events_body = response_json(initial_events).await;
    assert_eq!(initial_events_body["events"].as_array().unwrap().len(), 1);
    assert_eq!(
        initial_events_body["events"][0]["event_type"],
        "automation_task.created"
    );

    let cancel_task = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/automation-tasks/{task_id}/cancel"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cancel_task.status(), StatusCode::OK);
    let cancelled = response_json(cancel_task).await;
    assert_eq!(cancelled["state"], "cancelled");
    assert!(cancelled["cancel_requested_at"].is_string());
    assert!(cancelled["completed_at"].is_string());

    let logs = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/automation-tasks/{task_id}/logs"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(logs.status(), StatusCode::OK);
    let logs_body = response_json(logs).await;
    assert_eq!(logs_body["logs"].as_array().unwrap().len(), 1);
    assert_eq!(logs_body["logs"][0]["stream"], "system");

    let events = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/automation-tasks/{task_id}/events"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(events.status(), StatusCode::OK);
    let events_body = response_json(events).await;
    assert_eq!(events_body["events"].as_array().unwrap().len(), 2);
    assert_eq!(
        events_body["events"][1]["event_type"],
        "automation_task.cancelled"
    );
}

#[tokio::test]
async fn automation_tasks_can_create_their_own_session_binding() {
    let (app, token) = test_router();

    let create_task = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/automation-tasks")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "display_name": "Bootstrap task",
                        "executor": "playwright",
                        "session": {
                            "create_session": {
                                "labels": {
                                    "origin": "automation-task"
                                }
                            }
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_task.status(), StatusCode::CREATED);
    let task = response_json(create_task).await;
    let session_id = task["session"]["session_id"].as_str().unwrap().to_string();
    assert_eq!(task["session"]["source"], "created_session");
    assert_eq!(task["state"], "pending");

    let get_session = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/sessions/{session_id}"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_session.status(), StatusCode::OK);
    let session = response_json(get_session).await;
    assert_eq!(session["labels"]["origin"], "automation-task");
}

#[tokio::test]
async fn creates_workflow_definitions_versions_and_runs_with_default_sessions() {
    let (app, token) = test_router();

    let create_workflow = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workflows")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "stripe-monthly-export",
                        "description": "Export monthly payout reports",
                        "labels": {
                            "team": "finance"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_workflow.status(), StatusCode::CREATED);
    let workflow = response_json(create_workflow).await;
    let workflow_id = workflow["id"].as_str().unwrap().to_string();
    assert_eq!(workflow["latest_version"], Value::Null);

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
                        "entrypoint": "workflows/stripe/export-payouts.ts",
                        "input_schema": {
                            "type": "object",
                            "required": ["month"]
                        },
                        "output_schema": {
                            "type": "object",
                            "required": ["csv_file_id"]
                        },
                        "default_session": {
                            "labels": {
                                "origin": "workflow-run"
                            }
                        },
                        "allowed_credential_binding_ids": ["cred_stripe_prod"],
                        "allowed_file_workspace_ids": ["ws_finance_reports"]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_version.status(), StatusCode::CREATED);
    let version = response_json(create_version).await;
    assert_eq!(version["version"], "v1");
    assert_eq!(version["executor"], "playwright");

    let get_workflow = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/workflows/{workflow_id}"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_workflow.status(), StatusCode::OK);
    let workflow_body = response_json(get_workflow).await;
    assert_eq!(workflow_body["latest_version"], "v1");

    let get_version = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/workflows/{workflow_id}/versions/v1"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_version.status(), StatusCode::OK);

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
                        "input": {
                            "month": "2026-03"
                        },
                        "labels": {
                            "suite": "contract"
                        }
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
    let task_id = run["automation_task_id"].as_str().unwrap().to_string();
    let session_id = run["session_id"].as_str().unwrap().to_string();
    assert_eq!(run["state"], "pending");
    assert_eq!(run["workflow_version"], "v1");
    assert_eq!(run["labels"]["suite"], "contract");
    assert_eq!(run["input"]["month"], "2026-03");

    let get_run = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/workflow-runs/{run_id}"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_run.status(), StatusCode::OK);

    let run_events = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/workflow-runs/{run_id}/events"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(run_events.status(), StatusCode::OK);
    let events_body = response_json(run_events).await;
    assert_eq!(events_body["events"].as_array().unwrap().len(), 2);
    let event_types = events_body["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["event_type"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert!(event_types.contains(&"workflow_run.created".to_string()));
    assert!(event_types.contains(&"automation_task.created".to_string()));

    let run_logs = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/workflow-runs/{run_id}/logs"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(run_logs.status(), StatusCode::OK);
    let logs_body = response_json(run_logs).await;
    assert_eq!(logs_body["logs"].as_array().unwrap().len(), 0);

    let get_session = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/sessions/{session_id}"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_session.status(), StatusCode::OK);
    let session = response_json(get_session).await;
    assert_eq!(session["labels"]["origin"], "workflow-run");

    let get_task = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/automation-tasks/{task_id}"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_task.status(), StatusCode::OK);
}

#[tokio::test]
async fn workflow_runs_expose_source_snapshot_content_to_owner_and_automation_access() {
    let (app, token) = test_router();
    let source_repo = tempdir().unwrap();
    git(&["init", "--initial-branch=main"], source_repo.path());
    git(
        &["config", "user.email", "workflow@test.local"],
        source_repo.path(),
    );
    git(
        &["config", "user.name", "Workflow Test"],
        source_repo.path(),
    );
    fs::create_dir_all(source_repo.path().join("workflows")).unwrap();
    fs::write(source_repo.path().join("README.md"), "root\n").unwrap();
    fs::write(
        source_repo.path().join("workflows/demo.ts"),
        "export default async function demo() {}\n",
    )
    .unwrap();
    fs::write(source_repo.path().join("workflows/helper.txt"), "helper\n").unwrap();
    git(&["add", "."], source_repo.path());
    git(&["commit", "-m", "init"], source_repo.path());
    let resolved_commit = git_head(source_repo.path());

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
                            "name": "snapshot-workflow"
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
                        "entrypoint": "workflows/demo.ts",
                        "source": {
                            "kind": "git",
                            "repository_url": source_repo.path().to_string_lossy(),
                            "resolved_commit": resolved_commit.clone(),
                            "root_path": "workflows"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_version.status(), StatusCode::CREATED);

    let create_run = response_json(
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
    let run_id = create_run["id"].as_str().unwrap().to_string();
    let session_id = create_run["session_id"].as_str().unwrap().to_string();
    let source_snapshot = create_run["source_snapshot"].clone();
    assert_eq!(source_snapshot["entrypoint"], "workflows/demo.ts");
    assert_eq!(
        source_snapshot["source"]["resolved_commit"],
        resolved_commit
    );
    let content_path = source_snapshot["content_path"]
        .as_str()
        .unwrap()
        .to_string();

    let owner_download = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&content_path)
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(owner_download.status(), StatusCode::OK);
    let owner_bytes = response_bytes(owner_download).await;
    let mut owner_zip = ZipArchive::new(Cursor::new(owner_bytes.clone())).unwrap();
    let owner_names = (0..owner_zip.len())
        .map(|index| owner_zip.by_index(index).unwrap().name().to_string())
        .collect::<Vec<_>>();
    assert!(owner_names.contains(&"workflows/demo.ts".to_string()));
    assert!(owner_names.contains(&"workflows/helper.txt".to_string()));
    assert!(!owner_names.contains(&"README.md".to_string()));

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
    let automation_download = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/workflow-runs/{run_id}/source-snapshot/content"
                ))
                .header("x-bpane-automation-access-token", &automation_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(automation_download.status(), StatusCode::OK);
    let automation_bytes = response_bytes(automation_download).await;
    assert_eq!(automation_bytes, owner_bytes);
}

#[tokio::test]
async fn workflow_run_create_supports_external_correlation_and_safe_idempotent_retry() {
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
                            "name": "idempotent-workflow"
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
                        "entrypoint": "workflows/idempotent/run.mjs"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_version.status(), StatusCode::CREATED);

    let request_body = json!({
        "workflow_id": workflow_id,
        "version": "v1",
        "session": {
            "create_session": {}
        },
        "source_system": "camunda-prod",
        "source_reference": "process-instance-123/task-7",
        "client_request_id": "job-123-attempt-1",
        "input": {
            "customer_id": "cust-42"
        },
        "labels": {
            "suite": "contract"
        }
    });

    let first_create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workflow-runs")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(request_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first_create.status(), StatusCode::CREATED);
    let first_run = response_json(first_create).await;
    let run_id = first_run["id"].as_str().unwrap().to_string();
    let session_id = first_run["session_id"].as_str().unwrap().to_string();
    let task_id = first_run["automation_task_id"].as_str().unwrap().to_string();
    assert_eq!(first_run["source_system"], "camunda-prod");
    assert_eq!(
        first_run["source_reference"],
        "process-instance-123/task-7"
    );
    assert_eq!(first_run["client_request_id"], "job-123-attempt-1");

    let second_create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workflow-runs")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(request_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let second_status = second_create.status();
    let second_run = response_json(second_create).await;
    assert_eq!(second_status, StatusCode::OK, "{second_run:#}");
    assert_eq!(second_run["id"], run_id);
    assert_eq!(second_run["session_id"], session_id);
    assert_eq!(second_run["automation_task_id"], task_id);
    assert_eq!(second_run["source_system"], "camunda-prod");
    assert_eq!(
        second_run["source_reference"],
        "process-instance-123/task-7"
    );
    assert_eq!(second_run["client_request_id"], "job-123-attempt-1");

    let run_events = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/workflow-runs/{run_id}/events"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(run_events.status(), StatusCode::OK);
    let events = response_json(run_events).await;
    let created_count = events["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|event| event["event_type"] == "workflow_run.created")
        .count();
    assert_eq!(created_count, 1);
}

#[tokio::test]
async fn workflow_run_create_rejects_conflicting_idempotent_retry() {
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
                            "name": "idempotent-conflict"
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
                        "entrypoint": "workflows/idempotent/run.mjs"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_version.status(), StatusCode::CREATED);

    let first_request = json!({
        "workflow_id": workflow_id,
        "version": "v1",
        "session": {
            "create_session": {}
        },
        "source_system": "camunda-prod",
        "source_reference": "task-1",
        "client_request_id": "job-999-attempt-1",
        "input": {
            "customer_id": "cust-42"
        }
    });
    let first_create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workflow-runs")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(first_request.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first_create.status(), StatusCode::CREATED);

    let conflicting_request = json!({
        "workflow_id": workflow_id,
        "version": "v1",
        "session": {
            "create_session": {}
        },
        "source_system": "camunda-prod",
        "source_reference": "task-2",
        "client_request_id": "job-999-attempt-1",
        "input": {
            "customer_id": "cust-77"
        }
    });
    let conflicting_create = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workflow-runs")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(conflicting_request.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let conflicting_status = conflicting_create.status();
    let body = response_json(conflicting_create).await;
    assert_eq!(conflicting_status, StatusCode::CONFLICT, "{body:#}");
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("client_request_id")
    );
}

#[tokio::test]
#[tokio::test]
async fn workflow_runs_expose_workspace_input_content_to_owner_and_automation_access() {
    let (app, token) = test_router();

    let workspace = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/file-workspaces")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": "workflow-inputs"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let workspace_id = workspace["id"].as_str().unwrap().to_string();

    let file_bytes = b"month,total\n2026-03,42\n".to_vec();
    let upload_file = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/file-workspaces/{workspace_id}/files"))
                .header("authorization", bearer(&token))
                .header("content-type", "text/csv")
                .header("x-bpane-file-name", "monthly-report.csv")
                .body(Body::from(file_bytes.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(upload_file.status(), StatusCode::CREATED);
    let file = response_json(upload_file).await;
    let file_id = file["id"].as_str().unwrap().to_string();

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
                            "name": "workspace-input-workflow"
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
                        "entrypoint": "workflows/demo.ts",
                        "allowed_file_workspace_ids": [workspace_id]
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
                        "workspace_inputs": [
                            {
                                "workspace_id": workspace_id,
                                "file_id": file_id,
                                "mount_path": "inputs/monthly-report.csv"
                            }
                        ]
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
    let session_id = run["session_id"].as_str().unwrap().to_string();
    let workspace_inputs = run["workspace_inputs"].as_array().unwrap();
    assert_eq!(workspace_inputs.len(), 1);
    let workspace_input = &workspace_inputs[0];
    assert_eq!(workspace_input["workspace_id"], workspace_id);
    assert_eq!(workspace_input["file_id"], file_id);
    assert_eq!(workspace_input["mount_path"], "inputs/monthly-report.csv");

    let content_path = workspace_input["content_path"].as_str().unwrap().to_string();
    let owner_download = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&content_path)
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(owner_download.status(), StatusCode::OK);
    let owner_bytes = response_bytes(owner_download).await;
    assert_eq!(owner_bytes, file_bytes);

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
    let automation_download = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/workflow-runs/{run_id}/workspace-inputs/{}/content",
                    workspace_input["id"].as_str().unwrap()
                ))
                .header("x-bpane-automation-access-token", &automation_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(automation_download.status(), StatusCode::OK);
    let automation_bytes = response_bytes(automation_download).await;
    assert_eq!(automation_bytes, owner_bytes);
}

#[tokio::test]
async fn creates_lists_and_gets_credential_bindings() {
    let (app, token) = test_router();

    let created_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/credential-bindings")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "demo-login",
                        "provider": "vault_kv_v2",
                        "namespace": "smoke",
                        "allowed_origins": ["http://web:8080"],
                        "injection_mode": "form_fill",
                        "secret_payload": {
                            "username": "demo",
                            "password": "demo-demo"
                        },
                        "labels": {
                            "suite": "credential"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(created_response.status(), StatusCode::CREATED);
    let created = response_json(created_response).await;
    let binding_id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["name"], "demo-login");
    assert_eq!(created["provider"], "vault_kv_v2");
    assert_eq!(created["namespace"], "smoke");
    assert_eq!(created["allowed_origins"], json!(["http://web:8080"]));
    assert_eq!(created["injection_mode"], "form_fill");
    assert!(created["external_ref"].as_str().unwrap().starts_with("test/"));
    assert!(created.get("secret_payload").is_none());

    let listed = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/credential-bindings")
                    .header("authorization", bearer(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let bindings = listed["credential_bindings"].as_array().unwrap();
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0]["id"], binding_id);

    let fetched = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/credential-bindings/{binding_id}"))
                    .header("authorization", bearer(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(fetched["id"], binding_id);
    assert!(fetched.get("secret_payload").is_none());
}

#[tokio::test]
async fn workflow_runs_resolve_credential_bindings_via_automation_access() {
    let (app, token) = test_router();

    let created_binding = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/credential-bindings")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": "demo-login",
                            "provider": "vault_kv_v2",
                            "namespace": "smoke",
                            "allowed_origins": ["http://web:8080"],
                            "injection_mode": "form_fill",
                            "secret_payload": {
                                "username": "demo",
                                "password": "demo-demo"
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
    let binding_id = created_binding["id"].as_str().unwrap().to_string();

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
                            "name": "credential-workflow"
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
                        "entrypoint": "workflows/demo.ts",
                        "allowed_credential_binding_ids": [binding_id]
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
                        "credential_binding_ids": [binding_id]
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
    let session_id = run["session_id"].as_str().unwrap().to_string();
    let credential_bindings = run["credential_bindings"].as_array().unwrap();
    assert_eq!(credential_bindings.len(), 1);
    assert_eq!(credential_bindings[0]["id"], binding_id);

    let owner_resolve = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/workflow-runs/{run_id}/credential-bindings/{binding_id}/resolved"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(owner_resolve.status(), StatusCode::UNAUTHORIZED);

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

    let automation_resolve = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/api/v1/workflow-runs/{run_id}/credential-bindings/{binding_id}/resolved"
                    ))
                    .header("x-bpane-automation-access-token", &automation_token)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(automation_resolve["binding"]["id"], binding_id);
    assert_eq!(
        automation_resolve["payload"],
        json!({
            "username": "demo",
            "password": "demo-demo"
        })
    );

    let events = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/workflow-runs/{run_id}/events"))
                    .header("authorization", bearer(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert!(events["events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|event| event["event_type"] == "workflow_run.credential_binding_resolved"));
}

#[tokio::test]
async fn automation_access_token_can_update_automation_task_state_and_logs() {
    let (app, token) = test_router();

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

    let task = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/automation-tasks")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "display_name": "Executor task",
                            "executor": "playwright",
                            "session": {
                                "existing_session_id": session_id
                            },
                            "input": {
                                "step": "bootstrap"
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
    let task_id = task["id"].as_str().unwrap().to_string();

    let running = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/automation-tasks/{task_id}/state"))
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

    let log_append = app
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
                        "message": "opened dashboard"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(log_append.status(), StatusCode::OK);

    let succeeded = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/automation-tasks/{task_id}/state"))
                .header("x-bpane-automation-access-token", &automation_token)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "state": "succeeded",
                        "output": {
                            "result": "ok"
                        },
                        "artifact_refs": ["artifact://trace.zip"],
                        "message": "executor finished"
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
    assert_eq!(succeeded_body["output"]["result"], "ok");
    assert_eq!(succeeded_body["artifact_refs"][0], "artifact://trace.zip");

    let fetched = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/automation-tasks/{task_id}"))
                .header("x-bpane-automation-access-token", &automation_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(fetched.status(), StatusCode::OK);
    let fetched_body = response_json(fetched).await;
    assert_eq!(fetched_body["state"], "succeeded");
    assert_eq!(fetched_body["output"]["result"], "ok");

    let events = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/automation-tasks/{task_id}/events"))
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
    assert!(event_types.contains(&"automation_task.created".to_string()));
    assert!(event_types.contains(&"automation_task.running".to_string()));
    assert!(event_types.contains(&"automation_task.succeeded".to_string()));

    let logs = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/automation-tasks/{task_id}/logs"))
                .header("x-bpane-automation-access-token", &automation_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(logs.status(), StatusCode::OK);
    let logs_body = response_json(logs).await;
    assert_eq!(logs_body["logs"].as_array().unwrap().len(), 1);
    assert_eq!(logs_body["logs"][0]["stream"], "stdout");
    assert_eq!(logs_body["logs"][0]["message"], "opened dashboard");
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

#[tokio::test]
async fn workflow_definition_versions_can_pin_git_source_metadata() {
    let (app, token) = test_router();
    let temp = tempfile::tempdir().unwrap();

    let init = std::process::Command::new("git")
        .args(["init", "--initial-branch=main"])
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );
    for args in [
        vec!["config", "user.email", "workflow@test.local"],
        vec!["config", "user.name", "Workflow Test"],
    ] {
        let output = std::process::Command::new("git")
            .args(&args)
            .current_dir(temp.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    std::fs::create_dir_all(temp.path().join("workflows")).unwrap();
    std::fs::write(
        temp.path().join("workflows").join("report.ts"),
        "export default async function run() {}\n",
    )
    .unwrap();
    for args in [vec!["add", "."], vec!["commit", "-m", "init"]] {
        let output = std::process::Command::new("git")
            .args(&args)
            .current_dir(temp.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let head = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(head.status.success());
    let expected_commit = String::from_utf8_lossy(&head.stdout)
        .trim()
        .to_ascii_lowercase();

    let create_workflow = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workflows")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "git-backed-workflow",
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_workflow.status(), StatusCode::CREATED);
    let workflow = response_json(create_workflow).await;
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
                        "entrypoint": "workflows/report.ts",
                        "source": {
                            "kind": "git",
                            "repository_url": temp.path().to_string_lossy(),
                            "ref": "HEAD",
                            "root_path": "workflows"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_version.status(), StatusCode::CREATED);
    let version = response_json(create_version).await;
    assert_eq!(version["source"]["kind"], "git");
    assert_eq!(
        version["source"]["repository_url"],
        temp.path().to_string_lossy().to_string()
    );
    assert_eq!(version["source"]["ref"], "HEAD");
    assert_eq!(version["source"]["resolved_commit"], expected_commit);
    assert_eq!(version["source"]["root_path"], "workflows");

    let get_version = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/workflows/{workflow_id}/versions/v1"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_version.status(), StatusCode::OK);
    let fetched = response_json(get_version).await;
    assert_eq!(fetched["source"]["resolved_commit"], expected_commit);
}

#[tokio::test]
async fn workflow_runs_can_be_cancelled_and_surface_task_logs() {
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
                            "name": "demo-workflow"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let workflow_id = workflow["id"].as_str().unwrap();

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
                        "entrypoint": "workflows/demo.ts",
                        "default_session": {}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_version.status(), StatusCode::CREATED);

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
                            "version": "v1"
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

    let cancel_run = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/workflow-runs/{run_id}/cancel"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cancel_run.status(), StatusCode::OK);
    let cancelled = response_json(cancel_run).await;
    assert_eq!(cancelled["state"], "cancelled");

    let events = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/workflow-runs/{run_id}/events"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(events.status(), StatusCode::OK);
    let events_body = response_json(events).await;
    assert_eq!(events_body["events"].as_array().unwrap().len(), 5);
    let event_types = events_body["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["event_type"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert!(event_types.contains(&"workflow_run.created".to_string()));
    assert!(event_types.contains(&"automation_task.created".to_string()));
    assert!(event_types.contains(&"workflow_run.cancel_requested".to_string()));
    assert!(event_types.contains(&"workflow_run.cancelled".to_string()));
    assert!(event_types.contains(&"automation_task.cancelled".to_string()));

    let logs = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/workflow-runs/{run_id}/logs"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(logs.status(), StatusCode::OK);
    let logs_body = response_json(logs).await;
    let logs = logs_body["logs"].as_array().unwrap();
    assert_eq!(logs.len(), 2);
    assert!(logs.iter().all(|log| log["stream"] == "system"));
    assert!(logs
        .iter()
        .any(|log| log["message"] == "workflow run cancelled"));
}
