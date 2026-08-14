use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use bpane_runtime_contract::{
    BrokerApiVersion, BrowserNetworkIdentity, BrowserRuntimeFeatures, BrowserRuntimeLaunchRequest,
    IdempotencyKey, RuntimeOperation, RuntimeOperationRequest, RuntimeOperationResult,
    StorageHelperAction, StorageHelperRequest,
};
use serde_json::{json, Value};
use tokio::sync::Notify;
use tower::ServiceExt;
use uuid::Uuid;

use super::*;
use crate::{
    AuthenticationError, BrokerAuthenticator, ExecutionError, LedgerConfig, OperationLedger,
    RuntimeOperationExecutor,
};

#[derive(Default)]
struct TestAuthenticator;

#[async_trait]
impl BrokerAuthenticator for TestAuthenticator {
    async fn authenticate(&self, token: &str) -> Result<ServicePrincipal, AuthenticationError> {
        let error = match token {
            "expired" => Some(AuthenticationErrorCode::Expired),
            "wrong-audience" => Some(AuthenticationErrorCode::InvalidAudience),
            "wrong-issuer" => Some(AuthenticationErrorCode::InvalidIssuer),
            "wrong-client" => Some(AuthenticationErrorCode::ClientNotAllowed),
            "key-outage" => Some(AuthenticationErrorCode::KeyFetchFailed),
            "valid" => None,
            _ => Some(AuthenticationErrorCode::MalformedToken),
        };
        if let Some(code) = error {
            return Err(code.into());
        }
        Ok(ServicePrincipal {
            subject: "service-account-gateway".to_string(),
            client_id: "bpane-runtime-broker-gateway".to_string(),
            token_id: "token-1".to_string(),
            expires_at: 4_000_000_000,
        })
    }
}

#[derive(Default)]
struct AcceptingExecutor {
    executions: AtomicUsize,
}

#[async_trait]
impl RuntimeOperationExecutor for AcceptingExecutor {
    async fn execute(
        &self,
        _request: &RuntimeOperationRequest,
    ) -> Result<RuntimeOperationResult, ExecutionError> {
        self.executions.fetch_add(1, Ordering::SeqCst);
        Ok(RuntimeOperationResult::Accepted)
    }
}

struct BlockingExecutor {
    entered: Notify,
    release: Notify,
}

#[derive(Default)]
struct UnreadyExecutor;

#[async_trait]
impl RuntimeOperationExecutor for UnreadyExecutor {
    async fn check_readiness(&self) -> Result<(), ExecutionError> {
        Err(ExecutionErrorCode::AdapterFailed.into())
    }

    async fn execute(
        &self,
        _request: &RuntimeOperationRequest,
    ) -> Result<RuntimeOperationResult, ExecutionError> {
        Ok(RuntimeOperationResult::Accepted)
    }
}

#[async_trait]
impl RuntimeOperationExecutor for BlockingExecutor {
    async fn execute(
        &self,
        _request: &RuntimeOperationRequest,
    ) -> Result<RuntimeOperationResult, ExecutionError> {
        self.entered.notify_waiters();
        self.release.notified().await;
        Ok(RuntimeOperationResult::Accepted)
    }
}

fn operation_request() -> RuntimeOperationRequest {
    RuntimeOperationRequest {
        api_version: BrokerApiVersion::V1,
        request_id: Uuid::now_v7(),
        idempotency_key: IdempotencyKey::new(format!("browser:launch:{}", Uuid::now_v7())).unwrap(),
        operation: RuntimeOperation::LaunchBrowser(BrowserRuntimeLaunchRequest {
            session_id: Uuid::now_v7(),
            browser_context_id: None,
            features: Default::default(),
        }),
    }
}

fn settings(max_concurrent: usize, timeout_duration: Duration) -> BrokerApiSettings {
    BrokerApiSettings {
        max_request_bytes: NonZeroUsize::new(1_024).unwrap(),
        max_storage_payload_bytes: NonZeroUsize::new(1024 * 1024).unwrap(),
        max_concurrent: NonZeroUsize::new(max_concurrent).unwrap(),
        operation_timeout: timeout_duration,
    }
}

fn app_with_executor(
    executor: Arc<dyn RuntimeOperationExecutor>,
    api_settings: BrokerApiSettings,
) -> Router {
    build_router(BrokerState::new(
        Arc::new(TestAuthenticator),
        executor,
        Arc::new(OperationLedger::new(LedgerConfig {
            capacity: NonZeroUsize::new(32).unwrap(),
            completed_ttl: Duration::from_secs(60),
        })),
        api_settings,
    ))
}

fn post(request: &RuntimeOperationRequest, token: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder()
        .method("POST")
        .uri("/v1/operations")
        .header(
            "content-type",
            bpane_runtime_contract::RUNTIME_BROKER_V1_MEDIA_TYPE,
        );
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    builder
        .body(Body::from(serde_json::to_vec(request).unwrap()))
        .unwrap()
}

