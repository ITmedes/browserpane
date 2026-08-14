# BPANE-00178 Platform Telemetry Plan

Issue: [#178 Add platform telemetry, SLOs, and capacity evidence](https://github.com/ITmedes/browserpane/issues/178)

Status: implemented and validated; metrics-foundation checkpoint complete

## Business Case

BrowserPane already exposes useful session, recording, and workflow snapshots,
but operators cannot collect them through a standard monitoring interface. A
production deployment needs stable aggregate signals before it can define
service-level objectives, alerts, or defensible capacity limits. The first
checkpoint therefore establishes a standards-based metrics boundary rather than
adding another BrowserPane-specific diagnostics response.

## Example Use Case

An operator sees workflow starts slowing down during a pilot. Prometheus scrapes
the gateway and shows rising control-API latency while the runtime pool is at its
configured limit and another runtime is still starting. The operator can identify
capacity pressure without inspecting owner data, session identifiers, requested
URLs, credentials, or browser content.

## Checkpoint Scope

1. Add an application-owned `prometheus-client` registry and OpenMetrics text
   encoder to the gateway.
2. Instrument the HTTP control surface with RED signals:
   - request count,
   - response duration,
   - in-flight requests.
3. Export current runtime capacity:
   - active runtime assignments,
   - starting runtime assignments,
   - configured runtime assignment limit.
4. Expose `GET /metrics` as a scrape endpoint beside `/healthz` and `/readyz`.
5. Document the metric contract, bounded-label policy, private-network
   expectation, and a local scrape sequence.
6. Add unit, router integration, configuration, and compose smoke coverage.

## Architecture Decisions

### Registry ownership

Use the established `prometheus-client` crate with an explicit gateway-owned
registry. Do not install a process-global recorder. Explicit ownership keeps
parallel router tests isolated and leaves room for additional host and worker
registries without hidden global coupling.

### Metric names and labels

All exported metric names use the `browserpane_gateway_` prefix. HTTP metrics use
only bounded dimensions:

- HTTP method,
- matched Axum route template or a fixed `unmatched` fallback,
- response status class.

Do not use owner ids, project ids, session ids, workflow ids, recording ids,
resource names, raw request paths, URLs, hostnames, credentials, artifact refs,
or exception messages as metric labels. New labels require a cardinality and
sensitivity review.

### Runtime snapshot

The session-manager facade owns a sanitized aggregate capacity snapshot. Static
and Docker/broker-backed implementations report through the same contract. The
metrics handler refreshes gauges immediately before encoding, avoiding lifecycle
instrumentation spread across each runtime transition.

### Endpoint exposure

The metrics endpoint is unauthenticated for standard collector compatibility but
contains only aggregate, bounded data. It must be reachable only from a trusted
operator/collector network in production. The canonical local compose setup may
publish it indirectly through the existing development API port; #66 owns
production listener isolation and packaging.

## Error And Failure Behavior

- Metrics encoding failures return HTTP 500 without exposing internal details.
- An unmatched route is recorded with a fixed label, never its raw path.
- Metrics collection must not perform readiness probes, start runtimes, or mutate
  session lifecycle state.
- Runtime gauges must remain internally consistent when no runtime is assigned,
  while a runtime is starting, after startup, and after release.

## Implementation Steps

### 1. Metrics facade

- Add the standard metrics dependency.
- Implement the gateway registry, HTTP metric families, runtime gauges, and
  OpenMetrics encoder.
- Add focused tests for names, labels, histogram output, and sensitive-value
  absence.

### 2. Runtime capacity contract

- Add a sanitized capacity snapshot to `SessionManager`.
- Implement static-single and Docker/broker-backed snapshots.
- Cover empty, starting, ready, and released states without exposing assignment
  identifiers.

### 3. HTTP integration

- Add request instrumentation at the assembled gateway-router boundary.
- Add `GET /metrics` beside health/readiness routes and keep it outside drain
  rejection.
- Verify health, readiness, protected API, unmatched, and metrics requests.

### 4. Documentation and smoke

- Document metric names, label policy, scrape command, and production network
  expectation.
- Add a compose smoke that validates OpenMetrics content, request counters,
  runtime capacity, and absence of known owner/session values.
- Update the roadmap, maturity matrix, risk register, and validation matrix with
  checkpoint evidence without marking all of #178 complete.

## Acceptance Criteria

- `GET /metrics` returns valid OpenMetrics text with the expected content type.
- HTTP request count, duration, and in-flight metrics use matched route templates
  and bounded status classes.
- Runtime capacity reports aggregate active, starting, and limit values for every
  runtime backend.
- Metrics contain no dynamic resource identifiers, raw paths, URLs, credentials,
  browser content, or egress data.
- Metrics collection has no session/runtime lifecycle side effects.
- Unit, integration, and compose smoke tests cover success and failure paths.
- Operator documentation explains the private scrape-network requirement.

## Post-Implementation Smoke Sequence

1. Run gateway unit tests and strict Clippy for the metrics and runtime boundary.
2. Start the canonical compose stack and wait for `/readyz`.
3. Scrape `http://localhost:8932/metrics` and validate the OpenMetrics content
   type, `# EOF`, and gateway metric prefix.
4. Call `/healthz`, `/readyz`, one authenticated API route, and one unknown route;
   scrape again and verify their matched/fixed labels and status classes.
5. Create and start a Docker-backed session, scrape while it is active, then stop
   it and verify the aggregate runtime gauge returns to the expected value.
6. Search the scrape body for the test owner, session id, project id, and raw
   unknown path and verify none are present.
7. Run the gateway compose API and Docker-pool suites to confirm instrumentation
   did not change API or lifecycle behavior.
8. Run the repository documentation checks.

## Validation Evidence

Completed on 2026-08-14:

- `cargo test -p bpane-gateway`: 454 passed; one environment-gated test ignored.
- `cargo test -p bpane-gateway metrics::tests`: focused registry, encoding,
  bounded-label, and sensitive-value tests passed.
- `cargo test -p bpane-gateway --test compose_api_surface
  compose_gateway_health_and_readiness_surface -- --ignored --test-threads=1`:
  passed against the canonical compose stack, including healthy and degraded
  readiness metrics plus fixed unmatched-route redaction.
- `cargo test -p bpane-gateway --test compose_api_surface_docker_pool
  compose_docker_pool_session_capacity_api_surface -- --ignored
  --test-threads=1`: passed with two launched browser runtimes and verified the
  aggregate assignment sequence `2 -> 1 -> 0`, the configured limit, and absence
  of live session ids.
- `curl --fail-with-body --dump-header - http://localhost:8932/metrics`: returned
  HTTP 200, the OpenMetrics content type, `Cache-Control: no-store`, aggregate
  runtime gauges, bounded route labels, and `# EOF`.
- `cargo clippy -p bpane-gateway --all-targets -- -D warnings`: passed.
- `cargo fmt --all -- --check`: passed.
- `cargo llvm-cov -p bpane-gateway --tests --summary-only`: 454 passed and one
  environment-gated test was ignored; gateway test coverage reported 57.09%
  lines. The new `metrics.rs` boundary reported 94.71% line and 100% function
  coverage.
- `node scripts/check-repository-documents.mjs`: passed for 74 Markdown files,
  10 YAML files, and 3 workflows.
- `git diff --check`: passed.

## Follow-Up Checkpoints On #178

This plan does not close the parent issue. Subsequent bounded checkpoints remain:

1. OpenTelemetry trace propagation and correlation across gateway, runtime
   broker, workers, artifact stores, and callback delivery.
2. Recording, workflow, queue, storage, transport, and dependency metric
   expansion using the same facade.
3. Initial SLO definitions, recording rules, dashboards, alerts, synthetic
   checks, and operator runbooks.
4. Reproducible load profiles and documented capacity envelopes tied to release
   evidence.

## Out Of Scope

- A proprietary observability backend.
- Per-owner, per-project, per-session, or per-workflow metric labels.
- Requested URLs, headers, proxy data, browser contents, or decrypted traffic.
- Distributed tracing, dashboards, alerts, and final SLO thresholds in this
  checkpoint.
- Production deployment packaging or network-policy implementation owned by #66.
