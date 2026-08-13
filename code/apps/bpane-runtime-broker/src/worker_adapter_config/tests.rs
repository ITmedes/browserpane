use std::sync::Arc;

use async_trait::async_trait;
use bpane_runtime_contract::{
    BrokerApiVersion, BrowserRuntimeFeatures, BrowserRuntimeLaunchRequest, IdempotencyKey,
    RecordingWorkerCredentials, RecordingWorkerLaunchRequest, RuntimeOperation,
    RuntimeOperationRequest, RuntimeOperationResult, SecretValue,
};
use uuid::Uuid;

use super::*;
use crate::{ExecutionError, ExecutionErrorCode};

#[derive(Debug)]
struct StubExecutor;

#[async_trait]
impl RuntimeOperationExecutor for StubExecutor {
    async fn execute(
        &self,
        _request: &RuntimeOperationRequest,
    ) -> Result<RuntimeOperationResult, ExecutionError> {
        Ok(RuntimeOperationResult::Exists)
    }
}

fn document() -> serde_json::Value {
    serde_json::json!({
        "version": 1,
        "oidc": {
            "token_url": "http://keycloak:8080/realms/browserpane/protocol/openid-connect/token",
            "client_id": "bpane-worker",
            "scopes": ""
        },
        "workflow": {
            "network": "bpane-internal",
            "container_name_prefix": "bpane-workflow",
            "gateway_api_url": "http://gateway:8932",
            "work_root": "/tmp/bpane-workflows",
            "request_timeout_ms": 30000,
            "output_limit_bytes": 262144
        },
        "recording": {
            "network": "bpane-internal",
            "container_name_prefix": "bpane-recording",
            "artifact_volume": "bpane-recordings",
            "chrome_executable": "/usr/bin/chromium",
            "gateway_api_url": "http://gateway:8932",
            "page_url": "http://web:8080/recording-worker.html",
            "connect_gateway_url": "https://gateway:4433",
            "output_root": "/tmp/bpane-recordings",
            "cert_spki": "test-spki",
            "cert_spki_file": null,
            "headless": true,
            "connect_timeout_ms": 120000,
            "poll_interval_ms": 2000,
            "request_timeout_ms": 30000
        }
    })
}

fn settings(directory: &tempfile::TempDir) -> WorkerAdapterSettings {
    let config_file = directory.path().join("workers.json");
    let secret_file = directory.path().join("worker-secret");
    std::fs::write(&config_file, document().to_string()).unwrap();
    std::fs::write(&secret_file, "oidc-secret\n").unwrap();
    WorkerAdapterSettings {
        config_file: Some(config_file),
        workflow_image: Some(format!("workflow@sha256:{}", "a".repeat(64))),
        recording_image: Some(format!("recording@sha256:{}", "b".repeat(64))),
        oidc_client_secret_file: Some(secret_file),
    }
}

fn operation(operation: RuntimeOperation) -> RuntimeOperationRequest {
    RuntimeOperationRequest {
        api_version: BrokerApiVersion::V1,
        request_id: Uuid::now_v7(),
        idempotency_key: IdempotencyKey::new(format!("test:{}", Uuid::now_v7())).unwrap(),
        operation,
    }
}

#[test]
fn absent_configuration_preserves_the_browser_executor() {
    let settings = WorkerAdapterSettings {
        config_file: None,
        workflow_image: None,
        recording_image: None,
        oidc_client_secret_file: None,
    };

    settings
        .combine_executor(Arc::new(StubExecutor), None, 0)
        .unwrap();
}

#[tokio::test]
async fn complete_configuration_routes_browser_and_worker_families() {
    let directory = tempfile::tempdir().unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            listener,
            axum::Router::new().fallback(|| async {
                (
                    axum::http::StatusCode::NOT_FOUND,
                    axum::Json(serde_json::json!({ "message": "not found" })),
                )
            }),
        )
        .await
        .unwrap();
    });
    let executor = settings(&directory)
        .combine_executor(
            Arc::new(StubExecutor),
            Some(&format!("http://{address}")),
            5,
        )
        .unwrap();

    let browser = executor
        .execute(&operation(RuntimeOperation::LaunchBrowser(
            BrowserRuntimeLaunchRequest {
                session_id: Uuid::now_v7(),
                browser_context_id: None,
                features: BrowserRuntimeFeatures::default(),
            },
        )))
        .await
        .unwrap();
    assert_eq!(browser, RuntimeOperationResult::Exists);

    let recording = executor
        .execute(&operation(RuntimeOperation::LaunchRecording(
            RecordingWorkerLaunchRequest {
                session_id: Uuid::now_v7(),
                recording_id: Uuid::now_v7(),
                credentials: RecordingWorkerCredentials {
                    connect_ticket: SecretValue::new("connect").unwrap(),
                    session_automation_access_token: SecretValue::new("automation").unwrap(),
                    recording_worker_access_token: SecretValue::new("worker").unwrap(),
                    gateway_bearer_token: None,
                },
            },
        )))
        .await
        .unwrap_err();
    assert_eq!(recording.code, ExecutionErrorCode::AdapterFailed);
}

#[test]
fn partial_malformed_mutable_and_secret_configuration_fail_closed() {
    let directory = tempfile::tempdir().unwrap();
    let browser: Arc<dyn RuntimeOperationExecutor> = Arc::new(StubExecutor);
    let mut partial = settings(&directory);
    partial.recording_image = None;
    assert!(partial
        .combine_executor(Arc::clone(&browser), Some("http://docker:2375"), 5)
        .is_err());

    let mut mutable = settings(&directory);
    mutable.workflow_image = Some("workflow:latest".to_string());
    assert!(mutable
        .combine_executor(Arc::clone(&browser), Some("http://docker:2375"), 5)
        .is_err());

    let malformed = settings(&directory);
    std::fs::write(
        malformed.config_file.as_ref().unwrap(),
        r#"{"version":1,"unexpected":true}"#,
    )
    .unwrap();
    assert!(malformed
        .combine_executor(Arc::clone(&browser), Some("http://docker:2375"), 5)
        .is_err());

    let empty_secret = settings(&directory);
    std::fs::write(empty_secret.oidc_client_secret_file.as_ref().unwrap(), "\n").unwrap();
    let error = empty_secret
        .combine_executor(browser, Some("http://docker:2375"), 5)
        .err()
        .expect("empty worker secret must fail");
    assert!(!format!("{error:?}").contains("oidc-secret"));
}