async fn response_json(response: Response) -> Value {
    let bytes = to_bytes(response.into_body(), 16_384).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn health_routes_are_public_but_operations_require_authentication() {
    let app = app_with_executor(
        Arc::new(AcceptingExecutor::default()),
        settings(1, Duration::from_secs(1)),
    );
    for path in ["/livez", "/readyz"] {
        let response = app
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
    let response = app.oneshot(post(&operation_request(), None)).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        response_json(response).await["error"]["code"],
        "authentication_required"
    );
}

#[tokio::test]
async fn malformed_trace_context_does_not_reject_an_operation() {
    let request = operation_request();
    let mut http_request = post(&request, Some("valid"));
    http_request.headers_mut().insert(
        "traceparent",
        "malformed-sensitive-trace-marker".parse().unwrap(),
    );

    let response = app_with_executor(
        Arc::new(AcceptingExecutor::default()),
        settings(1, Duration::from_secs(1)),
    )
    .oneshot(http_request)
    .await
    .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert!(!response.headers().contains_key("traceparent"));
}

#[tokio::test]
async fn readiness_reflects_the_selected_adapter_without_requiring_authentication() {
    let app = app_with_executor(
        Arc::new(UnreadyExecutor),
        settings(1, Duration::from_secs(1)),
    );

    let liveness = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/livez")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let readiness = app
        .oneshot(
            Request::builder()
                .uri("/readyz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(liveness.status(), StatusCode::OK);
    assert_eq!(readiness.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn maps_expired_wrong_audience_client_and_key_failures() {
    let app = app_with_executor(
        Arc::new(AcceptingExecutor::default()),
        settings(1, Duration::from_secs(1)),
    );
    let cases = [
        (
            "expired",
            StatusCode::UNAUTHORIZED,
            "authentication_expired",
        ),
        (
            "wrong-audience",
            StatusCode::FORBIDDEN,
            "authentication_audience_invalid",
        ),
        (
            "wrong-client",
            StatusCode::FORBIDDEN,
            "authentication_client_denied",
        ),
        (
            "key-outage",
            StatusCode::SERVICE_UNAVAILABLE,
            "authentication_keys_unavailable",
        ),
    ];
    for (token, status, code) in cases {
        let response = app
            .clone()
            .oneshot(post(&operation_request(), Some(token)))
            .await
            .unwrap();
        assert_eq!(response.status(), status);
        assert_eq!(response_json(response).await["error"]["code"], code);
    }
}

#[tokio::test]
async fn rejects_malformed_oversized_and_semantically_invalid_requests() {
    let app = app_with_executor(
        Arc::new(AcceptingExecutor::default()),
        settings(1, Duration::from_secs(1)),
    );
    let malformed = Request::builder()
        .method("POST")
        .uri("/v1/operations")
        .header("authorization", "Bearer valid")
        .header(
            "content-type",
            bpane_runtime_contract::RUNTIME_BROKER_V1_MEDIA_TYPE,
        )
        .body(Body::from("not-json"))
        .unwrap();
    let response = app.clone().oneshot(malformed).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(response).await["error"]["code"],
        "request_malformed"
    );

    let oversized = Request::builder()
        .method("POST")
        .uri("/v1/operations")
        .header("authorization", "Bearer valid")
        .header(
            "content-type",
            bpane_runtime_contract::RUNTIME_BROKER_V1_MEDIA_TYPE,
        )
        .body(Body::from(vec![b'x'; 1_025]))
        .unwrap();
    let response = app.clone().oneshot(oversized).await.unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);

    let mut invalid = operation_request();
    invalid.request_id = Uuid::nil();
    let response = app
        .clone()
        .oneshot(post(&invalid, Some("valid")))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        response_json(response).await["error"]["code"],
        "invalid_resource_id"
    );

    let mut invalid_feature = operation_request();
    let RuntimeOperation::LaunchBrowser(browser) = &mut invalid_feature.operation else {
        panic!("test request must launch a browser");
    };
    browser.features = BrowserRuntimeFeatures {
        network_identity: BrowserNetworkIdentity {
            timezone: Some("../etc/passwd".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };
    let response = app
        .oneshot(post(&invalid_feature, Some("valid")))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        response_json(response).await["error"]["code"],
        "invalid_operation_parameters"
    );
}

#[tokio::test]
async fn rejects_unversioned_media_type() {
    let app = app_with_executor(
        Arc::new(AcceptingExecutor::default()),
        settings(1, Duration::from_secs(1)),
    );
    let request = operation_request();
    let raw = Request::builder()
        .method("POST")
        .uri("/v1/operations")
        .header("authorization", "Bearer valid")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&request).unwrap()))
        .unwrap();
    let response = app.oneshot(raw).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(
        response_json(response).await["error"]["code"],
        "unsupported_media_type"
    );
}

