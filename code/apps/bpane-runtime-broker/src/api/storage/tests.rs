use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use bpane_runtime_contract::{
    BrokerApiVersion, BrowserRuntimeLaunchRequest, IdempotencyKey, RuntimeOperation,
    RuntimeOperationRequest, RuntimeOperationResult, StorageHelperAction, StorageHelperRequest,
    RUNTIME_BROKER_PAYLOAD_BYTES_HEADER, RUNTIME_BROKER_PAYLOAD_SHA256_HEADER,
    RUNTIME_BROKER_REQUEST_ID_HEADER,
};
use sha2::{Digest, Sha256};
use tower::ServiceExt;
use uuid::Uuid;

use super::*;
use crate::{
    AuthenticationError, BrokerApiSettings, BrokerAuthenticator, BrokerState, ExecutionError,
    ExecutionErrorCode, LedgerConfig, OperationLedger, RuntimeOperationExecutor, ServicePrincipal,
    StorageExecutionOutput,
};

const BOUNDARY: &str = "bpane-storage-test-boundary";

#[derive(Default)]
struct TestAuthenticator;

#[async_trait]
impl BrokerAuthenticator for TestAuthenticator {
    async fn authenticate(&self, token: &str) -> Result<ServicePrincipal, AuthenticationError> {
        if token != "valid" {
            return Err(crate::AuthenticationErrorCode::MalformedToken.into());
        }
        Ok(ServicePrincipal {
            subject: "gateway".to_string(),
            client_id: "bpane-runtime-broker-gateway".to_string(),
            token_id: "token-1".to_string(),
            expires_at: 4_000_000_000,
        })
    }
}

struct StorageExecutor {
    executions: AtomicUsize,
    inputs: Mutex<Vec<Vec<u8>>>,
    export: Vec<u8>,
    invalid_output: bool,
}

#[async_trait]
impl RuntimeOperationExecutor for StorageExecutor {
    async fn execute(
        &self,
        _request: &RuntimeOperationRequest,
    ) -> Result<RuntimeOperationResult, ExecutionError> {
        Err(ExecutionErrorCode::AdapterUnavailable.into())
    }

    async fn execute_storage(
        &self,
        request: &RuntimeOperationRequest,
        payload: Option<&[u8]>,
    ) -> Result<StorageExecutionOutput, ExecutionError> {
        self.executions.fetch_add(1, Ordering::SeqCst);
        if let Some(payload) = payload {
            self.inputs.lock().unwrap().push(payload.to_vec());
        }
        let RuntimeOperation::RunStorageHelper(storage) = &request.operation else {
            return Err(ExecutionErrorCode::AdapterFailed.into());
        };
        let output = match storage.action {
            StorageHelperAction::ExportBrowserContext => {
                let digest = if self.invalid_output {
                    "0".repeat(64)
                } else {
                    hex::encode(Sha256::digest(&self.export))
                };
                StorageExecutionOutput {
                    result: RuntimeOperationResult::StoragePayload {
                        payload_bytes: self.export.len() as u64,
                        sha256_hex: digest,
                    },
                    payload: Some(self.export.clone()),
                }
            }
            StorageHelperAction::MeasureBrowserContext => StorageExecutionOutput {
                result: RuntimeOperationResult::StorageUsage { storage_bytes: 42 },
                payload: None,
            },
            _ => StorageExecutionOutput {
                result: RuntimeOperationResult::Accepted,
                payload: None,
            },
        };
        Ok(output)
    }
}

fn app(executor: Arc<dyn RuntimeOperationExecutor>, max_payload_bytes: usize) -> axum::Router {
    crate::build_router(BrokerState::new(
        Arc::new(TestAuthenticator),
        executor,
        Arc::new(OperationLedger::new(LedgerConfig {
            capacity: NonZeroUsize::new(16).unwrap(),
            completed_ttl: Duration::from_secs(60),
        })),
        BrokerApiSettings {
            max_request_bytes: NonZeroUsize::new(4096).unwrap(),
            max_storage_payload_bytes: NonZeroUsize::new(max_payload_bytes).unwrap(),
            max_concurrent: NonZeroUsize::new(2).unwrap(),
            operation_timeout: Duration::from_secs(1),
        },
    ))
}

