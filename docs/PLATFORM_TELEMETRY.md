# Platform Telemetry

Parent issue: [#178](https://github.com/ITmedes/browserpane/issues/178)

Runtime-tracing checkpoint: [#227](https://github.com/ITmedes/browserpane/issues/227)

Subsystem-metrics checkpoint: [#229](https://github.com/ITmedes/browserpane/issues/229)

SLI/alert baseline checkpoint: [#231](https://github.com/ITmedes/browserpane/issues/231)

BrowserPane exposes a standards-based gateway metrics foundation at
`GET /metrics` on the gateway HTTP API port. It also has an opt-in
OpenTelemetry checkpoint for gateway-to-runtime-broker browser lifecycle
operations. The metrics response uses OpenMetrics; traces use W3C Trace Context
and OTLP gRPC.

These are bounded prototype checkpoints. BrowserPane includes an initial
Prometheus recording-rule, alert, and runbook starter pack, but it does not yet
provide complete cross-process tracing, calibrated contractual SLOs,
dashboards, synthetic checks, alert routing, or load evidence required for a
Production claim.

## Current Metrics

| Metric | Type | Purpose |
| --- | --- | --- |
| `browserpane_gateway_http_requests_total` | Counter family | Completed HTTP requests by bounded method, matched route template, and status class. |
| `browserpane_gateway_http_request_duration_seconds` | Histogram family | HTTP response latency with the same bounded dimensions. |
| `browserpane_gateway_http_requests_in_flight` | Gauge | Requests currently executing; cancellation and unwind release the gauge. |
| `browserpane_gateway_runtime_active_assignments` | Gauge | Runtime assignments in the ready state. |
| `browserpane_gateway_runtime_starting_assignments` | Gauge | Runtime assignments still starting. |
| `browserpane_gateway_runtime_assignment_limit` | Gauge | Configured maximum runtime assignments for the selected backend. |
| `browserpane_gateway_workflow_produced_file_*_total` | Label-free counters | Produced-file upload attempts that succeed or fail. |
| `browserpane_gateway_workflow_event_delivery_*_total` | Label-free counters | Signed callback attempts, successes, retries, and terminal failures. |
| `browserpane_gateway_workflow_retention_*_total` | Label-free counters | Retention passes, candidates, deletions/clears, and failures. |
| `browserpane_gateway_recording_artifact_finalize_*_total` | Label-free counters | Recording artifact finalization requests, successes, and failures. |
| `browserpane_gateway_recording_failures_total` | Label-free counter | Recording segments moved into a failed state. |
| `browserpane_gateway_recording_playback_*_total` | Label-free counters | Playback manifest/export requests, export outcomes, and exported bytes. |
| `browserpane_gateway_recording_retention_*_total` | Label-free counters | Recording retention passes, candidates, deleted artifacts, and failures. |

Runtime gauges are refreshed from the `SessionManager` facade when the endpoint
is scraped. Scraping does not run dependency probes, start a browser, or mutate
session lifecycle state. Workflow and recording counters share the exact
`WorkflowObservability` and `RecordingObservability` instances used by the
authenticated operations snapshots. They are monotonic only for the lifetime
of a gateway process and reset on restart; the collector owns time-series
continuity.

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

## Prometheus SLI And Alert Starter Pack

The observability example also loads:

- `recording-rules.yml`: 15 aggregate indicators for gateway request rate/5xx
  ratio/p95 latency, runtime assignment utilization, workflow upload and event
  delivery outcomes, recording finalization, playback, and retention failures,
- `alert-rules.yml`: 10 conservative starter alerts for scrape absence,
  sustained gateway errors, sustained runtime saturation, workflow delivery or
  artifact failures, recording/finalization/playback failures, and retention,
- `rule-tests.yml`: deterministic healthy, firing, hold, recovery, no-traffic,
  process-counter-reset, and target-absent behavior, and
- `docs/operations/PROMETHEUS_ALERT_RUNBOOK.md`: aggregate-only triage,
  mitigation, recovery, and escalation guidance for every alert.

Validate the exact Prometheus configuration and behavior with the pinned
upstream toolchain:

```bash
node --test scripts/observability/prometheus-rules-contract.test.mjs
node scripts/observability/validate-prometheus-rules.mjs
```

The Docker-backed validator runs Prometheus/`promtool` 3.12.0 from the immutable
image recorded in `deploy/examples/observability/README.md`. The static contract
is part of the fast validation floor; semantic `promtool` validation is a named
compose-class stage and an explicit required repository GitHub check.

Ratios intentionally remain undefined when there is no traffic, ratio alerts
use minimum-volume or operation-increase gates, and counter resets are tested as
normal gateway restart behavior. Proposed thresholds and hold times must be
calibrated against a named deployment and workload before paging, contractual
SLO, or external availability use. The example does not include Alertmanager
routing, notification receivers, dashboards, durable storage, or a public
listener.

## Runtime Tracing Checkpoint

When explicitly enabled, `bpane-gateway` and `bpane-runtime-broker` export a
shared trace for browser runtime lifecycle operations. The demonstrated path
contains fixed spans for:

- the matched gateway HTTP request,
- the gateway runtime-broker client call,
- broker authentication and operation execution,
- broker policy evaluation, and
- Docker browser create, start, inspect, stop, and remove stages as applicable.

The shared `bpane-telemetry` crate owns SDK setup and standard W3C propagation;
the shared `bpane-runtime-client` owns internal request injection. Valid caller
`traceparent` and `tracestate` headers can continue the trace. Invalid context
is ignored and cannot reject an otherwise valid BrowserPane request. Baggage is
not configured or propagated.

Tracing is disabled by default. Enable it for both services with a private
collector endpoint:

```env
OTEL_TRACES_EXPORTER=otlp
OTEL_EXPORTER_OTLP_ENDPOINT=http://otel-collector:4317
OTEL_EXPORTER_OTLP_PROTOCOL=grpc
OTEL_TRACES_SAMPLER=parentbased_always_on
```

`OTEL_SDK_DISABLED=true` or `OTEL_TRACES_EXPORTER=none` disables export. The
endpoint must be an explicit credential-free root HTTP(S) URL; URL userinfo,
queries, and fragments are rejected. This checkpoint supports OTLP gRPC only.
Standard batch queue, batch size, schedule delay, export timeout, and sampler
variables are validated against bounded settings at startup.

Invalid explicit configuration fails startup with a redacted category-level
error. Once initialized, collector unavailability does not fail runtime
admission or readiness. The bounded batch queue may drop telemetry during an
outage; it is not a durable audit channel. Export resumes for later operations
after the collector recovers.

## Trace Data Policy

Trace span names and attributes are allowlisted and bounded. Current attributes
are limited to fixed operation kind/action/result/stage plus standard matched
HTTP route, method, and status. Do not add owner, project, session, workflow,
recording, artifact, URL/path/query, header, secret, browser-content,
egress-content, raw-error, source-location, thread, or log-event values.

`tracestate` is standard vendor correlation metadata. Callers and collectors
must never place credentials, tenant/resource identifiers, personal data, or
browser data in it. BrowserPane does not reflect `traceparent` or `tracestate`
to public callers and does not enable baggage propagation.

The operator owns collector TLS/authentication, private network placement,
sampling, redaction, backend access, encryption, retention, deletion, and
availability. BrowserPane provides no collector or trace storage in its
supported four-service single-node profile. The file-export collector under
`deploy/single-node/fixture/` is test evidence only and must not be treated as a
production backend.

## Validation

Focused checks:

```bash
cargo test -p bpane-gateway metrics::tests
cargo test -p bpane-gateway api::tests::health
cargo test -p bpane-gateway observability::tests
cargo test -p bpane-telemetry
node --test scripts/runtime-tracing/*.test.mjs
node scripts/validate-runtime-tracing-fixture.mjs
cargo test -p bpane-gateway --test compose_api_surface \
  compose_gateway_health_and_readiness_surface -- --ignored --test-threads=1
cargo test -p bpane-gateway --test compose_api_surface_docker_pool \
  compose_docker_pool_session_capacity_api_surface -- --ignored --test-threads=1
./scripts/start-single-node-fixture.sh
node scripts/smoke-runtime-tracing.mjs
```

The Compose metric cases validate OpenMetrics headers and framing,
health/readiness request labels, readiness failure labels, unmatched-path
redaction, the complete label-free workflow/recording metric catalog, real
workflow upload and callback counter movement, runtime active/starting/limit
transitions, and absence of live resource identifiers. The recording browser
smoke verifies finalization, playback request/outcome/byte movement and
redaction against the real worker and artifact path. The tracing smoke proves
caller-context continuation,
gateway/broker parentage and stage coverage, malformed-context tolerance,
collector outage and recovery, and absence of sensitive fixture markers.

## Remaining #178 Work

- OpenTelemetry propagation and correlation across workers, stores, event
  delivery, and runtime paths beyond the current browser lifecycle checkpoint.
- Workflow queue/running gauges, operation latency, and storage, transport,
  dependency, worker-process, and host metrics beyond the current bounded
  workflow/recording counters.
- Calibrated SLO/error-budget definitions, dashboards, alert routing,
  synthetic checks, and workload-specific threshold evidence beyond the #231
  starter rules and runbooks.
- Reproducible load profiles and documented capacity envelopes.
