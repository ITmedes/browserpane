use std::sync::Arc;
use std::time::Instant;

use axum::extract::{MatchedPath, Request, State};
use axum::http::{header, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use prometheus_client::encoding::{text::encode, EncodeLabelSet};
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::metrics::histogram::Histogram;
use prometheus_client::registry::Registry;
use tracing::Instrument;

use bpane_protocol::frame::ProtocolFailure;

use crate::recording::RecordingObservability;
use crate::session_manager::SessionManager;
use crate::workflow::WorkflowObservability;

const OPENMETRICS_CONTENT_TYPE: &str = "application/openmetrics-text; version=1.0.0; charset=utf-8";
const UNMATCHED_ROUTE: &str = "unmatched";

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct HttpRequestLabels {
    method: &'static str,
    route: String,
    status_class: &'static str,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct ProtocolFailureLabels {
    reason: &'static str,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct ProtocolDurationLabels {
    outcome: &'static str,
}

pub(crate) struct GatewayMetrics {
    registry: Registry,
    http_requests: Family<HttpRequestLabels, Counter>,
    http_request_duration_seconds: Family<HttpRequestLabels, Histogram>,
    http_requests_in_flight: Gauge,
    runtime_active_assignments: Gauge,
    runtime_starting_assignments: Gauge,
    runtime_assignment_limit: Gauge,
    protocol_negotiation_attempts: Counter,
    protocol_negotiation_successes: Counter,
    protocol_legacy_selections: Counter,
    protocol_negotiation_failures: Family<ProtocolFailureLabels, Counter>,
    protocol_handshake_duration_seconds: Family<ProtocolDurationLabels, Histogram>,
}

impl Default for GatewayMetrics {
    fn default() -> Self {
        let http_requests = Family::<HttpRequestLabels, Counter>::default();
        let http_request_duration_seconds =
            Family::<HttpRequestLabels, Histogram>::new_with_constructor(|| {
                Histogram::new([
                    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
                ])
            });
        let http_requests_in_flight = Gauge::default();
        let runtime_active_assignments = Gauge::default();
        let runtime_starting_assignments = Gauge::default();
        let runtime_assignment_limit = Gauge::default();
        let protocol_negotiation_attempts = Counter::default();
        let protocol_negotiation_successes = Counter::default();
        let protocol_legacy_selections = Counter::default();
        let protocol_negotiation_failures = Family::<ProtocolFailureLabels, Counter>::default();
        let protocol_handshake_duration_seconds =
            Family::<ProtocolDurationLabels, Histogram>::new_with_constructor(|| {
                Histogram::new([0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 3.0, 5.0, 10.0])
            });
        let mut registry = Registry::default();

        registry.register(
            "browserpane_gateway_http_requests",
            "Completed gateway HTTP requests",
            http_requests.clone(),
        );
        registry.register(
            "browserpane_gateway_http_request_duration_seconds",
            "Gateway HTTP request duration in seconds",
            http_request_duration_seconds.clone(),
        );
        registry.register(
            "browserpane_gateway_http_requests_in_flight",
            "Gateway HTTP requests currently in flight",
            http_requests_in_flight.clone(),
        );
        registry.register(
            "browserpane_gateway_runtime_active_assignments",
            "Runtime assignments in the ready state",
            runtime_active_assignments.clone(),
        );
        registry.register(
            "browserpane_gateway_runtime_starting_assignments",
            "Runtime assignments currently starting",
            runtime_starting_assignments.clone(),
        );
        registry.register(
            "browserpane_gateway_runtime_assignment_limit",
            "Configured maximum number of runtime assignments",
            runtime_assignment_limit.clone(),
        );
        registry.register(
            "browserpane_gateway_protocol_negotiation_attempts",
            "Authenticated browser protocol negotiation attempts",
            protocol_negotiation_attempts.clone(),
        );
        registry.register(
            "browserpane_gateway_protocol_negotiation_successes",
            "Successful protocol-v1 browser negotiations",
            protocol_negotiation_successes.clone(),
        );
        registry.register(
            "browserpane_gateway_protocol_legacy_selections",
            "Checked current-browser legacy protocol selections",
            protocol_legacy_selections.clone(),
        );
        registry.register(
            "browserpane_gateway_protocol_negotiation_failures",
            "Rejected browser protocol negotiations by fixed failure reason",
            protocol_negotiation_failures.clone(),
        );
        registry.register(
            "browserpane_gateway_protocol_handshake_duration_seconds",
            "Browser protocol handshake duration by fixed outcome",
            protocol_handshake_duration_seconds.clone(),
        );

        Self {
            registry,
            http_requests,
            http_request_duration_seconds,
            http_requests_in_flight,
            runtime_active_assignments,
            runtime_starting_assignments,
            runtime_assignment_limit,
            protocol_negotiation_attempts,
            protocol_negotiation_successes,
            protocol_legacy_selections,
            protocol_negotiation_failures,
            protocol_handshake_duration_seconds,
        }
    }
}

impl GatewayMetrics {
    pub(crate) fn with_observability(
        recording: &RecordingObservability,
        workflow: &WorkflowObservability,
    ) -> Self {
        let mut metrics = Self::default();
        recording.register_metrics(&mut metrics.registry);
        workflow.register_metrics(&mut metrics.registry);
        metrics
    }

    fn begin_http_request(&self) -> (Instant, InFlightGuard) {
        self.http_requests_in_flight.inc();
        (
            Instant::now(),
            InFlightGuard {
                gauge: self.http_requests_in_flight.clone(),
            },
        )
    }

    fn finish_http_request(
        &self,
        method: &Method,
        route: String,
        status: StatusCode,
        started_at: Instant,
    ) {
        let labels = HttpRequestLabels {
            method: bounded_method(method),
            route,
            status_class: status_class(status),
        };
        self.http_requests.get_or_create(&labels).inc();
        self.http_request_duration_seconds
            .get_or_create(&labels)
            .observe(started_at.elapsed().as_secs_f64());
    }

    pub(crate) fn begin_protocol_negotiation(&self) -> Instant {
        self.protocol_negotiation_attempts.inc();
        Instant::now()
    }

    pub(crate) fn record_protocol_negotiation_success(&self, started_at: Instant) {
        self.protocol_negotiation_successes.inc();
        self.observe_protocol_handshake("negotiated", started_at);
    }

    pub(crate) fn record_protocol_legacy_selection(&self, started_at: Instant) {
        self.protocol_legacy_selections.inc();
        self.observe_protocol_handshake("legacy", started_at);
    }

    pub(crate) fn record_protocol_negotiation_failure(
        &self,
        failure: ProtocolFailure,
        started_at: Instant,
    ) {
        self.protocol_negotiation_failures
            .get_or_create(&ProtocolFailureLabels {
                reason: failure.code(),
            })
            .inc();
        self.observe_protocol_handshake("rejected", started_at);
    }

    pub(crate) fn record_protocol_violation(&self, failure: ProtocolFailure) {
        self.protocol_negotiation_failures
            .get_or_create(&ProtocolFailureLabels {
                reason: failure.code(),
            })
            .inc();
    }

    fn observe_protocol_handshake(&self, outcome: &'static str, started_at: Instant) {
        self.protocol_handshake_duration_seconds
            .get_or_create(&ProtocolDurationLabels { outcome })
            .observe(started_at.elapsed().as_secs_f64());
    }

    async fn encode(&self, session_manager: &SessionManager) -> Result<String, std::fmt::Error> {
        let capacity = session_manager.capacity_snapshot().await;
        self.runtime_active_assignments
            .set(bounded_gauge(capacity.active_assignments));
        self.runtime_starting_assignments
            .set(bounded_gauge(capacity.starting_assignments));
        self.runtime_assignment_limit
            .set(bounded_gauge(capacity.assignment_limit));

        let mut output = String::new();
        encode(&mut output, &self.registry)?;
        Ok(output)
    }
}

struct InFlightGuard {
    gauge: Gauge,
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        self.gauge.dec();
    }
}

