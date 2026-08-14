use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use bpane_runtime_broker::{
    build_router, AuthenticationError, BrokerApiSettings, BrokerAuthenticator, BrokerState,
    ExecutionError, LedgerConfig, OperationLedger, RuntimeOperationExecutor, ServicePrincipal,
};
use bpane_runtime_contract::{
    BrokerApiVersion, BrowserRuntimeLaunchRequest, IdempotencyKey, RuntimeOperation,
    RuntimeOperationRequest, RuntimeOperationResult, RUNTIME_BROKER_V1_MEDIA_TYPE,
};
use opentelemetry::trace::{SpanId, TraceId, TracerProvider as _};
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::{InMemorySpanExporter, SdkTracerProvider, SpanData};
use tower::ServiceExt;
use tracing::instrument::WithSubscriber;
use tracing_subscriber::prelude::*;
use uuid::Uuid;

struct Authenticator;

#[async_trait]
impl BrokerAuthenticator for Authenticator {
    async fn authenticate(&self, token: &str) -> Result<ServicePrincipal, AuthenticationError> {
        assert_eq!(token, "service-token-sensitive-marker");
        Ok(ServicePrincipal {
            subject: "gateway-service".to_string(),
            client_id: "gateway-client".to_string(),
            token_id: "token-id-sensitive-marker".to_string(),
            expires_at: 4_000_000_000,
        })
    }
}

struct Executor;

#[async_trait]
impl RuntimeOperationExecutor for Executor {
    async fn execute(
        &self,
        _request: &RuntimeOperationRequest,
    ) -> Result<RuntimeOperationResult, ExecutionError> {
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

fn span<'a>(spans: &'a [SpanData], name: &str) -> &'a SpanData {
    spans
        .iter()
        .find(|span| span.name == name)
        .unwrap_or_else(|| panic!("missing span {name}: {spans:?}"))
}

#[tokio::test]
async fn continues_w3c_parent_with_bounded_sibling_stages_and_redacted_events() {
    opentelemetry::global::set_text_map_propagator(TraceContextPropagator::new());
    let exporter = InMemorySpanExporter::default();
    let provider = SdkTracerProvider::builder()
        .with_simple_exporter(exporter.clone())
        .build();
    let tracer = provider.tracer("bpane-runtime-broker-test");
    let subscriber =
        tracing_subscriber::registry().with(tracing_opentelemetry::layer().with_tracer(tracer));
    let operation = operation_request();
    let secret_session_id = operation.operation.resource_id().to_string();
    let secret_request_id = operation.request_id.to_string();
    let http_request = Request::builder()
        .method("POST")
        .uri("/v1/operations")
        .header("authorization", "Bearer service-token-sensitive-marker")
        .header("content-type", RUNTIME_BROKER_V1_MEDIA_TYPE)
        .header(
            "traceparent",
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
        )
        .header("baggage", "password=sensitive-baggage-marker")
        .body(Body::from(serde_json::to_vec(&operation).unwrap()))
        .unwrap();
    let settings = BrokerApiSettings {
        max_request_bytes: NonZeroUsize::new(65_536).unwrap(),
        max_storage_payload_bytes: NonZeroUsize::new(1024 * 1024).unwrap(),
        max_concurrent: NonZeroUsize::new(1).unwrap(),
        operation_timeout: Duration::from_secs(1),
    };
    let state = BrokerState::new(
        Arc::new(Authenticator),
        Arc::new(Executor),
        Arc::new(OperationLedger::new(LedgerConfig {
            capacity: NonZeroUsize::new(16).unwrap(),
            completed_ttl: Duration::from_secs(60),
        })),
        settings,
    );

    let response = async { build_router(state).oneshot(http_request).await.unwrap() }
        .with_subscriber(subscriber)
        .await;

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert!(!response.headers().contains_key("traceparent"));
    assert!(!response.headers().contains_key("tracestate"));
    assert!(!response.headers().contains_key("baggage"));
    provider.force_flush().unwrap();
    let spans = exporter.get_finished_spans().unwrap();
    let server = span(&spans, "browserpane.http.server");
    let authentication = span(&spans, "browserpane.runtime_broker.authenticate");
    let execution = span(&spans, "browserpane.runtime_broker.execute");
    assert_eq!(
        server.span_context.trace_id(),
        TraceId::from_hex("4bf92f3577b34da6a3ce929d0e0e4736").unwrap()
    );
    assert_eq!(
        server.parent_span_id,
        SpanId::from_hex("00f067aa0ba902b7").unwrap()
    );
    assert_eq!(authentication.parent_span_id, server.span_context.span_id());
    assert_eq!(execution.parent_span_id, server.span_context.span_id());
    assert!(server.attributes.iter().any(|attribute| {
        attribute.key.as_str() == "http.route" && attribute.value.as_str() == "/v1/operations"
    }));
    assert!(execution.attributes.iter().any(|attribute| {
        attribute.key.as_str() == "browserpane.operation.kind"
            && attribute.value.as_str() == "browser_runtime"
    }));
    let exported = format!("{spans:?}");
    for forbidden in [
        secret_session_id.as_str(),
        secret_request_id.as_str(),
        "service-token-sensitive-marker",
        "token-id-sensitive-marker",
        "sensitive-baggage-marker",
    ] {
        assert!(!exported.contains(forbidden), "exported {forbidden}");
    }
}
