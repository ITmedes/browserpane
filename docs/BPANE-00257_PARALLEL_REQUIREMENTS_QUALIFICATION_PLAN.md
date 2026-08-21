# BPANE-00257 Parallel Requirements Qualification Plan

## Metadata

- Issue: `#257`
- State: In Progress
- Owner: BrowserPane maintainers / Codex
- Lane: Foundation
- Target gate: Repeatable, bounded contributor delivery
- Depends on: `#241` and `#246` (complete through PRs #243 and #247)
- Last verified commit/date: `07a2ff44` / 2026-08-21

## Business Outcome

Keep the bounded Codex delivery loop productive when the first roadmap item is
waiting for a real stakeholder or external approval, without allowing the loop
to invent that decision or silently reorder the backlog.

The qualifier may consider a later issue only when canonical roadmap material
explicitly identifies it as parallel or as the fallback after the deferred
item. Requirements specification may create a missing focused plan only when
the live issue and repository evidence already establish a coherent bounded
direction.

## Example Use Case

Issue `#174` owns selection and delivery of a real Phase 0 browser activity.
The repository cannot name the real process owner, approve a deployment, or
accept its data and threat profile. Issue `#180` is independently documented as
a parallel Phase 0 governance gate.

Qualification records why `#174` is deferred, does not mutate or promote it,
and evaluates `#180`. If `#180` has no focused plan but its issue, root license,
package metadata, and canonical governance documents are sufficient to define
one without claiming legal approval, the qualifier returns
`NEEDS_SPECIFICATION`. A fresh specification session creates the one focused
plan and opens a documentation PR. A later iteration reassesses `#180` before
implementation.

## Current Evidence

- `dev_loop/routines/qualify.md` requires the first documented candidate to
  stop on any unresolved target selection and requires an existing focused
  plan before specification can be selected.
- `dev_loop/routines/specify.md` requires exactly one pre-existing focused
  plan and therefore cannot repair that omission.
- `dev_loop/loop.sh` already supports `NEEDS_SPECIFICATION`, a fresh
  documentation-only session, bounded specification cycles, PR convergence,
  and later requalification.
- Run `dev_loop/runs/20260821-155636` safely returned
  `NO_QUALIFICATION` because `#174` lacks a real selected process and external
  decisions. The result also identified `#180` as a separate legal-governance
  gate but did not evaluate it as documented parallel work.
- `docs/CURRENT_CONTEXT.md`, `docs/DELIVERY_ROADMAP.md`, and
  `docs/IMPLEMENTATION_WORK_ORDER.md` explicitly describe `#180` as parallel to
  `#174`.

## Scope

- Refine qualification selection policy to defer, rather than infer, a real
  external decision and evaluate only an explicitly documented parallel or
  fallback candidate.
- Require the qualification reason and summary to retain the deferred issue
  and blocker when another candidate is selected.
- Let qualification return `NEEDS_SPECIFICATION` for a missing focused plan
  when exactly one issue owns the slice and current evidence can define the
  complete plan without unresolved external policy.
- Let the specification routine create exactly one correctly named focused
  plan for that selected issue.
- Add isolated contract tests for the new prompt boundaries and align the
  contributor loop documentation.

## Non-Goals

- Selecting the real Phase 0 activity or inventing stakeholder identities,
  legal approval, production acceptance, secrets, or target-system access.
- Automatically relabeling a deferred issue as `state:blocked`.
- Selecting arbitrary lower-priority work because it is easier.
- Changing the structured-result schema or shell state machine.
- Implementing product, runtime, API, Admin-New, CLI, or deployment behavior.

## Decisions And Dependencies

- Roadmap order remains authoritative. A later candidate is eligible only when
  the canonical documents explicitly say it is parallel or fallback work.
- A deferred candidate remains Qualified and unchanged; the qualifier records
  the blocker but does not own issue lifecycle correction.
- Missing-plan specification is allowed only when the plan itself is the
  evidence-backed omission. It is not a mechanism for choosing unresolved
  legal, security, commercial, or real-world deployment policy.
- Conservative internal defaults may be documented only when already supported
  by repository evidence and clearly separated from external acceptance.
- The existing specification cycle, merge controls, and later requalification
  remain the execution mechanism.

## Contract Changes

- API/OpenAPI: N/A; contributor tooling only.
- Protocol/event schemas: N/A; no product wire contract changes.
- Database/migrations: N/A.
- Admin-new: N/A.
- CLI/SDK: N/A; `dev_loop/loop.sh` command and environment contract remain
  unchanged.
- Deployment/configuration: N/A.
- README/ARCH/AGENTS/operator docs: update only `dev_loop/README.md`; product
  README, architecture, and operator behavior are unchanged.

## Security And Data Impact

- External stakeholder identities, target credentials, customer data,
  production deployment acceptance, and legal approval remain non-inferable.
- A security or data decision may be documented only when current evidence
  already fixes it; otherwise the candidate remains deferred.
- The specification routine remains documentation-only and retains the
  project-scoped GitHub identity, clean-tree, single-PR, and secret-redaction
  controls.
- Parallel selection must not weaken a gate attached to the deferred issue.

## Migration, Compatibility, And Rollback

This is a prompt-policy extension. Existing Ready-first behavior, result
statuses, shell routing, lifecycle verification, cycle budgets, and merge
controls remain compatible.

Rollback restores the former qualifier/specifier text and tests. Open issues,
PRs, and run logs remain untouched; no data migration or product rollback is
required.

## Observability And Operator Feedback

- Qualification reasons and summaries identify both the deferred issue and the
  explicit parallel/fallback evidence.
- Existing local prompt, JSONL, result, stderr, journal, token, and duration
  records remain unchanged.
- A queue with no documented parallel/fallback continues to report
  `NO_QUALIFICATION` with the external blocker.

## Implementation Slices

1. Add this focused plan and synchronize issue `#257`.
2. Refine qualification and specification routine contracts.
3. Add semantic prompt-contract tests and contributor documentation.
4. Run focused shell and repository-document validation.

## Test Strategy

### Unit

- Assert qualification permits only explicitly documented parallel/fallback
  selection after retaining the deferred blocker.
- Assert qualification can classify a missing focused plan as
  `NEEDS_SPECIFICATION` only with sufficient evidence.
- Assert specification may create exactly one focused plan in that case.
- Assert legal approval, external stakeholder identity, production acceptance,
  and target access remain forbidden inferences.

### Integration

- Existing mocked qualification/specification routing tests remain green.
- Static routine-contract checks bind the prose policy used by actual Codex
  sessions to the intended safety boundary.

### Smoke And E2E

- Run one supervised loop iteration after merge with `#174` externally gated
  and `#180` explicitly parallel.
- Verify the qualifier evaluates `#180` without mutating `#174`.
- Verify a missing `#180` focused plan routes to one documentation-only
  specification PR.
- Verify a fixture with no explicit parallel/fallback still stops safely.

### Coverage And Quality

- Run `bash -n dev_loop/loop.sh dev_loop/tests/loop_test.sh`.
- Run `./dev_loop/tests/loop_test.sh`.
- Run `node scripts/validate.mjs --stage repository-baseline --stage
  repository-documents`.
- No Rust, TypeScript product, Compose, Admin-New, or API coverage is affected.

## Manual Test Sequence

1. Start from clean synchronized `main` with the approved project GitHub
   identity and no Ready issue.
2. Confirm `#174` remains Qualified and its plan records the missing real
   candidate decision.
3. Confirm canonical roadmap documents identify `#180` as parallel work.
4. Run `ITERATIONS=1 AUTO_QUALIFY=1 AUTO_MERGE=0 ./dev_loop/loop.sh`.
5. Verify `#174` is neither edited nor promoted.
6. Verify qualification evaluates `#180` and returns
   `NEEDS_SPECIFICATION` when only its focused plan is missing.
7. Verify a fresh session creates one `docs/BPANE-00180_*_PLAN.md`, updates
   only directly related requirements, preserves `state:qualified`, and opens
   one documentation PR.
8. Verify the loop watches the PR and stops at `green-awaiting-review`.
9. Remove the documented parallel relationship in an isolated fixture and
   verify qualification returns `NO_QUALIFICATION` without mutation.

## Documentation And Claim Impact

Update contributor-loop documentation only. This does not advance BrowserPane
capability maturity, complete `#174` or `#180`, or justify Pilot, legal,
security, production, API, Admin-New, or investor claims. `README.md`,
`ARCH.md`, OpenAPI, and product documentation do not need changes.

## Definition Of Done

- Issue `#257` and this plan agree on scope, safety boundaries, tests, and
  smoke evidence.
- The qualifier can defer a real external decision and evaluate only explicit
  parallel/fallback work.
- Missing focused plans can enter bounded specification when evidence is
  sufficient.
- External decisions remain fail-closed.
- Existing loop tests and repository document validation pass.

## Post-Implementation Smoke Sequence

1. Run shell syntax, loop contract tests, and repository document validation.
2. Run one supervised iteration with `#174` gated and `#180` documented in
   parallel.
3. Confirm `#174` remains untouched and its blocker is retained in the result
   evidence.
4. Confirm `#180` routes to one missing-plan specification PR.
5. Confirm the PR is documentation-only and `#180` stays Qualified.
6. Confirm an otherwise identical queue without an explicit parallel/fallback
   path stops without mutation.

## Evidence Record

- Issue: https://github.com/ITmedes/browserpane/issues/257
- `bash -n dev_loop/loop.sh dev_loop/tests/loop_test.sh` passed.
- `./dev_loop/tests/loop_test.sh` passed all 96 contract and driver tests.
- `node scripts/validate.mjs --stage repository-baseline --stage
  repository-documents` passed both stages.
- PR, commit, and post-merge supervised qualifier smoke: pending.
