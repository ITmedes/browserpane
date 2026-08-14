use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use bpane_runtime_client::{RuntimeBrokerClient, RuntimeBrokerClientError};
use bpane_runtime_contract::{
    BrokerApiVersion, RuntimeOperation, RuntimeOperationRequest, RuntimeOperationResponse,
    RuntimeOperationResult, WorkerExecutionState,
};
use tempfile::tempdir;
use tokio::time::sleep;

use super::*;
use crate::auth::{AuthValidator, AuthenticatedPrincipal};
use crate::session_access::{
    RecordingWorkerAccessTokenManager, SessionAutomationAccessTokenManager,
    SessionConnectTicketManager,
};
use crate::session_control::{
    CreateSessionRequest, PersistCompletedSessionRecordingRequest, SessionOwnerMode,
    SessionRecordingFormat, SessionRecordingPolicy,
};
use crate::worker_runtime_control::WorkerRuntimeControl;

#[derive(Default)]
struct RecordingBrokerClient {
    requests: Mutex<Vec<RuntimeOperationRequest>>,
}

#[async_trait]
impl RuntimeBrokerClient for RecordingBrokerClient {
    async fn check_readiness(&self) -> Result<(), RuntimeBrokerClientError> {
        Ok(())
    }

    async fn execute(
        &self,
        request: &RuntimeOperationRequest,
    ) -> Result<RuntimeOperationResponse, RuntimeBrokerClientError> {
        self.requests.lock().unwrap().push(request.clone());
        let result = match &request.operation {
            RuntimeOperation::LaunchRecording(_) => RuntimeOperationResult::Accepted,
            RuntimeOperation::ContainerLifecycle(request)
                if request.action == bpane_runtime_contract::ContainerLifecycleAction::Inspect =>
            {
                RuntimeOperationResult::WorkerState {
                    execution_state: WorkerExecutionState::Exited,
                    exit_code: Some(8),
                }
            }
            RuntimeOperation::ContainerLifecycle(_) => RuntimeOperationResult::Completed {
                exit_code: None,
                omitted_output_bytes: 0,
            },
            _ => RuntimeOperationResult::Absent,
        };
        Ok(RuntimeOperationResponse {
            api_version: BrokerApiVersion::V1,
            request_id: request.request_id,
            result,
        })
    }
}

