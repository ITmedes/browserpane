# BPANE-00273 Pre-Merge Compose Gate Plan

## Metadata

- Issue: `#273`
- State: In Progress
- Owner: BrowserPane maintainers / Codex
- Lane: Foundation
- Target gate: Bounded automatic delivery without publishing the first full
  Compose failure to `main`
- Depends on: `#241` Codex development loop (complete), existing Compose
  `workflow_dispatch`, and opt-in `AUTO_MERGE=1`
- Last verified commit/date: `6dd4838c` / 2026-08-22

## Business Outcome

BrowserPane's automatic local delivery loop obtains full Compose evidence while
the implementation pull request is still open and repairable. A browser,
gateway, admin, or integration regression therefore enters the existing bounded
repair cycle before publication instead of first appearing on `main` after the
issue and PR have closed.

## Example Use Case

A protocol implementation passes the lightweight pull-request Validation job,
but a browser-facing Compose smoke leaves its runtime in `starting`. With
automatic merge enabled, the loop dispatches Compose against the exact PR head,
observes the failed run, and gives that run and its failed jobs to a fresh repair
session. The repair updates the same PR. The changed head must pass Validation
and a new exact-head Compose run before it can merge. Supervised
`AUTO_MERGE=0` operation still leaves the green PR open without spending the
full Compose runner budget.

## Current Evidence

- `.github/workflows/compose.yml` intentionally runs on `main`, schedules, and
  manual dispatch, but not on every pull request.
- `dev_loop/loop.sh` currently waits for PR Validation, merges, and only then
  waits for Validation and Compose on the merge SHA.
- The repair routine can repair an open Codex PR from failed Actions evidence,
  but a post-merge failure has no open branch left to repair.
- Compose run `32538701324` on merge `6dd4838c` demonstrated the gap: both
  gateway API lanes passed while three browser/admin smoke lanes failed on
  runtime-start or Playwright timeouts. The loop correctly stopped, but could
  not route the failure into repair because PR `#272` was already merged.
- The existing workflow has ref-scoped concurrency, so a manual branch dispatch
  is isolated from the later `main` run.

## Scope

- In `AUTO_MERGE=1` mode, dispatch `compose.yml` for the exact current PR branch
  head after ordinary PR checks pass and before merge.
- Adopt an existing queued, running, or successful run for that exact SHA rather
  than dispatching a duplicate.
- Reject stale evidence after any repair, branch update, or external head
  movement.
- Bind the GitHub merge request to the tested head SHA so a last-moment head
  change fails instead of merging unchecked content.
- Route failure, cancellation exhaustion, dispatch failure, and timeout to the
  existing bounded repair decision while the PR remains open.
- Include the exact Compose run and failed-job snapshot in repair context.
- Retain post-merge exact-SHA Validation, Compose, and conditional Rust-builder
  checks as defense-in-depth before the next issue.
- Add isolated shell contract tests and align operator/current-context docs.

## Non-Goals

- Adding full Compose as an automatic trigger for every human pull request.
- Removing the post-merge publication checks or weakening branch protection.
- Automatically changing already-published `main` in response to an unrelated
  scheduled failure.
- Changing product runtime behavior or fixing unrelated smoke flakiness in this
  contributor-tooling slice.

## Decisions And Dependencies

- Use the existing `workflow_dispatch` entry point instead of a new workflow or
  a global pull-request trigger. This contains runner cost to opt-in automatic
  delivery while still testing the exact candidate commit.
- Compose success is SHA-bound, not branch-name-bound. A changed head invalidates
  all prior evidence.
- A failed exact-head Compose run is PR evidence, unlike the post-merge run, so
  the existing repair routine may inspect and repair it.
- The main run remains mandatory because the merge commit can differ from the PR
  head and because it verifies the published branch environment.

## Contract Changes

- API/OpenAPI: N/A; no product endpoint changes.
- Protocol/event schemas: N/A; no BrowserPane wire change.
- Database/migrations: N/A.
- Admin-new: N/A.
- CLI/SDK: N/A; this changes only the optional contributor loop.
- Deployment/configuration: add loop configuration for the pre-merge Compose
  workflow and bounded wait; the hosted Compose workflow itself is unchanged.
- README/ARCH/AGENTS/operator docs: update `dev_loop/README.md` and
  `docs/CURRENT_CONTEXT.md`. Root README, ARCH, and AGENTS do not need changes
  because product architecture, setup, and the high-level loop guardrail remain
  unchanged.

## Security And Data Impact

The driver uses the already-approved project GitHub identity to dispatch and
inspect Actions. No new credentials, permissions, logs, product data, or
multi-tenant boundaries are introduced. Repair context contains run URLs, job
names, conclusions, and commit SHAs only; it must not embed unredacted Compose
logs or artifacts. Existing diagnostic redaction and local run-data handling
remain authoritative.