fn storage_request(action: StorageHelperAction, declared: Option<u64>) -> RuntimeOperationRequest {
    let context_id = Uuid::now_v7();
    let storage = match action {
        StorageHelperAction::ImportBrowserContext => StorageHelperRequest {
            action,
            session_id: None,
            source_context_id: None,
            target_context_id: Some(context_id),
            file_target: None,
            declared_payload_bytes: declared,
        },
        StorageHelperAction::MaterializeSessionFiles => StorageHelperRequest {
            action,
            session_id: Some(context_id),
            source_context_id: None,
            target_context_id: None,
            file_target: Some(
                bpane_runtime_contract::SessionDataFileTarget::SessionBindingManifest,
            ),
            declared_payload_bytes: declared,
        },
        _ => StorageHelperRequest {
            action,
            session_id: None,
            source_context_id: Some(context_id),
            target_context_id: None,
            file_target: None,
            declared_payload_bytes: declared,
        },
    };
    RuntimeOperationRequest {
        api_version: BrokerApiVersion::V1,
        request_id: Uuid::now_v7(),
        idempotency_key: IdempotencyKey::new(format!("storage:{action:?}:{}", Uuid::now_v7()))
            .unwrap(),
        operation: RuntimeOperation::RunStorageHelper(storage),
    }
}

fn transfer(
    request: &RuntimeOperationRequest,
    payload: Option<&[u8]>,
    token: Option<&str>,
) -> Request<Body> {
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{BOUNDARY}\r\n").as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"request\"\r\n");
    body.extend_from_slice(
        format!(
            "Content-Type: {}\r\n\r\n",
            bpane_runtime_contract::RUNTIME_BROKER_V1_MEDIA_TYPE
        )
        .as_bytes(),
    );
    body.extend_from_slice(&serde_json::to_vec(request).unwrap());
    body.extend_from_slice(b"\r\n");
    if let Some(payload) = payload {
        body.extend_from_slice(format!("--{BOUNDARY}\r\n").as_bytes());
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"payload\"\r\n");
        body.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
        body.extend_from_slice(payload);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{BOUNDARY}--\r\n").as_bytes());
    let mut builder = Request::builder()
        .method("POST")
        .uri("/v1/storage-transfers")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={BOUNDARY}"),
        );
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    builder.body(Body::from(body)).unwrap()
}

async fn json(response: Response) -> serde_json::Value {
    let bytes = to_bytes(response.into_body(), 16_384).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn authenticates_and_idempotently_transfers_exact_input() {
    let executor = Arc::new(StorageExecutor {
        executions: AtomicUsize::new(0),
        inputs: Mutex::new(Vec::new()),
        export: Vec::new(),
        invalid_output: false,
    });
    let app = app(executor.clone(), 1024);
    let payload = b"profile-archive";
    let request = storage_request(
        StorageHelperAction::ImportBrowserContext,
        Some(payload.len() as u64),
    );

    let first = app
        .clone()
        .oneshot(transfer(&request, Some(payload), Some("valid")))
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::ACCEPTED);
    assert_eq!(json(first).await["result"]["state"], "accepted");

    let replay = app
        .clone()
        .oneshot(transfer(&request, Some(payload), Some("valid")))
        .await
        .unwrap();
    assert_eq!(replay.status(), StatusCode::ACCEPTED);
    assert_eq!(replay.headers()["x-bpane-idempotent-replay"], "true");
    assert_eq!(executor.executions.load(Ordering::SeqCst), 1);
    assert_eq!(executor.inputs.lock().unwrap().as_slice(), [payload]);

    let conflicting = app
        .oneshot(transfer(&request, Some(b"profile-archivf"), Some("valid")))
        .await
        .unwrap();
    assert_eq!(conflicting.status(), StatusCode::CONFLICT);
    assert_eq!(
        json(conflicting).await["error"]["code"],
        "idempotency_conflict"
    );
}