#[tokio::test]
async fn broker_mode_launches_monitors_and_removes_recording_worker() {
    let store = SessionStore::in_memory();
    let auth = Arc::new(AuthValidator::from_hmac_secret(vec![9; 32]));
    let client = Arc::new(RecordingBrokerClient::default());
    let broker: Arc<dyn RuntimeBrokerClient> = client.clone();
    let mut config = test_config(
        PathBuf::from("/worker-must-not-run"),
        PathBuf::from("/capture-must-not-exist"),
    );
    config.bearer_token = Some("recording-lifecycle-secret".to_string());
    let manager = RecordingLifecycleManager::new_with_worker_control(
        Some(config),
        auth,
        Arc::new(SessionConnectTicketManager::new(
            vec![5; 32],
            Duration::from_secs(300),
        )),
        Arc::new(SessionAutomationAccessTokenManager::new(
            vec![6; 32],
            Duration::from_secs(300),
        )),
        Arc::new(RecordingWorkerAccessTokenManager::new([7; 32])),
        store.clone(),
        WorkerRuntimeControl::from_broker(Some(broker)),
    )
    .unwrap();
    let session = create_session_with_mode(&store, SessionRecordingMode::Always).await;

    manager.ensure_auto_recording(&session).await.unwrap();

    let recording = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let current = store
                .get_latest_recording_for_session(session.id)
                .await
                .unwrap()
                .unwrap();
            if current.state.is_terminal() {
                break current;
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("broker recorder should reach its terminal state");

    assert!(matches!(
        recording.state,
        crate::session_control::SessionRecordingState::Failed
    ));
    assert!(recording.error.unwrap().contains("status Some(8)"));
    assert!(store
        .get_recording_worker_assignment(session.id)
        .await
        .unwrap()
        .is_none());
    let requests = client.requests.lock().unwrap();
    assert!(matches!(
        &requests[0].operation,
        RuntimeOperation::LaunchRecording(_)
    ));
    assert!(matches!(
        &requests[1].operation,
        RuntimeOperation::ContainerLifecycle(_)
    ));
    assert!(matches!(
        &requests[2].operation,
        RuntimeOperation::ContainerLifecycle(_)
    ));
    assert!(!format!("{requests:?}").contains("recording-lifecycle-secret"));
}

fn test_principal() -> AuthenticatedPrincipal {
    AuthenticatedPrincipal {
        subject: "owner".to_string(),
        issuer: "issuer".to_string(),
        display_name: Some("Owner".to_string()),
        client_id: None,
        safe_claims: Default::default(),
    }
}

fn test_config(script: PathBuf, capture_file: PathBuf) -> RecordingWorkerConfig {
    RecordingWorkerConfig {
        bin: script,
        args: vec![capture_file.to_string_lossy().to_string()],
        chrome_executable: PathBuf::from("/tmp/google-chrome"),
        gateway_api_url: "http://127.0.0.1:8932".to_string(),
        page_url: "http://127.0.0.1:8080".to_string(),
        output_root: PathBuf::from("/tmp/bpane-recordings"),
        cert_spki: Some("spki".to_string()),
        headless: true,
        connect_timeout: Duration::from_secs(1),
        poll_interval: Duration::from_millis(10),
        finalize_timeout: Duration::from_millis(100),
        request_timeout: Duration::from_secs(1),
        output_limit_bytes: 4096,
        bearer_token: Some("token".to_string()),
        oidc_token_url: None,
        oidc_client_id: None,
        oidc_client_secret: None,
        oidc_scopes: None,
    }
}

#[test]
fn worker_config_rejects_unbounded_runtime_settings() {
    let auth = AuthValidator::from_hmac_secret(vec![9; 32]);
    let mut config = test_config(
        PathBuf::from("/bin/sh"),
        PathBuf::from("/tmp/recording-capture"),
    );
    config.output_limit_bytes = 0;
    assert!(matches!(
        validate_config(&config, auth.is_oidc(), false),
        Err(RecordingLifecycleError::InvalidConfiguration(message))
            if message.contains("output limit")
    ));

    config.output_limit_bytes = 4096;
    config.request_timeout = Duration::ZERO;
    assert!(matches!(
        validate_config(&config, auth.is_oidc(), false),
        Err(RecordingLifecycleError::InvalidConfiguration(message))
            if message.contains("request timeout")
    ));
}

#[test]
fn broker_managed_worker_does_not_require_direct_oidc_credentials() {
    let mut config = test_config(
        PathBuf::from("/bin/false"),
        PathBuf::from("/tmp/unused-recording-output"),
    );
    config.bearer_token = None;

    assert!(validate_config(&config, true, true).is_ok());
    assert!(matches!(
        validate_config(&config, true, false),
        Err(RecordingLifecycleError::InvalidConfiguration(message))
            if message.contains("auth is not configured")
    ));
}

async fn create_session_with_mode(
    store: &SessionStore,
    mode: SessionRecordingMode,
) -> StoredSession {
    store
        .create_session(
            &test_principal(),
            CreateSessionRequest {
                project_id: None,
                template_id: None,
                browser_context: None,
                network_identity: None,
                owner_mode: None,
                viewport: None,
                capabilities: Default::default(),
                idle_timeout_sec: None,
                labels: HashMap::new(),
                integration_context: None,
                extension_ids: Vec::new(),
                extensions: Vec::new(),
                recording: SessionRecordingPolicy {
                    mode,
                    format: SessionRecordingFormat::Webm,
                    retention_sec: None,
                },
            },
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap()
}

fn create_capture_script(dir: &tempfile::TempDir) -> PathBuf {
    let script_path = dir.path().join("capture-env.sh");
    fs::write(
        &script_path,
        r#"#!/bin/sh
printf '%s %s %s %s %s\n' "${BPANE_RECORDING_SESSION_ID}" "${BPANE_RECORDING_ID}" "${BPANE_RECORDING_CONNECT_TICKET}" "${BPANE_SESSION_AUTOMATION_ACCESS_TOKEN}" "${BPANE_RECORDING_WORKER_ACCESS_TOKEN}" > "$1.tmp"
mv "$1.tmp" "$1"
"#,
    )
    .unwrap();
    let mut permissions = fs::metadata(&script_path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script_path, permissions).unwrap();
    script_path
}

fn create_noisy_failure_script(dir: &tempfile::TempDir) -> PathBuf {
    let script_path = dir.path().join("noisy-recorder.sh");
    fs::write(
        &script_path,
        r#"#!/bin/sh
i=0
while [ "$i" -lt 256 ]; do
  printf 'x' >&2
  i=$((i + 1))
done
printf '\nfinal recorder failure\n' >&2
exit 7
"#,
    )
    .unwrap();
    let mut permissions = fs::metadata(&script_path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script_path, permissions).unwrap();
    script_path
}

fn test_manager(
    config: RecordingWorkerConfig,
    auth: Arc<AuthValidator>,
    store: SessionStore,
) -> RecordingLifecycleManager {
    RecordingLifecycleManager::new(
        Some(config),
        auth,
        Arc::new(SessionConnectTicketManager::new(
            vec![5; 32],
            Duration::from_secs(300),
        )),
        Arc::new(SessionAutomationAccessTokenManager::new(
            vec![6; 32],
            Duration::from_secs(300),
        )),
        Arc::new(RecordingWorkerAccessTokenManager::new([7; 32])),
        store,
    )
    .unwrap()
}

#[tokio::test]
async fn always_mode_launches_worker_and_marks_unfinished_recording_failed() {
    let temp_dir = tempdir().unwrap();
    let capture_file = temp_dir.path().join("capture.txt");
    let script = create_capture_script(&temp_dir);
    let store = SessionStore::in_memory();
    let auth = Arc::new(AuthValidator::from_hmac_secret(vec![9; 32]));
    let manager = test_manager(
        test_config(script, capture_file.clone()),
        auth,
        store.clone(),
    );
    let session = create_session_with_mode(&store, SessionRecordingMode::Always).await;

    manager.ensure_auto_recording(&session).await.unwrap();

    for _ in 0..200 {
        if capture_file.exists() {
            break;
        }
        sleep(Duration::from_millis(10)).await;
    }
    assert!(capture_file.exists());

    let capture = fs::read_to_string(&capture_file).unwrap();
    assert!(capture.contains(&session.id.to_string()));
    let captured_env: Vec<&str> = capture.split_whitespace().collect();
    assert_eq!(captured_env.len(), 5);
    assert!(!captured_env[2].is_empty());
    assert!(!captured_env[3].is_empty());
    assert!(!captured_env[4].is_empty());

    let mut latest = None;
    for _ in 0..50 {
        latest = store
            .get_latest_recording_for_session(session.id)
            .await
            .unwrap();
        if latest
            .as_ref()
            .is_some_and(|recording| recording.state.is_terminal())
        {
            break;
        }
        sleep(Duration::from_millis(10)).await;
    }

    let recording = latest.expect("recording should exist");
    assert!(matches!(
        recording.state,
        crate::session_control::SessionRecordingState::Failed
    ));
}

#[tokio::test]
async fn bounds_noisy_recorder_output_and_preserves_failure_tail() {
    let temp_dir = tempdir().unwrap();
    let script = create_noisy_failure_script(&temp_dir);
    let store = SessionStore::in_memory();
    let auth = Arc::new(AuthValidator::from_hmac_secret(vec![9; 32]));
    let manager = test_manager(
        RecordingWorkerConfig {
            output_limit_bytes: 32,
            ..test_config(script, temp_dir.path().join("unused"))
        },
        auth,
        store.clone(),
    );
    let session = create_session_with_mode(&store, SessionRecordingMode::Always).await;

    manager.ensure_auto_recording(&session).await.unwrap();

    let recording = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let current = store
                .get_latest_recording_for_session(session.id)
                .await
                .unwrap()
                .expect("recording should exist");
            if current.state.is_terminal() {
                break current;
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("noisy recorder worker should terminate");

    let error = recording
        .error
        .expect("failed recording should expose an error");
    assert!(error.contains("final recorder failure"));
    assert!(error.contains("omitted"));
    assert!(error.contains("earlier output bytes"));
}

#[tokio::test]
async fn request_stop_and_wait_observes_recording_completion() {
    let store = SessionStore::in_memory();
    let auth = Arc::new(AuthValidator::from_hmac_secret(vec![9; 32]));
    let manager = test_manager(
        RecordingWorkerConfig {
            bin: PathBuf::from("/bin/sh"),
            args: vec!["-c".to_string(), "exit 0".to_string()],
            chrome_executable: PathBuf::from("/tmp/google-chrome"),
            gateway_api_url: "http://127.0.0.1:8932".to_string(),
            page_url: "http://127.0.0.1:8080".to_string(),
            output_root: PathBuf::from("/tmp/bpane-recordings"),
            cert_spki: None,
            headless: true,
            connect_timeout: Duration::from_secs(1),
            poll_interval: Duration::from_millis(10),
            finalize_timeout: Duration::from_secs(1),
            request_timeout: Duration::from_secs(1),
            output_limit_bytes: 4096,
            bearer_token: Some("token".to_string()),
            oidc_token_url: None,
            oidc_client_id: None,
            oidc_client_secret: None,
            oidc_scopes: None,
        },
        auth,
        store.clone(),
    );
    let session = create_session_with_mode(&store, SessionRecordingMode::Manual).await;
    let recording = store
        .create_recording_for_session(session.id, SessionRecordingFormat::Webm, None)
        .await
        .unwrap();

    let completion_store = store.clone();
    let session_id = session.id;
    let recording_id = recording.id;
    tokio::spawn(async move {
        sleep(Duration::from_millis(20)).await;
        let _ = completion_store
            .complete_recording_for_session(
                session_id,
                recording_id,
                PersistCompletedSessionRecordingRequest {
                    artifact_ref: "local_fs:session/recording.webm".to_string(),
                    mime_type: Some("video/webm".to_string()),
                    bytes: Some(42),
                    duration_ms: Some(1000),
                },
            )
            .await;
    });

    manager
        .request_stop_and_wait(session.id, SessionRecordingTerminationReason::SessionStop)
        .await
        .unwrap();

    let completed = store
        .get_recording_for_session(session.id, recording.id)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        completed.state,
        crate::session_control::SessionRecordingState::Ready
    ));
    assert_eq!(
        completed.termination_reason,
        Some(SessionRecordingTerminationReason::SessionStop)
    );
}

#[tokio::test]
async fn reconcile_fails_stale_recording_and_starts_a_fresh_one() {
    let temp_dir = tempdir().unwrap();
    let capture_file = temp_dir.path().join("capture.txt");
    let script = create_capture_script(&temp_dir);
    let store = SessionStore::in_memory();
    let auth = Arc::new(AuthValidator::from_hmac_secret(vec![9; 32]));
    let manager = test_manager(
        test_config(script, capture_file.clone()),
        auth,
        store.clone(),
    );
    let session = create_session_with_mode(&store, SessionRecordingMode::Always).await;
    let stale_recording = store
        .create_recording_for_session(session.id, SessionRecordingFormat::Webm, None)
        .await
        .unwrap();
    store
        .upsert_recording_worker_assignment(PersistedSessionRecordingWorkerAssignment {
            session_id: session.id,
            recording_id: stale_recording.id,
            status: SessionRecordingWorkerAssignmentStatus::Running,
            process_id: Some(7777),
        })
        .await
        .unwrap();

    manager.reconcile_persisted_state().await.unwrap();

    for _ in 0..200 {
        if capture_file.exists() {
            break;
        }
        sleep(Duration::from_millis(10)).await;
    }
    assert!(capture_file.exists());

    let stale = store
        .get_recording_for_session(session.id, stale_recording.id)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        stale.state,
        crate::session_control::SessionRecordingState::Failed
    ));
    assert_eq!(
        stale.error.as_deref(),
        Some("gateway restarted while recorder worker was active")
    );
    assert_eq!(
        stale.termination_reason,
        Some(SessionRecordingTerminationReason::GatewayRestart)
    );

    let listed = store.list_recordings_for_session(session.id).await.unwrap();
    assert_eq!(listed.len(), 2);
    assert_ne!(listed[0].id, stale_recording.id);
    assert_eq!(listed[0].previous_recording_id, Some(stale_recording.id));
}
