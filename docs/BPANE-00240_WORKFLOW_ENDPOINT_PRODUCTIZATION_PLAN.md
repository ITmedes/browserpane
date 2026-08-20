# BPANE-00240 Workflow Endpoint Productization Plan

Issue: [#240](https://github.com/ITmedes/browserpane/issues/240)

Status: Planned follow-up specification; not an executable Phase 0 slice

Lane: Production and Enterprise

Target gates: Production Baseline and selected Phase N capabilities

Last reviewed: 2026-08-20

## Business Outcome

Productize the successful polling endpoint for repeated use across revisions,
environments, callers, and connector platforms without expanding BrowserPane
into a process orchestrator.

## Example Use Case

After a bounded Pilot succeeds, an operator promotes a compatible immutable
workflow revision behind the same endpoint key. Multiple authorized process
systems receive signed lifecycle callbacks, replay missed events, observe
distributed traces, and survive rollback without changing their integration
contract.

## Scope

- immutable endpoint revisions, compatibility checks, promotion, rollback,
  disablement, and deprecation,
- queue/execution deadlines, heartbeat/progress, cancellation acknowledgement,
  attempt identity, and retry guidance,
- W3C Trace Context across endpoint, run, session, worker, evidence, and
  delivery,
- versioned CloudEvents-compatible callbacks and AsyncAPI,
- signing rotation, retry/backoff, replay, redelivery, dead-letter diagnostics,
  ordering, and retention,
- pagination, caller/endpoint limits, overload, maintenance, and readiness,
- connector-compatible discovery exports and bounded reference adapters,
- matching Admin-New, CLI, OpenAPI, AsyncAPI, and conformance evidence.

## Non-Goals

- BPMN, subprocess, scheduling, compensation, or broad retries.
- Human Handoff, owned by `#71` if later selected.
- Teach Mode or repair, owned by `#171` if later selected.
- Supporting every BPM/iPaaS vendor through handwritten APIs.

## Dependencies And Risks

- Requires the complete bounded endpoint contract from `#172`.
- Consumes event, telemetry, authorization, identity, and release contracts from
  `#28`, `#70`, `#75`, `#176`, and `#178` as selected.
- At-least-once callback delivery requires receiver idempotency; do not promise
  exactly-once delivery.
- Connector exports must be generated from the canonical API contract.

## Implementation Slices

1. Endpoint revisions, compatibility, promotion, and rollback.
2. Deadlines, progress, cancellation acknowledgement, and overload semantics.
3. Trace propagation and versioned callback contract.
4. Replay, redelivery, rotation, and delivery diagnostics.
5. Connector discovery, compatibility exports, and reference adapters.
6. Admin-New, CLI, capacity, security, and release qualification.

Each slice requires its own bounded plan before implementation.

## Post-Implementation Smoke Sequence

1. Promote and roll back compatible immutable revisions behind one endpoint.
2. Reject an incompatible revision without changing active callers.
3. Correlate invocation through run, session, worker, evidence, and callback.
4. Drop, retry, deduplicate, and replay a signed callback.
5. Exercise deadline, stale heartbeat, cancellation acknowledgement, throttle,
   saturation, maintenance, and degraded dependency states.
6. Generate a connector profile and run reference adapters against the same
   conformance fixture.
7. Verify secrets and unrestricted browser/process data are absent from events,
   diagnostics, and connector exports.