#[tokio::test]
async fn returns_correlated_verified_export_and_reexecutes_cached_read() {
    let export = b"browser-context-profile".to_vec();
    let executor = Arc::new(StorageExecutor {
        executions: AtomicUsize::new(0),
        inputs: Mutex::new(Vec::new()),
        export: export.clone(),
        invalid_output: false,
    });
    let app = app(executor.clone(), 1024);
    let request = storage_request(StorageHelperAction::ExportBrowserContext, None);

    for replayed in [false, true] {
        let response = app
            .clone()
            .oneshot(transfer(&request, None, Some("valid")))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[RUNTIME_BROKER_REQUEST_ID_HEADER],
            request.request_id.to_string()
        );
        assert_eq!(
            response.headers()[RUNTIME_BROKER_PAYLOAD_BYTES_HEADER],
            export.len().to_string()
        );
        assert_eq!(
            response.headers()[RUNTIME_BROKER_PAYLOAD_SHA256_HEADER],
            hex::encode(Sha256::digest(&export))
        );
        assert_eq!(
            response.headers().contains_key("x-bpane-idempotent-replay"),
            replayed
        );
        assert_eq!(
            to_bytes(response.into_body(), 1024).await.unwrap().as_ref(),
            export
        );
    }
    assert_eq!(executor.executions.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn rejects_missing_mismatched_unexpected_and_excessive_payloads() {
    let executor = Arc::new(StorageExecutor {
        executions: AtomicUsize::new(0),
        inputs: Mutex::new(Vec::new()),
        export: Vec::new(),
        invalid_output: false,
    });
    let app = app(executor.clone(), 4);
    let cases = [
        (
            storage_request(StorageHelperAction::ImportBrowserContext, Some(3)),
            None,
            StatusCode::UNPROCESSABLE_ENTITY,
            "payload_missing",
        ),
        (
            storage_request(StorageHelperAction::ImportBrowserContext, Some(4)),
            Some(&b"abc"[..]),
            StatusCode::UNPROCESSABLE_ENTITY,
            "payload_length_mismatch",
        ),
        (
            storage_request(StorageHelperAction::ExportBrowserContext, None),
            Some(&b"x"[..]),
            StatusCode::UNPROCESSABLE_ENTITY,
            "payload_unexpected",
        ),
        (
            storage_request(StorageHelperAction::ImportBrowserContext, Some(5)),
            Some(&b"abcde"[..]),
            StatusCode::PAYLOAD_TOO_LARGE,
            "payload_too_large",
        ),
    ];
    for (request, payload, status, code) in cases {
        let response = app
            .clone()
            .oneshot(transfer(&request, payload, Some("valid")))
            .await
            .unwrap();
        assert_eq!(response.status(), status);
        assert_eq!(json(response).await["error"]["code"], code);
    }
    assert_eq!(executor.executions.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn requires_authentication_and_rejects_invalid_adapter_output() {
    let executor = Arc::new(StorageExecutor {
        executions: AtomicUsize::new(0),
        inputs: Mutex::new(Vec::new()),
        export: b"secret-content-must-not-leak".to_vec(),
        invalid_output: true,
    });
    let app = app(executor, 1024);
    let request = storage_request(StorageHelperAction::ExportBrowserContext, None);

    let unauthenticated = app
        .clone()
        .oneshot(transfer(&request, None, None))
        .await
        .unwrap();
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

    let invalid = app
        .oneshot(transfer(&request, None, Some("valid")))
        .await
        .unwrap();
    assert_eq!(invalid.status(), StatusCode::BAD_GATEWAY);
    let body = json(invalid).await;
    assert_eq!(body["error"]["code"], "adapter_failed");
    assert!(!body.to_string().contains("secret-content"));
}

#[tokio::test]
async fn rejects_wrong_media_non_storage_and_interrupted_multipart() {
    let executor = Arc::new(StorageExecutor {
        executions: AtomicUsize::new(0),
        inputs: Mutex::new(Vec::new()),
        export: Vec::new(),
        invalid_output: false,
    });
    let app = app(executor.clone(), 1024);

    let wrong_media = Request::builder()
        .method("POST")
        .uri("/v1/storage-transfers")
        .header("authorization", "Bearer valid")
        .header("content-type", "application/octet-stream")
        .body(Body::from("not-multipart"))
        .unwrap();
    let response = app.clone().oneshot(wrong_media).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);

    let mut non_storage = storage_request(StorageHelperAction::ExportBrowserContext, None);
    non_storage.operation = RuntimeOperation::LaunchBrowser(BrowserRuntimeLaunchRequest {
        session_id: Uuid::now_v7(),
        browser_context_id: None,
        features: Default::default(),
    });
    let response = app
        .clone()
        .oneshot(transfer(&non_storage, None, Some("valid")))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(json(response).await["error"]["code"], "request_malformed");

    let truncated = Request::builder()
        .method("POST")
        .uri("/v1/storage-transfers")
        .header("authorization", "Bearer valid")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={BOUNDARY}"),
        )
        .body(Body::from(format!(
            "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"request\"\r\n\r\n{{"
        )))
        .unwrap();
    let response = app.oneshot(truncated).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(json(response).await["error"]["code"], "request_malformed");
    assert_eq!(executor.executions.load(Ordering::SeqCst), 0);
}
