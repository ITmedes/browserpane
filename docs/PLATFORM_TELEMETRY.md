# Platform Telemetry

Issue: [#178](https://github.com/ITmedes/browserpane/issues/178)

BrowserPane exposes a standards-based gateway metrics foundation at
`GET /metrics` on the gateway HTTP API port. The response uses the OpenMetrics
text format and is suitable for Prometheus-compatible collectors.

This is the first platform-telemetry checkpoint. It does not yet provide the
complete cross-process tracing, SLO, dashboard, alert, synthetic-check, or load
evidence required for a Production claim.

## Current Metrics

| Metric | Type | Purpose |
| --- | --- | --- |
| `browserpane_gateway_http_requests_total` | Counter family | Completed HTTP requests by bounded method, matched route template, and status class. |
| `browserpane_gateway_http_request_duration_seconds` | Histogram family | HTTP response latency with the same bounded dimensions. |
| `browserpane_gateway_http_requests_in_flight` | Gauge | Requests currently executing; cancellation and unwind release the gauge. |
| `browserpane_gateway_runtime_active_assignments` | Gauge | Runtime assignments in the ready state. |
| `browserpane_gateway_runtime_starting_assignments` | Gauge | Runtime assignments still starting. |
| `browserpane_gateway_runtime_assignment_limit` | Gauge | Configured maximum runtime assignments for the selected backend. |

Runtime gauges are refreshed from the `SessionManager` facade when the endpoint
is scraped. Scraping does not run dependency probes, start a browser, or mutate
session lifecycle state.

## Label And Data Policy

HTTP metric labels are limited to:

- standard HTTP method or the fixed value `OTHER`,
- the matched Axum route template or the fixed value `unmatched`,
- fixed response classes `1xx` through `5xx` or `unknown`.

Metrics must not contain owner, project, session, workflow, recording, or
artifact identifiers; resource names; raw request paths; URLs; hostnames;
headers; credentials; error messages; browser content; or egress traffic. A new
label requires both a cardinality review and a sensitivity review.

The endpoint is intentionally unauthenticated for collector compatibility but
is not intended as a public Internet endpoint. Production deployments must
restrict the gateway HTTP listener or `/metrics` route to a trusted operator or
collector network. Deployment packaging and network-policy enforcement remain
owned by issue #66.

## Local Scrape

With the standard Compose stack running:

```bash
curl --fail --show-error --silent \
  http://localhost:8932/metrics
```

The response content type is
`application/openmetrics-text; version=1.0.0; charset=utf-8`, carries
`Cache-Control: no-store`, and ends with `# EOF`.

A minimal collector job is available at
[`deploy/examples/observability/prometheus.yml`](../deploy/examples/observability/prometheus.yml):

```yaml
scrape_configs:
  - job_name: browserpane-gateway
    metrics_path: /metrics
    static_configs:
      - targets: [gateway:8932]
```

The example assumes the collector shares a private network with the gateway.
Do not publish the scrape target merely to make the example reachable.

## Validation

Focused checks:

```bash
cargo test -p bpane-gateway metrics::tests
cargo test -p bpane-gateway api::tests::health
cargo test -p bpane-gateway --test compose_api_surface \
  compose_gateway_health_and_readiness_surface -- --ignored --test-threads=1
cargo test -p bpane-gateway --test compose_api_surface_docker_pool \
  compose_docker_pool_session_capacity_api_surface -- --ignored --test-threads=1
```

The Compose cases validate OpenMetrics headers and framing, health/readiness
request labels, readiness failure labels, unmatched-path redaction, runtime
active/starting/limit transitions, and absence of live session identifiers.

## Remaining #178 Work

- OpenTelemetry propagation and correlation across gateway, runtime broker,
  workers, stores, and event delivery.
- Recording, workflow, queue, storage, transport, dependency, and host metrics.
- Initial SLOs, recording rules, dashboards, alerts, synthetic checks, and
  response runbooks.
- Reproducible load profiles and documented capacity envelopes.
