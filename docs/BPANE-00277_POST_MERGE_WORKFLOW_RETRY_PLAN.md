# BPANE-00277 Post-Merge Workflow Retry Plan

## Metadata

- Issue: `#277`
- State: Review
- Owner: BrowserPane maintainers
- Lane: Foundation
- Target gate: Contributor-loop publication reliability checkpoint
- Depends on: closed `#273` and `#275`, merged through PR `#274`
- Last verified commit/date: `aee785a1` / 2026-08-22

## Business Outcome

One transient GitHub Actions failure after an already-validated automatic merge
does not strand the local delivery loop, while repeated or mismatched failures
remain visible and stop all further issue delivery.

## Example Use Case

A change passes required branch checks and exact-head Compose, then merges. The
first main Compose attempt encounters one transient Keycloak route-readiness
timeout. The loop reruns only failed jobs for that workflow run and exact merge
SHA once. A passing rerun completes the iteration; another failure stops the
loop with both attempts and diagnostics recorded.

## Current Evidence

- PR #274 exact-head Compose run `32559531743` passed at
  `89e4a1841ebff2ef201822cf8cc9df71df58dc9f`.
- Published main Compose run `32560886312` then failed Admin Unified on one
  60-second `compose-admin-new-api-companion` authenticated-route timeout. Four
  other lanes passed or were cancelled by fail-fast rather than reporting the
  same deterministic defect.
- `wait_for_exact_workflow` can rerun cancelled runs through
  `AUTO_RERUN_CANCELLED`; failed post-merge runs return immediately.
- The loop already verifies workflow name and exact head SHA, records run ids,
  and refuses to start the next iteration until required post-merge workflows
  pass.

## Scope

- Add a separately named bounded failed post-merge run retry count with default
  `1`, lower bound `0`, and conservative upper bound.
- Apply it only in automatic-merge post-publication validation after the exact
  PR head passed all required pre-merge gates.
- Rerun failed jobs for the existing required workflow run and verify workflow,
  run id/attempt, and exact merge SHA before accepting the result.
- Keep failed and cancelled rerun budgets separate and deterministic.
- Record attempts and final fixed outcomes in console/journal evidence with
  bounded redacted diagnostics.
- Add deterministic shell tests and update loop README/help/context/roadmap.

## Non-Goals

- No retry of pre-merge Compose, ordinary branch checks, arbitrary workflows,
  unrelated SHAs, local product tests, or unbounded flake suppression.
- No automatic product-code repair after publication and no successful outcome
  when the bounded rerun still fails.
- No BrowserPane API, protocol, persistence, Admin-New product, SDK, CLI,
  runtime, deployment, or release-gate change.

## Decisions And Dependencies

- #273/#275 remain the owners of exact-head pre-merge evidence and readiness
  smoke semantics; both are complete.
- A successful exact-head pre-merge run is required before failed post-merge
  retry is eligible. This is recovery from a publication-time transient, not a
  replacement for pre-merge evidence.
- Default one is enough to distinguish a transient first attempt from a
  repeated failure without normalizing flaky checks.
- Repeated Admin-New authentication failure is a root-cause defect and cannot
  be waived by this issue.
- #277 precedes #266 only as contributor-loop reliability work; it does not
  change protocol scope or dependencies.

## Contract Changes

- API/OpenAPI: N/A; no product HTTP contract changes.
- Protocol/event schemas: N/A; no BrowserPane wire/event change.
- Database/migrations: N/A; no persisted product data.
- Admin-new: N/A; its Compose job is evidence, not modified product scope.
- CLI/SDK: N/A for BrowserPane product surfaces; `dev_loop/loop.sh` gains one
  contributor configuration variable and help/README documentation.
- Deployment/configuration: N/A for BrowserPane deployments. Local loop config
  validates the retry count before any mutation.
- README/ARCH/AGENTS/operator docs: root product README and ARCH are N/A;
  `dev_loop/README.md`, script help, current context, and roadmap are updated.

## Security And Data Impact

Reuse the approved project-scoped `gh` identity and existing workflow APIs. Do
not print tokens, environment values, raw logs, browser content, or diagnostics
that bypass the existing redaction path. Accept success only for the exact
required workflow and merge SHA. Bound retries, polling, timeout, and output so
an unavailable Actions service cannot create an infinite loop or resource
amplification.

## Migration, Compatibility, And Rollback

The setting is additive. Value `0` preserves fail-immediately behavior; default
`1` enables one failed-job rerun. Existing cancelled-run configuration remains
independent. Rollback is a script revert or setting the failed retry count to
`0`; there is no data migration. Older environments that omit the variable use
the documented default.

## Observability And Operator Feedback

Log the fixed workflow name, run id, exact SHA prefix, current attempt, retry
budget, dispatch result, and final fixed conclusion. Journal the iteration as
landed only after every required workflow passes. On exhaustion, retain
`post-merge-failed` and show the bounded existing failure snapshot plus rerun
attempt count. Never treat a stale or mismatched run as evidence.

