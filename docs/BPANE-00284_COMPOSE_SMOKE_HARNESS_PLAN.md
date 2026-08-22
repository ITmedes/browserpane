# BPANE-00284 Compose Smoke Harness Plan

## Metadata

- Issue: `#284`
- State: Qualified
- Owner: BrowserPane maintainers
- Lane: Foundation
- Target gate: deterministic Compose qualification
- Depends on: `#283`; completed `#235`
- Last verified commit/date: `e79164cc3a84` / 2026-08-22

## Business Outcome

Stateful integration tests fail for product behavior, not because a prior stage
left BrowserPane resources or Docker state behind. Maintainers gain one shared,
bounded way to wait for every supported readiness boundary and to prove cleanup.

## Example Use Case

A gateway API stage creates and stops a session while runtime teardown is still
in progress. The following MCP stage receives a distinct namespace and starts
only after the harness proves the prior stage has no active CI-owned sessions,
containers, or temporary storage. A teardown timeout fails with focused evidence
instead of surfacing later as an unrelated MCP connection timeout.

## Current Evidence

- Compose smokes use several package-local wait loops and browser locators with
  different readiness semantics.
- Recent failures have included resources still in `starting`, missing MCP
  runtime containers, stale admitted clients, file chooser timeouts, and
  session-selection timeouts.
- The gateway distinguishes process readiness, runtime state, transport state,
  and worker/artifact lifecycle, but tests do not consume one shared model.
- `scripts/ci/cleanup-compose.sh` cleans the workflow boundary; it does not prove
  stage-level isolation within broad sequential smoke suites.

## Scope

- Add a shared harness for OIDC, control-plane, runtime, transport, workflow
  worker, recording worker, and artifact readiness.
- Replace arbitrary sleeps in promotion smokes with bounded state-aware waits.
- Allocate unique run/stage namespaces to CI-owned resources.
- Add stage teardown and leak assertions for active test sessions, dynamic
  containers, temporary volumes/files, and helper processes where applicable.
- Preserve primary failure evidence when teardown also fails.
- Add deterministic dependency-loss, timeout, stale-resource, and cancellation
  fixtures.

## Non-Goals

- No scenario reduction or product lifecycle behavior change.
- No global deletion of operator/developer resources.
- No image fan-out, path selector, retry policy, or evidence reuse.
- No requirement that independent hosted jobs share a live Compose stack.

## Decisions And Dependencies

- Consume the versioned result and failure taxonomy from #283.
- Readiness states are typed and ordered; HTTP reachability alone is not browser
  or worker readiness.
- Cleanup only targets resources carrying the current CI namespace.
- Timeouts are finite, stage-specific, and report the last observed safe state.
- Stage isolation is the prerequisite for meaningful affected-area execution in
  #286.

## Contract Changes

- API/OpenAPI: N/A; consume existing authenticated status APIs.
- Protocol/event schemas: N/A.
- Database/migrations: N/A; CI namespacing uses existing metadata fields or
  labels without persistence changes.
- Admin-new: no user-visible feature; test helpers may be consolidated.
- CLI/SDK: no public contract change.
- Deployment/configuration: additive test namespace and deadline variables.
- README/ARCH/AGENTS/operator docs: synchronize validation/contributor commands
  if helper entry points change.

## Security And Data Impact

- Cleanup must prove ownership by CI namespace and must fail rather than delete
  resources it cannot attribute.
- Diagnostics follow #283 redaction and size bounds.
- Test credentials remain scoped and short lived; names must not embed identity
  claims or secrets.
- Concurrent jobs must not share mutable credentials or resource namespaces.

## Migration, Compatibility, And Rollback

- Migrate one lane at a time while retaining existing assertions and scenario
  order until parity is demonstrated.
- Keep a bounded compatibility adapter for package-local helpers during the
  transition; remove it only after all call sites use the shared harness.
- Rollback restores previous wait helpers and workflow wiring. Product state and
  database schemas are unaffected.
- Cancellation/cleanup behavior must be validated before making the harness
  mandatory for every lane.

## Observability And Operator Feedback

- Emit readiness boundary, attempt count, elapsed time, last state, deadline,
  namespace, and cleanup inventory into #283 evidence.
- Never emit bearer tokens, browser content, URLs, or raw unbounded API bodies.
- Distinguish setup, readiness, product action, and teardown failures.

## Implementation Slices

1. Inventory waits, sleeps, resource names, and teardown paths across all lanes.
2. Implement and unit-test shared deadline/readiness and namespace libraries.
3. Migrate gateway and browser integration stages, including negative fixtures.
4. Migrate Admin-New, compatibility-admin, MCP, workflow, and recording stages.
5. Enforce stage cleanup invariants and remove superseded helper duplication.

## Test Strategy

### Unit

- State transitions, deadlines, aborts, last-state reporting, namespaces,
  ownership checks, cleanup inventory, and dual primary/cleanup failures.

### Integration

- Delayed readiness, dependency loss/recovery, stale resources, worker failure,
  cancellation, timeout, and partial cleanup against controlled fixtures.
- Two concurrent namespaces cannot observe or remove each other's resources.

### Smoke And E2E

- Run every existing promotion lane through the shared harness.
- Run two consecutive full workflows and prove zero cross-run CI state.
- Cancel a running workflow and prove bounded cleanup and retained evidence.

### Coverage And Quality

- Preserve the full existing promotion inventory.
- Require changed-code coverage for harness libraries and shell/static checks for
  wrappers.
- Compare first-pass reliability and timing against the #283 baseline.

## Manual Test Sequence

1. Start a clean local Compose stack and run one namespaced gateway stage.
2. Inspect the readiness evidence and CI-owned resource inventory.
3. Force delayed runtime readiness and verify a bounded typed timeout.
4. Force teardown failure and verify the primary result remains visible.
5. Rerun the same stage with a new namespace and verify no collision.
6. Run the full hosted workflow twice and compare leak/cleanup summaries.

## Documentation And Claim Impact

Update `VALIDATION_MATRIX.md` and contributor/test guidance. This improves test
determinism only; it does not promote product maturity or deployment support.

## Definition Of Done

- All Compose lanes use shared typed waits and unique namespaces.
- Arbitrary readiness sleeps in the promoted paths are removed.
- Cleanup is ownership-safe, bounded, independently reported, and enforced.
- Existing positive, denial, recovery, workflow, recording, MCP, and admin
  scenarios remain covered.
- Two consecutive full runs show no cross-run CI-owned state.
- Evidence and timing comparison are linked in #284.

## Post-Implementation Smoke Sequence

1. Run harness unit and integration fixtures.
2. Run the canonical Compose profile locally with a unique namespace.
3. Exercise delayed readiness, stale resource, cancellation, and cleanup error.
4. Run all five hosted lanes twice consecutively.
5. Verify no CI-owned sessions, containers, volumes, or helpers remain.
6. Record reliability and timing deltas against #283.

## Evidence Record

Record the PR, commit, unit/integration outputs, two hosted workflow runs,
cancellation run, leak inventories, and baseline comparison in issue `#284`.
