# BPANE-00268 Protocol Compatibility Qualification Plan

## Metadata

- Issue: `#268`
- State: Qualified
- Owner: BrowserPane maintainers
- Lane: Production
- Target gate: Production Baseline protocol compatibility checkpoint
- Depends on: closed `#263`, `#264`, `#265`, `#266`, and `#267`
- Parent program: `#175`; protocol slice 6 of 6 and closure owner
- Last verified commit/date: `e21e2206a93a` / 2026-08-21

## Business Outcome

Operators have one executable rolling-upgrade, rollback, feature-regression,
failure-isolation, and diagnostics checkpoint for BrowserPane protocol v1.

## Example Use Case

An operator deploys gateway first and browser second, observes explicit v1 or
legacy mode in the existing Admin-New session view, exercises browser features,
rejects an unsupported peer without disturbing another viewer, and validates
the documented rollback before accepting the protocol checkpoint.

## Current Evidence

#263-#267 are designed to provide contract, codec, gateway, browser, and parser
evidence separately. No single current Compose matrix proves those components,
Admin-New diagnostics, rollout defaults, rollback, and feature paths together.

## Scope

- Add a supported-Compose matrix for v1/current, both checked legacy rollout
  directions, unsupported/malformed/downgrade, legacy disable/recovery,
  restricted viewer, late join, owner promotion, reconnect, and cleanup.
- Exercise tiles, ROI video, input, independent resize, clipboard, desktop
  audio, and file transfer where enabled, plus recording/workflow connection
  paths affected by session readiness.
- Integrate bounded live version/mode/capability/outcome diagnostics and typed
  feedback into existing Admin-New session preview/observability surfaces.
- Finalize gateway metrics/logs, SDK evidence, configuration, operator rollout/
  rollback, README, ARCH, validation, maturity, R-007, and release notes.
- Reconcile children and close #175 only after all evidence passes.

## Non-Goals

- Persistent protocol history, standalone catalog, product CLI resource,
  release channels/SBOM/signing, threat-model closure, capacity optimization,
  legacy removal, or broad compatibility/Production claims.

## Decisions And Dependencies

- All predecessor issues must be closed before implementation starts.
- Initial legacy defaults and removal authority remain as frozen by #263 and
  later #75 release governance.
- Existing Admin-New session surfaces consume diagnostics; no new route exists.
- #72, #75, #168, and #169 remain independent after #175 closes.

## Contract Changes

- API/OpenAPI and database: N/A; diagnostics remain connection-local.
- Protocol/event schemas: no new semantics beyond prior child fixes.
- Admin-new: live session compatibility details and typed connection feedback.
- CLI/SDK: no product CLI command; verify the SDK read-only snapshot/errors.
- Deployment/configuration: documented legacy settings, timeout, rollout order,
  disablement, rollback, and exact fixture matrix.
- README/ARCH/AGENTS/operator docs: complete support and validation boundary.

## Security And Data Impact

UI, SDK, logs, and metrics expose only bounded version, mode, capability names,
duration, and fixed outcomes. Exclude credentials, URLs, claims, browser/media/
file content, raw errors, paths, and resource IDs in metric labels. Negative
peers must not disturb healthy clients or start resources before negotiation.

## Migration, Compatibility, And Rollback

Prove gateway-first and client-first overlap, reversible legacy disablement,
and previous-fixture recovery. Publish exact supported rows and limitations.
Changing or removing legacy defaults remains #75 work.

## Observability And Operator Feedback

Verify label-bounded metrics, sanitized gateway logs, SDK snapshot, and
Admin-New panel/error behavior for success, legacy, rejection, and timeout.
Diagnostics are live, not a durable audit/history store.

## Implementation Slices

1. Compose fixture matrix and current feature/negative/recovery automation.
2. Admin-New diagnostics and typed feedback with component tests/smokes.
3. Operator/configuration/docs/evidence reconciliation and #175 closure audit.

## Test Strategy

### Unit

Cover Admin-New view models/components, SDK diagnostics/errors, metric labels,
configuration parsing, and fixed user-visible states.

### Integration

Cover all version/legacy/error rows, feature intersections, shared-session
isolation, diagnostics redaction, and recording/workflow readiness consumers.

### Smoke And E2E

Run the real Compose matrix, current browser feature smokes, restricted viewer,
multisession/reconnect, Admin-New live/observability, workflow, recording, and
cleanup paths affected by protocol readiness.

### Coverage And Quality

Run impacted Rust/Node unit, integration, coverage, build/type/lint, Compose API,
browser-client, Admin-New, workflow, recording, dependency, and repository
validation checks. Do not lower floors; record any device-gated exclusion.

## Manual Test Sequence

1. Run current/current v1 and all enabled feature paths.
2. Run gateway-first and client-first checked legacy fixtures.
3. Disable/restore each legacy setting and verify typed outcomes/recovery.
4. Run unsupported/malformed/downgrade/limit cases beside a healthy viewer.
5. Exercise viewer, late join, promotion, reconnect, recording, and workflow.
6. Inspect Admin-New, SDK, gateway logs, and metrics for usefulness/redaction.
7. Execute rollback, restore v1 defaults, clean artifacts, and link evidence.

## Documentation And Claim Impact

After all evidence, BrowserPane may claim v1 compatibility only for the exact
published matrix and named deployment profiles. It may not claim an industry
standard, arbitrary clients, Production, scale, or third-party interoperability.

## Definition Of Done

- #263-#267 are closed with linked evidence.
- Complete Compose, feature, negative, recovery, and rollback matrix passes.
- Admin-New/SDK/log/metric diagnostics are bounded and redacted.
- README, ARCH, operator/configuration, validation, maturity, risk, issue, and
  release-note wording match implementation.
- #175 closes with a child/evidence checklist; adjacent owners stay open.

## Post-Implementation Smoke Sequence

1. Run current/current v1 plus all enabled browser features.
2. Run both rolling-upgrade directions and rollback.
3. Run legacy disable/recovery and complete negative matrix.
4. Run viewer/late-join/reconnect/recording/workflow paths.
5. Verify Admin-New/SDK/log/metric diagnostics and redaction.
6. Run all impacted quality/coverage/Compose/repository checks and cleanup.

## Evidence Record

Record PR/commit, predecessor links, exact fixture/matrix revisions, Compose and
feature results, coverage, screenshots where useful, redaction/cardinality
review, rollout/rollback result, docs/claims, #175 closure, and residual risks.
