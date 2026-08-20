# BPANE-00246 Requirements Specification Loop Plan

## Metadata

- Issue: `#246`
- State: Review
- Owner: BrowserPane maintainers / Codex
- Lane: Foundation
- Target gate: Repeatable, bounded contributor delivery
- Depends on: `#241` Codex delivery loop (complete through PR #243)
- Last verified commit/date: `51101a34` / 2026-08-20

## Business Outcome

BrowserPane's local Codex loop can resolve evidence-backed requirements gaps
without returning control to one long-lived interactive session. Qualification,
specification, and implementation remain separate auditable responsibilities:
the qualifier identifies one coherent gap set, a fresh session prepares one
reviewable requirements PR, and only a later qualification pass may authorize
product implementation.

## Example Use Case

The Ready queue is empty and `#172` is the documented next Qualified Pilot
Value candidate. Its core direction is decided, but its issue and focused plan
do not explicitly define migration, rollback, deployment, protocol/SDK impact,
regression coverage, or the boundary to generalized RBAC in `#176`.

The qualifier returns `NEEDS_SPECIFICATION` for `#172` with those concrete
gaps. The driver starts a fresh specification session, which reconciles the
live issue and `docs/BPANE-00172_PHASE_0_WORKFLOW_ENDPOINT_PLAN.md` against
current code and canonical requirements. It opens one documentation PR and
exits. After that PR is reviewed and merged, a later iteration reruns
qualification. Product implementation starts only if the reassessed issue is
then promoted to Ready.

If the same audit instead finds an undecided authorization model, unresolved
legal constraint, ambiguous roadmap order, or conflicting owner issue, the
loop stops without inventing a decision or creating a PR.

## Scope

- Add `NEEDS_SPECIFICATION` and `SPECIFIED` routine-result contracts.
- Teach qualification to distinguish resolvable specification omissions from
  unresolved decisions and unsafe state.
- Add a specification routine limited to one selected Qualified issue, its
  focused plan, and directly related requirements or roadmap documentation.
- Bound repeated requirements PRs with a validated per-run specification-cycle
  budget; exhaustion requires maintainer review.
- Reuse the existing `codex/BPANE-*` PR mutex, CI watcher, repair budget,
  branch-currency check, supervised review default, and opt-in merge path.
- Prevent a specification session from promoting the issue or implementing
  product/runtime code.
- Rerun qualification only in a later iteration after the specification PR has
  merged and required published-main checks have passed.
- Keep contributor guidance, delivery context, and the `#172` reference case
  aligned with the implemented loop.

## Non-Goals

- Automatic product, security, architecture, legal, or commercial decisions.
- Product code, runtime configuration, database migrations, API schemas, or UI
  implementation inside a specification session.
- Creating, splitting, merging, closing, or reprioritizing product issues.
- Promoting an issue to Ready before its specification PR is merged and
  reassessed.
- Running specification and product proposal in the same Codex session or the
  same iteration.
- More than one specification PR per iteration.
- Automatic merge by default or bypassing branch protection and review.

## Decisions And State Machine

### Qualification Outcomes

- `QUALIFIED`: the selected issue already passes the readiness contract; the
  qualifier comments and promotes it to Ready.
- `NEEDS_SPECIFICATION`: one live Qualified candidate is correctly ordered and
  has a decided, bounded direction, but issue/plan omissions can be resolved
  from repository, issue, dependency, and canonical-document evidence.
- `NO_QUALIFICATION`: no candidate can safely progress because ordering,
  dependencies, ownership, or required product decisions remain unresolved.
- `HALT`: repository, identity, concurrency, or mutation safety is compromised.

`NEEDS_SPECIFICATION` includes exactly one issue number and a non-empty,
actionable gap list. Qualification remains repository-read-only and does not
change issue labels for this outcome.

### Specification Outcome

A fresh specification session receives the issue number and qualification
reason. It may update the canonical issue body/comments and focused planning
documents needed to close those gaps. It returns:

- `SPECIFIED` with issue number, ready PR URL, and commit SHA after exactly one
  `codex/BPANE-*` documentation PR is open; or
- `HALT` if evidence is insufficient, a real decision is required, scope would
  enter product code, or safe issue/document reconciliation is impossible.

The session must preserve `state:qualified`, must not add `state:ready`, and
must not wait for CI or merge. The shell driver verifies the selected issue
came from the original Qualified queue and adopts exactly one matching Codex
PR for its existing convergence flow.

### Iteration Boundary

One iteration performs either specification convergence or product proposal
convergence, never both. With `AUTO_MERGE=0`, a green specification PR remains
open for review. With `AUTO_MERGE=1`, the driver may merge it after the normal
checks and branch-currency verification, then validates the required workflows
on the merge SHA. The next iteration starts from synchronized `main` and reruns
qualification from first principles.

The driver allows at most `MAX_SPECIFICATION_CYCLES` specification sessions in
one run (default `3`). Exhaustion stops the run with durable evidence instead
of repeatedly rewriting the same contract. `AUTO_QUALIFY=0` remains the way to
disable automatic qualification and specification entirely.

## Contract Changes

- API/OpenAPI: N/A; contributor tooling only.
- BrowserPane protocol and event schemas: N/A.
- Database/migrations: N/A.
- Admin-New and product CLI/SDK: N/A.
- Local contributor schema: add `NEEDS_SPECIFICATION` and `SPECIFIED` to
  `dev_loop/schemas/routine-result.schema.json` and its stricter shell
  validator.
- Local contributor command: `dev_loop/loop.sh` gains a bounded specification
  phase but retains its existing command-line interface.
- Local configuration: `MAX_SPECIFICATION_CYCLES` is a required positive
  integer with default `3`.
- Runtime deployment/configuration: N/A. Existing local Codex, GitHub, Git, and
  disk-space prerequisites apply.

## Security And Data Impact

- The specification session has repository and GitHub write access under the
  same explicit project-identity allowlist as proposal/repair sessions.
- It must not inspect or publish local tokens, secrets, private run logs, or
  credential values.
- It may not weaken acceptance criteria, security boundaries, test gates, or
  issue ownership merely to obtain Ready status.
- Issue mutations must be summarized and linked to the specification PR so the
  temporary issue/document synchronization window is reviewable.
- Generated prompts and JSONL remain local, ignored, and potentially sensitive.

## Migration, Compatibility, And Rollback

The change is additive to the local contributor loop. Existing Ready-first
proposal behavior, manual `AUTO_QUALIFY=0` mode, open-PR adoption, repairs,
STOP handling, and merge policy remain compatible. Existing result statuses
retain their meaning.

Rollback removes the specification routine and statuses and restores
qualification's previous stop behavior. Open specification PRs and issue
history remain visible for manual review; the loop must never reset or delete
them during rollback.

## Failure Behavior And Observability

- Invalid or incomplete `NEEDS_SPECIFICATION`/`SPECIFIED` results fail closed.
- A reported issue outside the pre-session Qualified queue is rejected.
- Missing or multiple Codex PRs after `SPECIFIED` are rejected.
- Exhausting the per-run specification-cycle budget stops before another Codex
  session is launched.
- A specification session that changes lifecycle labels, creates product code,
  or cannot provide a coherent issue/plan contract returns `HALT`.
- The console and journal distinguish qualification gaps, specification
  failure, specification PR convergence, green review wait, and later
  qualification.
- Prompts, JSONL, stderr, final result, usage, duration, and PR number use the
  existing local run log contract.

## Implementation Steps

1. Extend the structured result schema and strict shell validator with
   `NEEDS_SPECIFICATION` and `SPECIFIED` invariants.
2. Refine the qualification routine so evidence-backed document omissions
   route to specification while unresolved decisions still stop.
3. Add a dedicated specification routine and driver context containing the
   selected issue, exact gap reason, live issue/plan evidence, and mutation
   boundaries.
4. Route specification PRs through the existing CI, repair, branch-currency,
   review, optional merge, and post-merge workflow flow without starting a
   proposal in the same iteration.
5. Add isolated shell tests for schemas, routing, context, candidate checks,
   and invalid outcomes.
6. Align `dev_loop/README.md`, `AGENTS.md`, current context, roadmap, issue
   inventory, the completed `#241` plan, and the `#172` reference plan.
7. Reconcile the live `#172` issue with its focused plan while preserving its
   Qualified state, then validate the resulting documentation and loop.

## Test Strategy

### Unit

- Accept valid `NEEDS_SPECIFICATION` and `SPECIFIED` objects.
- Reject missing issue, reason, PR URL, commit SHA, or forbidden fields for the
  new statuses.
- Preserve validation of every existing status.
- Verify qualification gate return codes and retained issue/reason values.
- Verify specification context construction under `set -u`.
- Reject zero or malformed specification-cycle budgets.

### Integration

- Mock a zero-Ready queue, a live Qualified candidate, and a qualifier result
  requiring specification; assert the driver routes to specification rather
  than stopping or proposing.
- Mock a successful specification result and exactly one Codex PR; assert the
  PR can be adopted by the normal watcher.
- Reject a result for an issue not present in the original Qualified queue.
- Confirm `AUTO_QUALIFY=0` continues to bypass both qualification and
  specification.

### Smoke And E2E

- Run one supervised iteration against `#172` after this contributor change
  lands, with `AUTO_MERGE=0`, and inspect the generated specification PR.
- Merge that PR manually, rerun one supervised iteration, and confirm the issue
  is reassessed before implementation.
- Exercise an intentionally unresolved decision fixture and confirm no issue,
  branch, PR, or label mutation.

### Quality Gates

- `bash -n dev_loop/loop.sh dev_loop/tests/loop_test.sh`
- `./dev_loop/tests/loop_test.sh`
- `node scripts/validate.mjs --stage repository-baseline --stage repository-documents`
- Review shell quoting, `set -euo pipefail`, bounded waits, clean-tree
  enforcement, GitHub identity checks, and non-destructive Git behavior.

## Post-Implementation Smoke Sequence

1. Start from clean synchronized `main` with project-scoped `gh` identity.
2. Run `./dev_loop/loop.sh --check` and the shell/document validation commands.
3. Confirm there is no Ready issue and `#172` remains Qualified.
4. Run `ITERATIONS=1 AUTO_QUALIFY=1 AUTO_MERGE=0 ./dev_loop/loop.sh`.
5. Confirm the qualifier returns `NEEDS_SPECIFICATION` with one issue and a
   concrete gap list, without label or Git mutation.
6. Confirm a fresh session updates only `#172` and directly related docs and
   opens exactly one ready `codex/BPANE-*` PR.
7. Confirm the driver watches checks and stops at `green-awaiting-review`.
8. Merge the PR, synchronize `main`, and rerun one iteration.
9. Confirm qualification reassesses the merged contract and only then may
   promote `#172`; implementation must occur in a separate proposal session.
10. Repeat with an unresolved-decision candidate and confirm a safe stop with
    no repository, issue, label, branch, or PR mutation.

## Documentation And Claim Impact

Update contributor guidance and delivery context only. This slice does not add
a BrowserPane product capability, advance a product gate, or justify runtime,
security, BPM, or production-readiness claims. `README.md`, `ARCH.md`, OpenAPI,
Admin-New documentation, and investor material require no change.

## Implementation Evidence

- `bash -n dev_loop/loop.sh dev_loop/tests/loop_test.sh` passed.
- `dev_loop/tests/loop_test.sh` passed all 66 isolated contract, routing,
  lifecycle, budget, identity, disk, CI-state, and lock tests.
- `node scripts/validate.mjs --stage repository-baseline --stage
  repository-documents` passed both stages.
- `direnv exec . bash -c 'source dev_loop/loop.sh; preflight_tools'` passed
  with the approved `thebackplane` identity and `ADMIN` repository permission.
- The live `#172` issue and focused plan were reconciled and verified to retain
  only `state:qualified` among lifecycle labels.
- The supervised real-loop smoke remains post-merge evidence because the
  current loop on `main` does not yet contain the specification phase.

## Definition Of Done

- Issue `#246` and this plan agree on business case, boundaries, acceptance,
  failure behavior, test evidence, and smoke sequence.
- Qualification can route one resolvable gap set to specification without
  treating it as Ready or implementing it.
- Specification produces at most one reviewable requirements PR and cannot
  mutate product code or lifecycle labels.
- Existing proposal/repair behavior remains green.
- A later iteration always reassesses the merged specification before product
  implementation.
- The `#172` issue and plan form a coherent, explicit reference contract while
  remaining Qualified until the normal qualifier promotes it.
