# BPANE-00227 OpenTelemetry Runtime Tracing Plan

## Metadata

- Issue: [#227](https://github.com/ITmedes/browserpane/issues/227)
- State: Review
- Owner: BrowserPane maintainers
- Lane: Production
- Target gate: Production Baseline
- Depends on: #150, #178 metrics checkpoint, #214, #225/PR #226
- Last verified commit/date: implementation commits through `ee776b66` plus the
  Slice 4 fixture/qualification changes in this changeset, 2026-08-14

## Business Outcome

Operators can follow one browser runtime lifecycle operation across gateway and
runtime broker in a standard telemetry backend. The first tracing checkpoint
separates admission, broker policy/authentication, and Docker/runtime latency
without timestamp matching or exposing tenant/session/browser data.

## Example Use Case

An operator investigates a session that remains unavailable after Start. The
telemetry backend shows one trace spanning gateway admission, the authenticated
typed broker request, broker validation, and Docker create/start/status work.
The failing stage and duration are visible, while owner/session identifiers,
URLs, credentials, browser content, artifact refs, and raw errors are absent.

## Initial Evidence

- Gateway and broker already use `tracing` plus `tracing-subscriber`, but only
  install local formatted logging subscribers.
- Gateway exposes bounded OpenMetrics HTTP RED and runtime-capacity metrics.
- `bpane-runtime-client` is the shared authenticated gateway-to-broker HTTP
  boundary; broker APIs already carry typed idempotency request IDs and bounded
  audit events.
- Before this slice, no OpenTelemetry SDK/exporter, W3C propagation middleware,
  OTLP fixture, or shared trace existed.
- The single-node profile supplies the broker-only topology and live browser
  runtime qualification needed for a representative cross-service smoke.

## Scope

1. Add one small shared telemetry crate using maintained OpenTelemetry,
   `tracing-opentelemetry`, and W3C Trace Context implementations.
2. Use standard `OTEL_*` configuration. Trace export is disabled by default;
   explicitly selected OTLP export is startup-validated and uses bounded batch
   processing/shutdown.
3. Create inbound gateway HTTP spans with bounded matched-route, method, and
   status attributes and safe handling of absent/malformed caller context.
4. Inject the active context in `bpane-runtime-client` and extract it in broker
   middleware so runtime operations share one trace.
5. Instrument representative gateway admission/broker request, broker auth and
   policy, and Docker browser create/start/status stages.
6. Add a private local collector fixture and executable correlation/redaction
   smoke against the broker-only single-node path.
7. Document configuration, collector isolation, sampling/retention ownership,
   cardinality/redaction rules, failure behavior, and remaining #178 scope.

## Non-Goals

- Worker, host media, storage, callback, and every owner-API trace path.
- Production SLOs, dashboards, alerts, synthetics, or capacity/load claims.
- A BrowserPane-specific propagator, collector, or telemetry storage backend.
- Resource IDs, URLs, browser data, credentials, or raw errors as span
  attributes.
- Closing parent issue #178.

## Decisions And Dependencies

- W3C Trace Context is the wire contract; OpenTelemetry SDKs own parsing and
  serialization. BrowserPane will not manually parse `traceparent` or
  `tracestate`.
- `tracestate` remains standard vendor correlation metadata and must not carry
  credentials, tenant/resource identifiers, personal data, or browser data.
  Baggage propagation is not enabled.
- OTLP is the only exporter in this checkpoint. The collector remains the
  vendor-neutral routing/redaction/storage boundary.
- The binaries assign stable `service.name` values. Operators own endpoint,
  sampling, collector auth/TLS, backend retention, and collector availability.
- Invalid external context is ignored and replaced by a safe local root;
  tracing must not reject a valid BrowserPane request.
- Export failure is non-blocking after startup. Explicitly invalid exporter
  configuration fails startup with a redacted error.
- The shared runtime client performs injection once for browser, worker, and
  storage requests, while this smoke instruments only the browser lifecycle.

Primary references:

- [W3C Trace Context](https://www.w3.org/TR/trace-context/)
- [OpenTelemetry Rust](https://opentelemetry.io/docs/languages/rust/)
- [OpenTelemetry OTLP exporter](https://docs.rs/opentelemetry-otlp/latest/opentelemetry_otlp/)

## Contract Changes

- API/OpenAPI: no owner API schema or response change.
- Protocol/event schemas: W3C `traceparent`/`tracestate` on internal HTTP only;
  no BrowserPane wire-protocol change.
- Database/migrations: N/A; spans are exported, not persisted by BrowserPane.
- Admin-new: no new UI; existing session start/connect is the manual trigger.
- CLI/SDK: no new BrowserPane command; standard `OTEL_*` deployment variables.
- Deployment/configuration: optional private OTLP endpoint/configuration for
  gateway and broker plus a local collector fixture.
- README/ARCH/AGENTS/operator docs: document runtime roles, configuration,
  supported evidence, and bounded maturity.

## Security And Data Impact

- Accept trace context only as opaque standard correlation state; never use it
  for authentication, authorization, ownership, idempotency, or lookup.
- Do not reflect trace headers to public callers by default.
- Span names/attributes are allowlisted and bounded. Exclude owner, project,
  session, workflow, recording, file/artifact, URL/path/query, header, secret,
  browser-content, egress-content, and raw-error values.
- Collector ingress stays private; production collector TLS/auth, sampling,
  access, encryption, retention, deletion, and export policy are operator-owned.
- Batch queues, export timeout, and shutdown are bounded; exporter failure must
  not become runtime admission backpressure.

## Migration, Compatibility, And Rollback

- With tracing disabled, binaries retain current logging and runtime behavior.
- Existing deployments need no new variable. Enabling OTLP is additive.
- Mixed versions remain functional: absent propagation creates separate traces,
  while business requests continue normally.
- Rollback removes `OTEL_*` settings and deploys the previous images. No schema,
  state, or artifact migration is involved.

## Observability And Operator Feedback

- Stable service names: `bpane-gateway` and `bpane-runtime-broker`.
- Span names use fixed route/operation/stage values. Result attributes use
  bounded status/error-code categories.
- Formatted logs retain current messages and gain standard trace/span fields
  only while an active exported context exists.
- Exporter startup errors identify the invalid setting/category without printing
  endpoint credentials or headers. Runtime export outages are bounded and do
  not alter readiness in this checkpoint.

## Implementation Slices

### Slice 1: Shared telemetry foundation

1. Add `code/shared/bpane-telemetry` to the workspace.
2. Implement standard environment resolution, W3C propagation, optional OTLP
   provider construction, subscriber layering, bounded batch export, and
   explicit shutdown guard.
3. Add tests for disabled/default behavior, valid/invalid configuration,
   W3C extract/inject, invalid-context fallback, and redacted errors.

Commit: `feat(telemetry): add shared OpenTelemetry foundation`.

Status: complete in `c8936515`.

### Slice 2: Gateway and runtime-client propagation

1. Initialize gateway telemetry through the shared crate.
2. Add bounded route-template request spans and status recording.
3. Inject active context into every runtime-broker HTTP request.
4. Add unit/router/client tests for valid, absent, malformed, and sensitive
   header cases.

Commit: `feat(gateway): propagate runtime trace context`.

Status: complete in `398089a9`.

### Slice 3: Broker and Docker lifecycle spans

1. Initialize broker telemetry and extract context before operation handling.
2. Add fixed operation/auth/policy/execution spans and bounded result fields.
3. Add browser Docker create/start/status child spans without resource IDs.
4. Prove authentication, idempotency, timeout, and runtime behavior are
   unchanged.

Commit: `feat(runtime): trace broker browser lifecycle`.

Status: complete in `ee776b66`.

### Slice 4: Collector fixture, smoke, and documentation

1. Add a digest-pinned private OpenTelemetry Collector fixture/export capture.
2. Prove one gateway/broker trace, caller-context continuation, malformed
   fallback, collector outage tolerance, and sensitive-marker absence.
3. Update the telemetry/operator/security/validation/maturity/roadmap documents.

Commit: `test(telemetry): qualify cross-service runtime traces`.

Status: complete in this Slice 4 changeset; PR evidence pending.

## Test Strategy

### Unit

- configuration default/enable/disable/protocol/endpoint/error cases,
- W3C inject/extract and malformed context,
- bounded span names/attributes and forbidden-value absence,
- provider shutdown and disabled no-op behavior,
- runtime-client header propagation without overwriting unrelated headers.

### Integration

- gateway router continues/creates context using matched route metadata,
- broker middleware parents operation spans to injected gateway context,
- auth denials and policy/runtime failures record bounded codes,
- collector outage does not change operation result or readiness,
- existing broker contract/idempotency tests remain green.

### Smoke And E2E

- broker-only single-node fixture with private collector,
- authenticated session create/start/connect/stop,
- exported gateway/broker trace relationship and stage coverage,
- valid caller context and malformed-context fallback,
- collector outage plus recovery,
- redaction scan and existing broker isolation/restart/admin session smokes.

### Coverage And Quality

- `cargo fmt --all`, strict Clippy for all changed crates, focused crate tests,
  workspace tests, and Rust coverage ratchet,
- static deployment/document contracts and `git diff --check`,
- no coverage exclusion without a documented exporter/runtime reason.

## Manual Test Sequence

1. Start the supplied telemetry collector and broker-only single-node fixture.
2. Log in to `/admin-new/` and create a stopped session.
3. Start/connect the session and verify the browser opens normally.
4. Find the runtime-operation trace in the local collector output.
5. Confirm gateway admission/client and broker auth/policy/Docker spans share one
   trace and have correct parent/child ordering.
6. Confirm no owner/session id, visited URL, bearer, secret marker, or browser
   content appears in the span data.
7. Stop the collector, start/stop another session, and verify the operation
   remains functional with a bounded exporter diagnostic.
8. Restart the collector and verify later traces export again.
9. Stop the fixture and remove its transient collector data.

## Documentation And Claim Impact

- Move only gateway-to-broker browser lifecycle tracing from Planned to
  Prototype evidence.
- Keep cross-process worker/store/event tracing, complete SLOs, alerts,
  synthetics, and load envelopes open on #178.
- Update README/ARCH/AGENTS only for runnable standard configuration and the
  supported trace boundary. Do not imply production monitoring completeness.

## Definition Of Done

- #227 acceptance criteria and smoke sequence pass.
- Shared standard-library implementation replaces per-binary duplication.
- Disabled behavior is compatible and exporter failure is non-blocking.
- Propagation, malformed context, denial, outage, and redaction tests pass.
- Existing gateway/broker/session behavior remains green.
- Plan, issue, telemetry docs, maturity, risk, roadmap, and validation evidence
  are synchronized.

## Post-Implementation Smoke Sequence

1. Run shared telemetry, runtime-client, gateway, and broker unit tests.
2. Run strict workspace formatting/Clippy and Rust coverage.
3. Start the private collector plus broker-only fixture.
4. Execute session create/start/connect/stop with valid and malformed context.
5. Verify one cross-service trace and all redaction/cardinality invariants.
6. Interrupt the collector and prove session lifecycle remains available.
7. Run broker isolation/restart and admin-new session smokes.
8. Run repository deployment/document contracts and `git diff --check`.

## Evidence Record

- Plan: `a36f94dc`.
- Shared telemetry foundation: `c8936515`.
- Gateway and runtime-client propagation: `398089a9`.
- Broker and Docker lifecycle spans: `ee776b66`.
- Shared crate, runtime-client, broker, gateway-focused, strict Clippy, fixture
  contract, OTLP parser, and Docker image build checks passed locally.
- Live single-node smoke continued the supplied caller trace across
  `bpane-gateway` and `bpane-runtime-broker`, observed 24 lifecycle spans on the
  first trace and 22 after collector recovery, tolerated malformed context and
  collector outage, and found none of 21 sensitive markers in exported OTLP
  evidence.
- The local collector is fixture-only. The supported single-node profile stays
  four services and requires an operator-owned collector when tracing is
  enabled.
- PR, hosted checks, merge evidence, and #227 closure remain pending. Parent
  #178 remains open for broader traces, metrics, SLOs, alerts, runbooks, and
  capacity/load evidence.
