# BPANE-00172 Phase 0 Workflow Endpoint Plan

Issue: [#172](https://github.com/ITmedes/browserpane/issues/172)

Status: Qualified; implement after the supported workflow package contract in
`#47` is frozen

Lane: Pilot Value

Target gate: Phase 0 Operational Proof

Last reviewed: 2026-08-20

## Business Outcome

Expose one approved BrowserPane workflow as one stable asynchronous activity
that an external BPM can invoke, poll, and cancel without an interactive owner
token or knowledge of internal workflow ids.

## Example Use Case

A process engine invokes `retrieve-supplier-report` with a reporting period,
process correlation, and idempotency key. BrowserPane authenticates the caller,
validates input, runs one immutable workflow in one isolated browser session,
and returns schema-valid JSON plus an authorized report artifact. A portal
challenge returns terminal `external_intervention_required` to the process
engine instead of creating a BrowserPane human task.

## Current Baseline

Implemented:

- immutable Git commit-pinned workflow versions,
- workflow runs, session bindings, admission, cancellation, logs, events, and
  produced files,
- owner-scoped client request idempotency,
- OIDC validation and service-principal registry metadata,
- Admin-New workflow and run inspection.

Missing:

- stable project endpoint resource/key,
- enforced endpoint grants,
- input/output schema enforcement,
- endpoint/caller idempotency with payload fingerprint,
- typed outcomes and side-effect certainty,
- external invoke/status/cancel routes and conformance fixture.

## Scope

1. Add project-scoped endpoint resources with `draft`, `active`, and `disabled`
   lifecycle and one approved immutable workflow version binding.
2. Authorize external OIDC client-credentials callers through active registered
   service principals and explicit invoke/read/cancel project-endpoint grants.
3. Add asynchronous invoke, status, and cancel routes with stable links and a
   configured execution timeout.
4. Enforce JSON Schema Draft 2020-12 input before runtime creation and output
   before success.
5. Use RFC 9457 for HTTP request errors and stable domain outcomes for run
   completion.
6. Scope idempotency to endpoint/caller and reject key reuse with changed input.
7. Expose side effects as `none`, `confirmed`, or `uncertain`.
8. Separate bounded JSON from authorized artifact references with checksum,
   media type, retention, and expiry metadata.
9. Add matching Admin-New, CLI, OpenAPI, and fake-BPM polling surfaces.

## Non-Goals

- BrowserPane subprocesses or BPMN orchestration.
- BrowserPane-managed Human Handoff.
- Teach Mode or workflow generation.
- Webhooks, callbacks, endpoint revision promotion, connector generation,
  tracing expansion, throttling, or production SLOs; these belong to `#237`.
- BrowserPane-issued API keys.

## Dependencies And Risks

- `#47` is the required workflow package/publishing contract.
- `#174` consumes this endpoint for the selected Pilot process; endpoint
  development uses a deterministic fixture and does not wait for candidate
  selection.
- `#180` must pass before external Pilot use relies on open-source governance.
- `#21` and `#66` are conditional on selected artifact and deployment needs.
- An uncertain mutating side effect must never be presented as safely
  retryable.

## Implementation Steps

1. Freeze endpoint, grant, request, result, outcome, and side-effect schemas.
2. Add in-memory/Postgres resources and shared contract tests.
3. Implement service-principal authorization and negative cross-project tests.
4. Add schema validation, idempotency fingerprinting, and outcome mapping.
5. Add invoke/status/cancel API routes and workflow/session lifecycle binding.
6. Add Admin-New endpoint catalog/detail and caller-grant controls.
7. Add CLI operations, OpenAPI contract, examples, and fake-BPM fixture.
8. Run unit, store-contract, API, worker, compose, admin, CLI, and negative
   security validation.

## Test Strategy

### Unit

- endpoint lifecycle and key validation,
- JSON Schema compilation and bounded validation errors,
- request fingerprint and idempotency conflicts,
- outcome, retryability, timeout, cancellation, and side-effect mapping,
- artifact result bounds and redaction.

### Integration

- in-memory/Postgres parity,
- service-principal/project/endpoint authorization,
- input denial before runtime creation,
- invalid output preventing success,
- cancellation propagation and retained terminal evidence.

### Smoke And E2E

- valid client-credentials invocation through success,
- identical replay and changed-payload conflict,
- invalid input, invalid output, policy denial, runtime failure, timeout, and
  cancellation,
- challenge mapped to `external_intervention_required`,
- ambiguous submit mapped to `side_effect_state=uncertain`,
- API/Admin-New/CLI/OpenAPI/fake-BPM consistency.

## Post-Implementation Smoke Sequence

1. Create a project, service principal, immutable workflow version, and active
   endpoint.
2. Invoke with valid input and poll through schema-valid success.
3. Replay the same request and verify no duplicate browser action.
4. Reuse the key with changed input and verify conflict.
5. Verify invalid input creates no session or worker.
6. Verify invalid output cannot produce success.
7. Exercise denial, technical failure, timeout, and cancellation.
8. Verify challenge and uncertain-side-effect terminal outcomes.
9. Verify artifacts, correlation, redaction, retention, and authorization.
10. Run API, Admin-New, CLI, and fake-BPM conformance smokes.

## Definition Of Done

- The issue acceptance criteria and this plan agree.
- Every public surface exposes one endpoint contract.
- Negative authorization and pre-side-effect validation are proven.
- A target-like BPM can invoke, poll, reconcile, and cancel the activity.
- No Human Handoff, Teach Mode, subprocess, or advanced `#237` scope is added.