## Implementation Slices

1. Add validated configuration and pure failed/cancelled retry-budget behavior.
2. Add exact-run failed-job rerun orchestration, evidence verification, and
   bounded diagnostics.
3. Extend deterministic loop tests, README/help/context, and final validation.

## Test Strategy

### Unit

Extend shell contracts for default, zero, valid bounded values, negative,
non-numeric, and over-limit configuration. Cover failed/cancelled budget
accounting and nounset-safe execution.

### Integration

Mock `gh run view` and `gh run rerun --failed` sequences for first-failure then
success, repeated failure, rerun rejection, wrong SHA, wrong workflow, missing
run, timeout, cancelled/failure interaction, and multiple required workflows.
Assert no next issue selection before final success.

### Smoke And E2E

Run the complete loop shell suite. Exercise a mocked full automatic-merge
iteration through pre-merge success, merge, post-merge failure, one rerun, and
landed outcome; repeat with exhaustion and verify terminal behavior. Record the
first later real loop-owned PR as live confirmation without making that future
delivery a dependency of this bounded implementation.

### Coverage And Quality

Run `bash dev_loop/tests/loop_test.sh`, repository document/workflow policy,
shell syntax checks, `git diff --check`, and the fast validation profile if
shared scripts change. Preserve all current assertions and add explicit branch
coverage for every retry outcome.

## Manual Test Sequence

1. Set the failed post-merge retry count to `1` in a disposable loop fixture.
2. Return an exact-SHA failed workflow followed by a passing rerun; verify one
   `--failed` rerun and a landed iteration.
3. Return two failures; verify terminal `post-merge-failed`, diagnostics, and no
   next issue selection.
4. Set the count to `0`; verify the first failure preserves current behavior.
5. Return cancelled then failed, failed then cancelled, wrong SHA, rerun
   rejection, and timeout; verify separate budgets and fail-closed outcomes.
6. Run the full loop test and repository-document suites and inspect the journal
   for bounded non-sensitive evidence.

## Documentation And Claim Impact

Document only contributor-loop retry semantics and the exact observed evidence.
Do not claim that Compose is flake-free, that a retry proves product health, or
that repeated failures are acceptable. Product maturity, API, runtime, and
deployment claims do not change.

## Definition Of Done

- One failed-job rerun is enabled by default only for eligible exact-SHA
  post-merge workflow validation.
- Repeated failure and every identity/SHA/dispatch/timeout mismatch halt before
  another issue starts.
- Failed and cancelled budgets, output, journal, and configuration are bounded
  and deterministic.
- Loop shell tests, syntax, repository documents, and diff validation pass.
- Issue, plan, loop README/help, current context, and roadmap agree.

## Post-Implementation Smoke Sequence

1. Run shell syntax and `bash dev_loop/tests/loop_test.sh`.
2. Run repository document/workflow validation and `git diff --check`.
3. Run mocked success-after-one-failure and repeated-failure full iterations.
4. Run wrong-SHA/workflow, unavailable run, rejected rerun, timeout, and mixed
   cancelled/failed scenarios.
5. Verify journal/output redaction and no next-issue selection on failure.
6. Link the first later real automatic-merge use as operational confirmation.

## Evidence Record

- Implementation commit: `aee785a1`.
- Configuration: `POST_MERGE_FAILED_RERUNS` defaults to `1`, accepts `0`
  through `3`, rejects empty, negative, non-numeric, and over-limit values, and
  is nounset-safe. `AUTO_RERUN_CANCELLED` remains independent.
- Mocked state matrix: exact run/SHA/workflow success after one failed-job
  rerun, repeated failure, disabled retry, rejected rerun, changed SHA, changed
  workflow identity, unavailable run, timeout, cancelled-then-failed,
  failed-then-cancelled, and stop-before-next-workflow all pass. The journal
  contract records fixed workflow/run/attempt/retry/conclusion fields without
  raw logs.
- `bash -n dev_loop/loop.sh dev_loop/tests/loop_test.sh`: passed.
- `bash dev_loop/tests/loop_test.sh`: passed, 150 tests.
- `node scripts/check-repository-documents.mjs`: passed, 118 Markdown, 19
  YAML, and 3 workflow documents.
- `node scripts/validate.mjs --profile fast`: passed all 44 stages, including
  repository/security/dependency checks, Rust fmt/clippy/tests/coverage, Node
  checks/tests/coverage/builds, OpenAPI compatibility, and egress observer
  checks.
- `git diff --check`: passed.
- API/OpenAPI, protocol, database, Admin-New product, product CLI/SDK, runtime,
  deployment, and real Compose stack validation: N/A; this changes only the
  contributor loop and its documentation. The first later loop-owned automatic
  merge remains the planned live confirmation because post-merge evidence
  cannot exist before this change is published.
