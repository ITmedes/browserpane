use std::sync::Arc;
use std::time::Duration;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use tower::ServiceExt;

use super::*;
use crate::auth::AuthValidator;
use crate::automation_access_token::SessionAutomationAccessTokenManager;
use crate::connect_ticket::SessionConnectTicketManager;
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
        recording_artifact_store: test_artifact_store(),
        recording_observability: Arc::new(RecordingObservability::default()),
        recording_lifecycle: Arc::new(RecordingLifecycleManager::disabled()),
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
        recording_artifact_store: test_artifact_store(),
        recording_observability: Arc::new(RecordingObservability::default()),
        recording_lifecycle: Arc::new(RecordingLifecycleManager::disabled()),
        idle_stop_timeout: Duration::from_secs(300),
        public_gateway_url: "https://localhost:4433".to_string(),
        default_owner_mode: SessionOwnerMode::Collaborative,
    });
    (build_api_router(state), token, agent_server)
}

fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

async fn response_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
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
        recording_artifact_store: artifact_store.clone(),
        recording_observability: Arc::new(RecordingObservability::default()),
        recording_lifecycle: Arc::new(RecordingLifecycleManager::disabled()),
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
        recording_artifact_store: test_artifact_store(),
        recording_observability: observability.clone(),
        recording_lifecycle: Arc::new(RecordingLifecycleManager::disabled()),
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
        recording_artifact_store: artifact_store.clone(),
        recording_observability: Arc::new(RecordingObservability::default()),
        recording_lifecycle: Arc::new(RecordingLifecycleManager::disabled()),
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
        recording_artifact_store: test_artifact_store(),
        recording_observability: Arc::new(RecordingObservability::default()),
        recording_lifecycle: Arc::new(RecordingLifecycleManager::disabled()),
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
        recording_artifact_store: test_artifact_store(),
        recording_observability: Arc::new(RecordingObservability::default()),
        recording_lifecycle: Arc::new(RecordingLifecycleManager::disabled()),
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