#[derive(Clone)]
struct MetricsEndpointState {
    metrics: Arc<GatewayMetrics>,
    session_manager: Arc<SessionManager>,
}

pub(crate) fn metrics_routes(
    metrics: Arc<GatewayMetrics>,
    session_manager: Arc<SessionManager>,
) -> Router {
    Router::new()
        .route("/metrics", get(get_metrics))
        .with_state(MetricsEndpointState {
            metrics,
            session_manager,
        })
}

pub(crate) async fn instrument_http_request(
    State(metrics): State<Arc<GatewayMetrics>>,
    request: Request,
    next: Next,
) -> Response {
    let method = request.method().clone();
    let route = request.extensions().get::<MatchedPath>().map_or_else(
        || UNMATCHED_ROUTE.to_string(),
        |path| path.as_str().to_string(),
    );
    let span = tracing::info_span!(
        "browserpane.http.server",
        otel.kind = "server",
        otel.status_code = tracing::field::Empty,
        "http.request.method" = bounded_method(&method),
        "http.route" = route.as_str(),
        "http.response.status_code" = tracing::field::Empty,
    );
    bpane_telemetry::set_parent_from_headers(&span, request.headers());
    let (started_at, _in_flight) = metrics.begin_http_request();
    let response = next.run(request).instrument(span.clone()).await;
    span.record("http.response.status_code", response.status().as_u16());
    span.record(
        "otel.status_code",
        if response.status().is_server_error() {
            "ERROR"
        } else {
            "OK"
        },
    );
    metrics.finish_http_request(&method, route, response.status(), started_at);
    response
}

