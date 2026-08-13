use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use bpane_runtime_contract::{
    BrokerApiVersion, BrowserRuntimeLaunchRequest, IdempotencyKey, RuntimeOperation,
    RuntimeOperationRequest, RuntimeOperationResult, SecretValue, RUNTIME_BROKER_V1_MEDIA_TYPE,
};
use serde_json::json;
use tokio::net::TcpListener;
use uuid::Uuid;

use super::*;

struct StaticTokenProvider {
    token: SecretValue,
}

#[async_trait]
impl AccessTokenProvider for StaticTokenProvider {
    async fn access_token(&self) -> Result<SecretValue, RuntimeBrokerClientError> {
        Ok(self.token.clone())
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
        }),
    }
}

fn client(base_url: String, timeout: Duration) -> HttpRuntimeBrokerClient {
    HttpRuntimeBrokerClient::new(
        RuntimeBrokerClientConfig {
            base_url,
            request_timeout: timeout,
            max_response_bytes: 8_192,
        },
        Arc::new(StaticTokenProvider {
            token: SecretValue::new("service-token-never-log").unwrap(),
        }),
    )
    .unwrap()
}

async fn spawn(router: Router) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    format!("http://{address}")
}

fn versioned_response(response: impl IntoResponse) -> Response {
    let mut response = response.into_response();
    response.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static(RUNTIME_BROKER_V1_MEDIA_TYPE),
    );
    response
}

#[tokio::test]
async fn sends_typed_authenticated_request_and_correlates_response() {
    async fn handler(headers: HeaderMap, body: String) -> Response {
        assert_eq!(headers["authorization"], "Bearer service-token-never-log");
        assert_eq!(headers["content-type"], RUNTIME_BROKER_V1_MEDIA_TYPE);
        assert_eq!(headers["accept"], RUNTIME_BROKER_V1_MEDIA_TYPE);
        let request: RuntimeOperationRequest = serde_json::from_str(&body).unwrap();
        versioned_response(Json(RuntimeOperationResponse {
            api_version: BrokerApiVersion::V1,
            request_id: request.request_id,
            result: RuntimeOperationResult::Accepted,
        }))
    }
    let base_url = spawn(Router::new().route("/v1/operations", post(handler))).await;
    let request = operation_request();

    let response = client(base_url, Duration::from_secs(1))
        .execute(&request)
        .await
        .unwrap();

    assert_eq!(response.request_id, request.request_id);
    assert_eq!(response.result, RuntimeOperationResult::Accepted);
}

#[tokio::test]
async fn maps_status_without_copying_untrusted_error_body() {
    let submitted = "backend-secret-never-return";
    let cases = [
        (
            StatusCode::UNAUTHORIZED,
            RuntimeBrokerClientErrorCode::AuthenticationRejected,
        ),
        (StatusCode::CONFLICT, RuntimeBrokerClientErrorCode::Conflict),
        (
            StatusCode::TOO_MANY_REQUESTS,
            RuntimeBrokerClientErrorCode::Overloaded,
        ),
        (
            StatusCode::SERVICE_UNAVAILABLE,
            RuntimeBrokerClientErrorCode::Unavailable,
        ),
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            RuntimeBrokerClientErrorCode::Rejected,
        ),
    ];
    for (status, expected) in cases {
        let router = Router::new().route(
            "/v1/operations",
            post(move || async move { (status, submitted) }),
        );
        let error = client(spawn(router).await, Duration::from_secs(1))
            .execute(&operation_request())
            .await
            .unwrap_err();
        assert_eq!(error.code, expected);
        assert!(!error.to_string().contains(submitted));
    }
}

#[tokio::test]
async fn rejects_timeout_oversized_malformed_and_uncorrelated_responses() {
    let request = operation_request();
    let timeout_router = Router::new().route(
        "/v1/operations",
        post(|| async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            StatusCode::ACCEPTED
        }),
    );
    let error = client(spawn(timeout_router).await, Duration::from_millis(5))
        .execute(&request)
        .await
        .unwrap_err();
    assert_eq!(error.code, RuntimeBrokerClientErrorCode::TimedOut);

    let oversized = Router::new().route(
        "/v1/operations",
        post(|| async { versioned_response((StatusCode::ACCEPTED, vec![b'x'; 8_193])) }),
    );
    let error = client(spawn(oversized).await, Duration::from_secs(1))
        .execute(&request)
        .await
        .unwrap_err();
    assert_eq!(error.code, RuntimeBrokerClientErrorCode::ResponseTooLarge);

    let malformed = Router::new().route(
        "/v1/operations",
        post(|| async { versioned_response((StatusCode::ACCEPTED, "not-json")) }),
    );
    let error = client(spawn(malformed).await, Duration::from_secs(1))
        .execute(&request)
        .await
        .unwrap_err();
    assert_eq!(error.code, RuntimeBrokerClientErrorCode::InvalidResponse);

    let uncorrelated = Router::new().route(
        "/v1/operations",
        post(|| async {
            versioned_response(Json(json!({
                "api_version": "v1",
                "request_id": Uuid::now_v7(),
                "result": { "state": "accepted" }
            })))
        }),
    );
    let error = client(spawn(uncorrelated).await, Duration::from_secs(1))
        .execute(&request)
        .await
        .unwrap_err();
    assert_eq!(error.code, RuntimeBrokerClientErrorCode::InvalidResponse);

    let request_id = request.request_id;
    let unversioned = Router::new().route(
        "/v1/operations",
        post(move || async move {
            Json(RuntimeOperationResponse {
                api_version: BrokerApiVersion::V1,
                request_id,
                result: RuntimeOperationResult::Accepted,
            })
        }),
    );
    let error = client(spawn(unversioned).await, Duration::from_secs(1))
        .execute(&request)
        .await
        .unwrap_err();
    assert_eq!(error.code, RuntimeBrokerClientErrorCode::InvalidResponse);
}

#[test]
fn rejects_unsafe_or_unbounded_client_configuration() {
    let provider: Arc<dyn AccessTokenProvider> = Arc::new(StaticTokenProvider {
        token: SecretValue::new("token").unwrap(),
    });
    let cases = [
        ("file:///tmp/socket", Duration::from_secs(1), 1),
        ("http://user:secret@broker", Duration::from_secs(1), 1),
        ("http://broker/internal", Duration::from_secs(1), 1),
        ("http://broker?secret=value", Duration::from_secs(1), 1),
        ("http://broker", Duration::ZERO, 1),
        ("http://broker", Duration::from_secs(1), 0),
        ("http://broker", Duration::from_secs(1), 1_048_577),
    ];
    for (base_url, request_timeout, max_response_bytes) in cases {
        assert_eq!(
            HttpRuntimeBrokerClient::new(
                RuntimeBrokerClientConfig {
                    base_url: base_url.to_string(),
                    request_timeout,
                    max_response_bytes,
                },
                Arc::clone(&provider),
            )
            .unwrap_err()
            .code,
            RuntimeBrokerClientErrorCode::InvalidConfiguration
        );
    }
}

#[tokio::test]
async fn rejects_invalid_request_before_token_or_network_work() {
    let base_url = spawn(Router::new().route(
        "/v1/operations",
        post(|| async { (StatusCode::INTERNAL_SERVER_ERROR, Body::empty()) }),
    ))
    .await;
    let mut request = operation_request();
    request.request_id = Uuid::nil();
    let error = client(base_url, Duration::from_secs(1))
        .execute(&request)
        .await
        .unwrap_err();
    assert_eq!(error.code, RuntimeBrokerClientErrorCode::InvalidRequest);
}
