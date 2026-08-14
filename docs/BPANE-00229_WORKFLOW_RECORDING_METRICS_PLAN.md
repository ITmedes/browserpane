# BPANE-00229 Workflow And Recording Metrics Plan

## Metadata

- Issue: [#229](https://github.com/ITmedes/browserpane/issues/229)
- Parent: [#178](https://github.com/ITmedes/browserpane/issues/178)
- State: In Progress
- Lane: Production
- Target gate: Production Baseline
- Depends on: #150, #178 metrics checkpoint, #225, #227
- Last verified commit/date: `4fd35fba`, 2026-08-14

## Business Outcome

Operators can scrape workflow, event-delivery, recording, playback, and
retention outcomes from the same OpenMetrics endpoint as gateway HTTP and
runtime-capacity signals. The authenticated operations snapshots and metrics
surface use one counter source, so dashboards and alerts do not disagree with
the admin diagnostics.

## Example Use Case

A workflow reaches a terminal success state, but its produced file or signed
callback does not reach the downstream process. Prometheus shows produced-file
upload failures or callback retries increasing while gateway HTTP and runtime
capacity remain healthy. The operator can isolate the failing subsystem without
collecting owner ids, workflow ids, target URLs, artifact refs, or payload data.

## Current Evidence

- `GatewayMetrics` owns an isolated `prometheus-client` registry and exports
  bounded HTTP RED plus runtime-capacity metrics at `GET /metrics`.
- `WorkflowObservability` already records produced-file, event-delivery, and
  retention counters for the authenticated workflow operations snapshot.
- `RecordingObservability` already records finalization, playback, byte, and
  retention counters for the authenticated recording operations snapshot.
- Those observability counters currently use private atomics, so they cannot be
  registered with the OpenMetrics registry without duplicate accounting or an
  unsafe scrape-time delta bridge.

## Scope

1. Replace the workflow and recording counter storage with clonable
   `prometheus-client` counters while preserving the current facade and
   snapshot resource values.
2. Give each observability facade an explicit registry-registration method.
3. Build production `GatewayMetrics` with the same observability instances used
   by API and lifecycle services.
4. Export stable, help-documented, label-free counter families for:
   - workflow produced-file uploads and failures,
   - event-delivery attempts, successes, retries, and failures,
   - workflow retention passes/candidates/deletions/clears/failures,
   - recording artifact finalization requests/successes/failures,
   - recording failures,
   - playback manifest/export requests, successes/failures, and bytes,
   - recording retention passes/candidates/deletions/failures.
5. Add shared-source, monotonicity, exact-increment, type/help, and redaction
   tests plus representative Compose smoke evidence.
6. Synchronize platform telemetry, architecture, roadmap, maturity, risk,
   validation, issue context, and README where user/operator claims change.

## Non-Goals

- Dynamic owner, project, session, workflow, recording, URL, or artifact labels.
- New owner APIs, OpenAPI schemas, browser protocol messages, CLI commands, or
  admin-new controls.
- Current workflow queue/running gauges, host/transport/store/dependency
  metrics, or per-operation latency histograms.
- Traces beyond the browser lifecycle checkpoint in #227.
- SLO thresholds, recording rules, dashboards, alerts, synthetic checks, or
  capacity/load envelopes.
- Durable audit/accounting semantics or closing parent #178.

## Design Decisions

- Use the existing `prometheus-client` dependency. Do not introduce a second
  metrics facade, process-global recorder, custom OpenMetrics encoder, or
  scrape-time text concatenation.
- Counters remain process-local and reset on gateway restart. Prometheus owns
  time-series continuity; durable audit and usage records remain separate.
- Metric names are stable and label-free. The subsystem and operation are part
  of the metric name because the operation set is small and fixed.
- API snapshots continue to expose the existing JSON fields and values. This
  slice changes internal storage, not the owner API contract.
- Test routers may use isolated default metrics. The production router must
  explicitly register the exact observability instances held by `ApiState`.
- Duplicate metric registration must fail during construction rather than
  silently replacing a collector.

## Contract Changes

- API/OpenAPI: none.
- Database/migrations: none.
- Browser protocol: none.
- Admin-new/CLI/SDK: none.
- Metrics: additive `browserpane_gateway_workflow_*_total` and
  `browserpane_gateway_recording_*_total` series at the existing private
  `GET /metrics` endpoint.
- Deployment: no new service or configuration; existing collector scrapes gain
  the additional series after gateway rollout.

## Security And Data Impact

- New series have no labels and contain aggregate counts/bytes only.
- No resource identifiers, names, target URLs, headers, credentials, payloads,
  browser content, artifact refs, raw errors, or CA material may enter help
  text, names, or values.
- `/metrics` remains unauthenticated for collector compatibility and must stay
  on a trusted operator network.
- Metrics are operational signals, not tenant billing, audit, or compliance
  evidence.

## Implementation Slices

### Slice 1: Shared counter storage

1. Convert both observability facades from private atomics to clonable standard
   counters.
2. Preserve every record method and authenticated snapshot field.
3. Add exact-increment and clone-sharing unit tests.

Commit: `refactor(telemetry): share workflow and recording counters`.

### Slice 2: Gateway registry integration

1. Add explicit workflow and recording metric registration.
2. Construct production `GatewayMetrics` from the `ApiState` observability
   instances before the state is moved into the router.
3. Prove the snapshot and scrape observe the same increments exactly once.

Commit: `feat(telemetry): export workflow and recording metrics`.

### Slice 3: Integration smoke and documentation

1. Extend focused router/Compose coverage for success, failure, retry, bytes,
   retention, OpenMetrics types/help, and sensitive-marker absence.
2. Update telemetry and governance documents without broadening readiness
   claims.
3. Run focused, full fast-profile, and representative live qualifications.

Commit: `test(telemetry): qualify subsystem OpenMetrics`.

## Test Strategy

### Unit

- each record method increments exactly one counter,
- snapshots preserve all existing fields and timestamp behavior,
- cloned counters share state,
- registration emits every expected `TYPE`/`HELP` and `_total` series,
- no dynamic labels or sensitive fixture markers appear.

### Integration

- one shared observability instance drives both the authenticated snapshot and
  `/metrics`,
- separate test routers remain isolated,
- HTTP/runtime metrics remain present and unchanged,
- encoding failure behavior and scrape side-effect guarantees remain intact.

### Smoke And E2E

- run a workflow that uploads one produced file and sends a callback,
- run controlled callback retry/failure and upload failure cases,
- finalize a recording and export playback,
- run controlled recording/playback and retention failures,
- compare pre/post scrapes and scan for forbidden values,
- restart the gateway and confirm documented process-local reset semantics.

### Coverage And Quality

- gateway unit tests and strict changed-crate Clippy,
- Rust workspace tests and coverage ratchet,
- canonical fast validation profile,
- representative Compose API/workflow/recording smokes,
- docs/security contracts, formatting, and `git diff --check`.

## Manual Test Sequence

1. Start the single-node fixture and wait for readiness.
2. Scrape `/metrics` and save the initial workflow/recording counter values.
3. Execute the single-node qualification workflow and verify the produced-file
   upload counter increases.
4. Configure one local event subscription, execute a workflow, and verify
   delivery attempt plus success or bounded retry/failure changes.
5. Create an always-recorded session, finalize it, and download the playback
   export.
6. Verify finalization, playback export, and playback byte counters increase.
7. Trigger controlled upload, delivery, export, and retention failures and
   verify only the expected bounded failure counters change.
8. Search the scrape for owner/session/workflow/recording ids, target URLs,
   bearer values, payload markers, and artifact refs; all must be absent.
9. Restart the gateway and confirm the process-local counters reset while
   persisted resources remain available.

## Definition Of Done

- Issue #229 scope and acceptance criteria pass.
- API snapshots and OpenMetrics use the same standard counter instances.
- Every scoped signal is exported with stable names and no dynamic labels.
- Success, failure, retry, retention, byte, restart, and redaction paths are
  covered at the appropriate unit/integration/smoke level.
- Existing HTTP/runtime metrics and workflow/recording behavior remain green.
- README and operator/governance docs are synchronized, or the PR explicitly
  records why a document needs no change.
- Parent #178 remains open for queue/state gauges, latency, broader traces,
  SLOs/dashboards/alerts/synthetics, and capacity evidence.

## Post-Implementation Smoke Sequence

1. Run workflow/recording observability and gateway metrics unit tests.
2. Run gateway tests, strict Clippy, formatting, and Rust coverage.
3. Run repository docs/security/OpenAPI contracts.
4. Start the broker-only single-node fixture and take a baseline scrape.
5. Exercise workflow upload/callback success and controlled failure paths.
6. Exercise recording finalization/playback success and controlled failures.
7. Verify exact counter deltas, OpenMetrics framing, and forbidden-value
   absence.
8. Restart the gateway and verify reset semantics plus persisted-resource
   availability.
9. Run the existing single-node qualification and runtime-tracing smoke.
10. Run the canonical fast profile and impacted Compose lanes.