## Migration, Compatibility, And Rollback

The behavior is additive and only active with `AUTO_MERGE=1`. Supervised mode
is unchanged. Existing manually dispatched exact-head runs are reusable. A
rollback removes the pre-merge gate and its configuration/tests/docs, returning
to post-merge-only Compose evidence; no data migration or workflow change is
required. If dispatch is unavailable, the loop fails closed and leaves the PR
open for manual recovery.

## Observability And Operator Feedback

- Console output distinguishes dispatch, adoption, run state, exact SHA, and
  failure/timeout outcomes.
- Repair context identifies the exact run URL and failed jobs.
- The run journal records `pre-merge-compose-failed` only if bounded repair is
  exhausted or halted; successful repair remains part of the PR's repair count.
- Post-merge failures retain their existing explicit stop outcome.

## Implementation Slices

1. Add a generic exact-commit workflow waiter and pre-merge Compose dispatcher
   with duplicate-run adoption and bounded cancellation handling.
2. Wire the gate between green/current PR verification and merge; recheck the
   head and base after Compose and route failures to repair.
3. Extend repair context, shell tests, and operator/current-context docs.
4. Run shell syntax, isolated loop tests, repository document validation, and a
   real branch workflow dispatch before requesting merge.

## Test Strategy

### Unit

- Parse successful, failed, cancelled, and pending exact-SHA workflow snapshots.
- Adopt an existing exact-head run without dispatch.
- Dispatch once when no exact-head run exists.
- Reject dispatch failure and bounded timeout.
- Verify changed-head evidence is not accepted by the merge path.
- Validate new boolean and timeout configuration.

### Integration

- Source the production driver in library mode with mocked `gh` boundaries.
- Verify supervised mode remains independent of the pre-merge dispatcher.
- Verify branch Compose failure produces repair context with the exact run and
  failed jobs.
- Preserve existing merge-gate, post-merge workflow, identity, disk, and routine
  result tests.

### Smoke And E2E

- Manually dispatch the real Compose workflow on this branch and wait for all
  five lanes.
- On a disposable Codex PR, confirm restart adopts an in-progress exact-head
  run rather than creating a duplicate.
- Confirm a new commit requires a new Compose dispatch.

### Coverage And Quality

Run `bash -n dev_loop/loop.sh dev_loop/tests/loop_test.sh`, the complete loop
contract suite, and repository document validation. Rust, browser-client,
Admin-New, API, OpenAPI, and product coverage are N/A because this slice changes
only Bash contributor orchestration and Markdown documentation.

## Manual Test Sequence

1. Start from clean synchronized `main` with the approved project identity.
2. Run `./dev_loop/loop.sh --check` and `./dev_loop/tests/loop_test.sh`.
3. Adopt a disposable green Codex PR with `AUTO_MERGE=0`; verify it stops for
   review and does not dispatch Compose.
4. Restart with `AUTO_MERGE=1`; verify one Compose run targets the PR head SHA.
5. Restart while that run is active; verify the same run is adopted.
6. Force a disposable smoke failure; verify the PR remains open and repair gets
   the Compose run evidence.
7. Push the repair; verify both Validation and a fresh Compose run pass for the
   new head before merge.
8. Verify the loop then waits for the exact merge-SHA publication workflows.

## Documentation And Claim Impact

This is contributor delivery tooling and does not alter BrowserPane capability,
security, maturity, or investor claims. Keep `dev_loop/README.md`, this plan,
and the current-context handoff aligned. No README, ARCH, OpenAPI, capability
matrix, risk-register, or investor repository update is required.

## Definition Of Done

- `AUTO_MERGE=1` requires successful exact-head Compose evidence before merge.
- Existing exact-head runs are adopted without duplicate dispatch.
- Changed heads invalidate old Compose evidence.
- Failures enter bounded repair with actionable run metadata.
- Supervised mode and post-merge publication checks remain intact.
- Shell tests, syntax, document policy, and a real branch Compose run pass.
- Issue, plan, PR, and evidence are synchronized.

## Post-Implementation Smoke Sequence

1. Run `bash -n dev_loop/loop.sh dev_loop/tests/loop_test.sh`.
2. Run `./dev_loop/tests/loop_test.sh`.
3. Run `node scripts/check-repository-documents.mjs`.
4. Dispatch `Compose` against this branch and verify the run head SHA matches
   the branch tip.
5. Verify all Compose jobs pass or repair the open PR before merge.
6. After merge, verify Validation and Compose pass on the exact merge SHA.

## Evidence Record

Pending implementation, PR, exact test output, and branch/main workflow links.