#[tokio::test]
async fn json_route_rejects_streaming_storage_actions() {
    let app = app_with_executor(
        Arc::new(AcceptingExecutor::default()),
        settings(1, Duration::from_secs(1)),
    );
    let request = RuntimeOperationRequest {
        api_version: BrokerApiVersion::V1,
        request_id: Uuid::now_v7(),
        idempotency_key: IdempotencyKey::new("storage:import:json-route").unwrap(),
        operation: RuntimeOperation::RunStorageHelper(StorageHelperRequest {
            action: StorageHelperAction::ImportBrowserContext,
            session_id: None,
            source_context_id: None,
            target_context_id: Some(Uuid::now_v7()),
            file_target: None,
            declared_payload_bytes: Some(4),
        }),
    };

    let response = app
        .clone()
        .oneshot(post(&request, Some("valid")))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        response_json(response).await["error"]["code"],
        "invalid_operation_parameters"
    );

    let measure = RuntimeOperationRequest {
        api_version: BrokerApiVersion::V1,
        request_id: Uuid::now_v7(),
        idempotency_key: IdempotencyKey::new("storage:measure:json-route").unwrap(),
        operation: RuntimeOperation::RunStorageHelper(StorageHelperRequest {
            action: StorageHelperAction::MeasureBrowserContext,
            session_id: None,
            source_context_id: Some(Uuid::now_v7()),
            target_context_id: None,
            file_target: None,
            declared_payload_bytes: None,
        }),
    };
    let response = app.oneshot(post(&measure, Some("valid"))).await.unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
}

#[tokio::test]
async fn exact_retry_is_cached_and_conflicting_reuse_is_denied() {
    let executor = Arc::new(AcceptingExecutor::default());
    let app = app_with_executor(executor.clone(), settings(1, Duration::from_secs(1)));
    let original = operation_request();
    let response = app
        .clone()
        .oneshot(post(&original, Some("valid")))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    let response = app
        .clone()
        .oneshot(post(&original, Some("valid")))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(response.headers()["x-bpane-idempotent-replay"], "true");
    assert_eq!(executor.executions.load(Ordering::SeqCst), 1);

    let mut conflicting = original.clone();
    conflicting.operation = RuntimeOperation::LaunchBrowser(BrowserRuntimeLaunchRequest {
        session_id: Uuid::now_v7(),
        browser_context_id: None,
        features: Default::default(),
    });
    let response = app
        .clone()
        .oneshot(post(&conflicting, Some("valid")))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        response_json(response).await["error"]["code"],
        "idempotency_conflict"
    );

    let mut replay = original;
    replay.idempotency_key = IdempotencyKey::new("browser:launch:different").unwrap();
    let response = app.oneshot(post(&replay, Some("valid"))).await.unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        response_json(response).await["error"]["code"],
        "replay_conflict"
    );
}

#[tokio::test]
async fn enforces_concurrency_backpressure() {
    let executor = Arc::new(BlockingExecutor {
        entered: Notify::new(),
        release: Notify::new(),
    });
    let app = app_with_executor(executor.clone(), settings(1, Duration::from_secs(2)));
    let first_app = app.clone();
    let first = tokio::spawn(async move {
        first_app
            .oneshot(post(&operation_request(), Some("valid")))
            .await
            .unwrap()
    });
    executor.entered.notified().await;
    let response = app
        .oneshot(post(&operation_request(), Some("valid")))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        response_json(response).await["error"]["code"],
        "operation_overloaded"
    );
    executor.release.notify_waiters();
    assert_eq!(first.await.unwrap().status(), StatusCode::ACCEPTED);
}

#[tokio::test]
async fn times_out_executor_and_redacts_request_secrets_from_errors() {
    let executor = Arc::new(BlockingExecutor {
        entered: Notify::new(),
        release: Notify::new(),
    });
    let app = app_with_executor(executor, settings(1, Duration::from_millis(5)));
    let response = app
        .oneshot(post(&operation_request(), Some("valid")))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    let json = response_json(response).await;
    assert_eq!(json["error"]["code"], "operation_timed_out");
    assert!(!json.to_string().contains("valid"));
}

#[tokio::test]
async fn foundation_executor_fails_closed() {
    let app = app_with_executor(
        Arc::new(crate::RejectingRuntimeExecutor),
        settings(1, Duration::from_secs(1)),
    );
    let response = app
        .oneshot(post(&operation_request(), Some("valid")))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        response_json(response).await,
        json!({
            "error": {
                "code": "adapter_unavailable",
                "message": "the requested runtime adapter is not enabled"
            }
        })
    );
}