async fn get_metrics(State(state): State<MetricsEndpointState>) -> Response {
    match state.metrics.encode(&state.session_manager).await {
        Ok(output) => (
            [
                (
                    header::CONTENT_TYPE,
                    HeaderValue::from_static(OPENMETRICS_CONTENT_TYPE),
                ),
                (header::CACHE_CONTROL, HeaderValue::from_static("no-store")),
            ],
            output,
        )
            .into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

fn bounded_method(method: &Method) -> &'static str {
    match *method {
        Method::GET => "GET",
        Method::POST => "POST",
        Method::PUT => "PUT",
        Method::PATCH => "PATCH",
        Method::DELETE => "DELETE",
        Method::HEAD => "HEAD",
        Method::OPTIONS => "OPTIONS",
        Method::CONNECT => "CONNECT",
        Method::TRACE => "TRACE",
        _ => "OTHER",
    }
}

fn status_class(status: StatusCode) -> &'static str {
    match status.as_u16() {
        100..=199 => "1xx",
        200..=299 => "2xx",
        300..=399 => "3xx",
        400..=499 => "4xx",
        500..=599 => "5xx",
        _ => "unknown",
    }
}

fn bounded_gauge(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use axum::body::Body;
    use axum::http::Request;
    use axum::middleware;
    use chrono::Utc;
    use tower::ServiceExt;

    use super::*;
    use crate::session_manager::SessionManagerConfig;

    fn test_session_manager() -> SessionManager {
        SessionManager::new(SessionManagerConfig::StaticSingle {
            agent_socket_path: "/tmp/test.sock".to_string(),
            cdp_endpoint: None,
            idle_timeout: Duration::from_secs(300),
        })
        .unwrap()
    }

    #[test]
    fn labels_are_bounded() {
        assert_eq!(bounded_method(&Method::GET), "GET");
        assert_eq!(
            bounded_method(&Method::from_bytes(b"PROPFIND").unwrap()),
            "OTHER"
        );
        assert_eq!(status_class(StatusCode::TOO_MANY_REQUESTS), "4xx");
        assert_eq!(status_class(StatusCode::BAD_GATEWAY), "5xx");
    }

    #[tokio::test]
    async fn encoding_exports_http_and_runtime_metrics_without_resource_values() {
        let metrics = GatewayMetrics::default();
        let session_manager = test_session_manager();
        let secret_session_id = uuid::Uuid::now_v7();
        session_manager.resolve(secret_session_id).await.unwrap();
        let started_at = Instant::now()
            .checked_sub(Duration::from_millis(10))
            .unwrap();

        let (_, in_flight) = metrics.begin_http_request();
        drop(in_flight);
        metrics.finish_http_request(
            &Method::GET,
            "/api/v1/sessions/{session_id}".to_string(),
            StatusCode::OK,
            started_at,
        );
        let protocol_started = Instant::now()
            .checked_sub(Duration::from_millis(10))
            .unwrap();
        let _ = metrics.begin_protocol_negotiation();
        metrics.record_protocol_negotiation_success(protocol_started);
        metrics.record_protocol_legacy_selection(protocol_started);
        metrics.record_protocol_negotiation_failure(
            ProtocolFailure::UnsupportedProtocolVersion,
            protocol_started,
        );
        metrics.record_protocol_violation(ProtocolFailure::UnexpectedProtocolFrame);

        let output = metrics.encode(&session_manager).await.unwrap();
        assert!(output.contains("browserpane_gateway_http_requests_total"));
        assert!(output.contains("method=\"GET\""));
        assert!(output.contains("route=\"/api/v1/sessions/{session_id}\""));
        assert!(output.contains("status_class=\"2xx\""));
        assert!(output.contains("browserpane_gateway_http_request_duration_seconds"));
        assert!(output.contains("browserpane_gateway_http_requests_in_flight 0"));
        assert!(output.contains("browserpane_gateway_runtime_active_assignments 1"));
        assert!(output.contains("browserpane_gateway_runtime_starting_assignments 0"));
        assert!(output.contains("browserpane_gateway_runtime_assignment_limit 1"));
        assert!(output.contains("browserpane_gateway_protocol_negotiation_attempts_total 1"));
        assert!(output.contains("browserpane_gateway_protocol_negotiation_successes_total 1"));
        assert!(output.contains("browserpane_gateway_protocol_legacy_selections_total 1"));
        assert!(output.contains("reason=\"unsupported_protocol_version\""));
        assert!(output.contains("reason=\"unexpected_protocol_frame\""));
        assert!(output.contains("outcome=\"negotiated\""));
        assert!(output.contains("outcome=\"legacy\""));
        assert!(output.contains("outcome=\"rejected\""));
        assert!(output.ends_with("# EOF\n"));
        assert!(!output.contains(&secret_session_id.to_string()));
    }

    #[tokio::test]
    async fn subsystem_metrics_share_observability_counters_without_dynamic_labels() {
        let recording = RecordingObservability::default();
        let workflow = WorkflowObservability::default();
        let metrics = GatewayMetrics::with_observability(&recording, &workflow);
        let sensitive_marker = uuid::Uuid::now_v7().to_string();
        let now = Utc::now();

        workflow.record_produced_file_upload();
        workflow.record_produced_file_upload();
        workflow.record_produced_file_upload_failure();
        workflow.record_event_delivery_attempt();
        workflow.record_event_delivery_success(now).await;
        workflow.record_event_delivery_retry();
        workflow.record_event_delivery_failure();
        workflow.record_retention_pass(now, 2, 3).await;
        workflow.record_retention_deleted_logs(4);
        workflow.record_retention_cleared_output();
        workflow.record_retention_failure();
        workflow.record_endpoint_invocation_accepted();
        workflow.record_endpoint_invocation_replay();
        workflow.record_endpoint_terminal(
            crate::workflow_endpoints::WorkflowOutcomeCategory::Success,
            crate::workflow_endpoints::WorkflowSideEffectState::Confirmed,
        );

        recording.record_artifact_finalize_request();
        recording.record_artifact_finalize_success();
        recording.record_artifact_finalize_failure();
        recording.record_recording_failure();
        recording.record_playback_manifest_request();
        recording.record_playback_export_request();
        recording.record_playback_export_success(4_096, now).await;
        recording.record_playback_export_failure();
        recording.record_retention_pass(now, 5).await;
        recording.record_retention_deleted_artifact();
        recording.record_retention_failure();

        let workflow_snapshot = workflow.snapshot().await;
        let recording_snapshot = recording.snapshot().await;
        let output = metrics.encode(&test_session_manager()).await.unwrap();

        assert_eq!(workflow_snapshot.produced_file_uploads_total, 2);
        assert_eq!(workflow_snapshot.retention_deleted_logs_total, 4);
        assert_eq!(recording_snapshot.playback_export_bytes_total, 4_096);
        assert_metric_value(
            &output,
            "browserpane_gateway_workflow_produced_file_uploads_total",
            2,
        );
        assert_metric_value(
            &output,
            "browserpane_gateway_workflow_retention_deleted_logs_total",
            4,
        );
        assert_metric_value(
            &output,
            "browserpane_gateway_recording_playback_export_bytes_total",
            4_096,
        );
        assert!(output.contains("browserpane_gateway_workflow_endpoint_operations_total"));
        assert!(output.contains("operation=\"invoke\""));
        assert!(output.contains("outcome=\"idempotent_replay\""));
        assert!(output.contains("side_effect_state=\"confirmed\""));

        for name in WORKFLOW_COUNTERS.into_iter().chain(RECORDING_COUNTERS) {
            assert!(
                output.contains(&format!("# HELP {name} ")),
                "missing HELP for {name}"
            );
            assert!(
                output.contains(&format!("# TYPE {name} counter")),
                "missing counter TYPE for {name}"
            );
            assert!(
                output
                    .lines()
                    .any(|line| line.starts_with(&format!("{name}_total "))),
                "missing sample for {name}"
            );
            assert!(
                !output
                    .lines()
                    .any(|line| line.starts_with(&format!("{name}_total{{"))),
                "unexpected labels for {name}"
            );
        }
        assert!(!output.contains(&sensitive_marker));
    }

    #[tokio::test]
    async fn request_instrumentation_uses_route_templates_and_ignores_malformed_context() {
        let metrics = Arc::new(GatewayMetrics::default());
        let app = Router::new()
            .route("/sessions/{session_id}", get(|| async { StatusCode::OK }))
            .layer(middleware::from_fn_with_state(
                metrics.clone(),
                instrument_http_request,
            ));
        let secret_session_id = uuid::Uuid::now_v7().to_string();
        let secret_query = "secret-query-marker";
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/sessions/{secret_session_id}?token={secret_query}"
                    ))
                    .header("traceparent", "malformed-sensitive-trace-marker")
                    .header("baggage", "password=sensitive-baggage-marker")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let output = metrics.encode(&test_session_manager()).await.unwrap();
        assert!(output.contains("route=\"/sessions/{session_id}\""));
        assert!(!output.contains(&secret_session_id));
        assert!(!output.contains(secret_query));
        assert!(!output.contains("sensitive-trace-marker"));
        assert!(!output.contains("sensitive-baggage-marker"));
    }

    fn assert_metric_value(output: &str, name: &str, expected: u64) {
        assert!(
            output
                .lines()
                .any(|line| line == format!("{name} {expected}")),
            "expected {name} to equal {expected}"
        );
    }

    const WORKFLOW_COUNTERS: [&str; 12] = [
        "browserpane_gateway_workflow_produced_file_uploads",
        "browserpane_gateway_workflow_produced_file_upload_failures",
        "browserpane_gateway_workflow_event_delivery_attempts",
        "browserpane_gateway_workflow_event_delivery_successes",
        "browserpane_gateway_workflow_event_delivery_retries",
        "browserpane_gateway_workflow_event_delivery_failures",
        "browserpane_gateway_workflow_retention_passes",
        "browserpane_gateway_workflow_retention_log_candidates",
        "browserpane_gateway_workflow_retention_output_candidates",
        "browserpane_gateway_workflow_retention_deleted_logs",
        "browserpane_gateway_workflow_retention_cleared_outputs",
        "browserpane_gateway_workflow_retention_failures",
    ];

    const RECORDING_COUNTERS: [&str; 13] = [
        "browserpane_gateway_recording_artifact_finalize_requests",
        "browserpane_gateway_recording_artifact_finalize_successes",
        "browserpane_gateway_recording_artifact_finalize_failures",
        "browserpane_gateway_recording_failures",
        "browserpane_gateway_recording_playback_manifest_requests",
        "browserpane_gateway_recording_playback_export_requests",
        "browserpane_gateway_recording_playback_export_successes",
        "browserpane_gateway_recording_playback_export_failures",
        "browserpane_gateway_recording_playback_export_bytes",
        "browserpane_gateway_recording_retention_passes",
        "browserpane_gateway_recording_retention_candidates",
        "browserpane_gateway_recording_retention_deleted_artifacts",
        "browserpane_gateway_recording_retention_failures",
    ];
}
