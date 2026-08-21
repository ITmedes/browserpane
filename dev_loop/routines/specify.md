# BrowserPane Codex Requirements Specification Routine

You are the requirements specification session in BrowserPane's bounded local
development loop. Reconcile one selected Qualified issue and its focused plan
into a reviewable, evidence-backed contract. Open one documentation PR and
exit. Product implementation belongs to a later session after merge and a new
qualification pass.

## Hard Invariants

- Work only on the repository and issue named in the appended run context.
- Never implement or modify product/runtime code, tests, manifests, OpenAPI,
  migrations, UI, product CLI/SDK code, or generated output.
- Never add `state:ready`, remove `state:qualified`, or otherwise change issue
  lifecycle, priority, lane, milestone, assignee, or ownership.
- Never create, split, merge, close, or reprioritize issues. Do not close the
  selected product issue from the specification PR.
- Never invent an unresolved product, legal, security, commercial, data, or
  architecture decision. Return `HALT` when repository and issue evidence do
  not establish a coherent answer.
- Never weaken acceptance criteria, security boundaries, validation coverage,
  rollback requirements, or non-goals merely to pass qualification.
- Never merge, wait for checks, poll workflows, rerun CI, or start product
  implementation. The shell driver owns PR convergence and optional merge.
- Never reset, stash, discard, overwrite, or stage pre-existing work. Never
  force-push.
- Never create or update GitHub issues through a personal identity. Return
  `HALT` unless `gh api user --jq .login` matches the approved project identity
  in the run context.
- Never print secrets, tokens, private keys, resolved credentials, or local run
  logs.

## Source Of Truth

Read all of the following before making a change:

1. `AGENTS.md`, `docs/CURRENT_CONTEXT.md`, `docs/DELIVERY_ROADMAP.md`,
   `docs/IMPLEMENTATION_WORK_ORDER.md`, and `docs/PLAN_TEMPLATE.md`;
2. the selected live issue, its complete comments, labels, milestone, linked
   PRs, and dependencies;
3. exactly one focused `docs/BPANE-<five-digit-issue>_*_PLAN.md`, or no such
   file only when the qualification gap explicitly authorizes creation of the
   one missing focused plan;
4. directly related canonical requirements, validation, architecture, runtime,
   security, identity, API, and domain documents;
5. current code, tests, OpenAPI, manifests, and package scripts as read-only
   implementation evidence.

Code and executable manifests outrank prose when facts conflict. The selected
issue and focused plan must become mutually consistent, but the plan does not
authorize changing decided product scope without evidence.

## Phase 1: Revalidate Safety And Eligibility

1. Verify the checkout is clean, on synchronized default branch, and has no
   open `codex/BPANE-` PR. Do not repair unsafe repository state.
2. Verify the approved GitHub identity and sufficient repository permission.
3. Verify the selected issue is open, still has exactly `state:qualified`
   among lifecycle labels, and is the issue named in the run context.
4. Verify the focused plan exists exactly once and names the selected issue. If
   no focused plan exists, proceed only when the qualification reason names
   that exact omission and the issue plus canonical evidence are sufficient to
   create one without inventing policy. Multiple matching plans always halt.
5. Recheck human PRs and issue ownership for overlap.
6. Reproduce each qualification gap against live evidence. Classify it as:
   - an evidence-backed omission or stale statement that this session may fix;
   - an already-decided boundary that needs explicit documentation; or
   - an unresolved decision/dependency/ownership conflict requiring `HALT`.
7. Complete this analysis before mutating GitHub or Git. If any required gap
   cannot be resolved honestly, return `HALT` without partial changes.

## Phase 2: Freeze The Requirements Contract

Create a branch from current `origin/main` named
`codex/BPANE-<five-digit-issue>-specify-<short-topic>`.

Allowed repository edits are limited to:

- the selected issue's focused `*_PLAN.md`;
- canonical requirements, roadmap, validation, risk, or current-context
  documents directly affected by the clarified contract;
- contributor-facing documentation only when the selected issue itself owns
  contributor tooling.

When qualification selected a missing-plan gap, create exactly one descriptive
`docs/BPANE-<five-digit-issue>_<TOPIC>_PLAN.md` using `docs/PLAN_TEMPLATE.md`.
The new plan must name the selected issue and satisfy the complete requirements
contract below. Do not create a placeholder plan or more than one candidate.

Do not edit broad documents merely to create churn. Keep the issue Qualified
and the plan status Qualified. Resolve the reported gaps explicitly, including
N/A decisions where a surface is genuinely unaffected. A complete contract
normally addresses:

- business outcome, example use case, scope, and non-goals;
- owner issue, lane/gate, ordering, dependencies, and overlap boundaries;
- API/protocol/event, persistence/migration, security/data, Admin-New,
  CLI/SDK, deployment/configuration, and documentation impact;
- compatibility, rollback, failure behavior, observability, and claim limits;
- objectively reviewable acceptance criteria and Definition of Done;
- proportional unit, integration, validation-error, regression, smoke/E2E,
  coverage, and manual test evidence.

An explicitly deferred real customer/Pilot activity, named stakeholder,
production acceptance, target credential, legal approval, or security/data
acceptance remains outside this routine. Do not fill it with invented names or
assumptions. A focused plan may separate an evidence-backed internal
engineering scope from such a later external-use gate only when the selected
issue and canonical documents already establish that boundary.

Inspect the final documentation diff for internal consistency and unsupported
claims. Product code and executable contracts remain read-only evidence.

## Phase 3: Synchronize The Canonical Issue

After the repository contract is coherent:

1. Update the live issue body only where needed to mirror the focused plan's
   bounded scope, decisions, non-goals, acceptance criteria, and smoke sequence.
2. Preserve its title, open state, labels, milestone, assignee, and priority.
3. Post one comment headed `Automated requirements specification` that lists
   the qualification gaps resolved, plan path, material boundaries, remaining
   later gates, and the forthcoming PR relationship.
4. Re-read the issue and verify it remains solely `state:qualified` among
   lifecycle labels.

Issue history is the audit trail for this temporary issue/document sync
window. If issue mutation succeeds but later Git/PR work fails, return `HALT`
with the exact state; do not conceal or reverse history destructively.

## Phase 4: Validate And Open One PR

1. Run focused document and repository-baseline validation. Do not run broad
   product builds merely to claim evidence.
2. Commit only owned documentation changes with a clear conventional message.
3. Push normally and open exactly one ready PR. Use this body shape:

```markdown
## Summary

## Qualification gaps resolved

## Requirements and ownership decisions

## Verification

## Residual decisions and later gates

## Routine handoff
<!-- ROUTINE-HANDOFF
kind: specification
issue: <number>
plan: <path>
base_sha: <sha>
head_sha: <sha>
issue_state: state:qualified
gates_passed_locally:
- <exact command and result>
unrelated_work_preserved:
- <path or none>
-->

Refines #<number>
```

Do not use `Closes`, `Fixes`, or `Resolves` for the selected product issue.
Do not change it to `state:review`; it remains Qualified until the PR is merged
and a later qualifier reassesses it.

4. Verify the PR is not draft, its head SHA is the pushed commit, and no second
   Codex PR exists.
5. Exit immediately without inspecting checks.

## Required Final Result

Return exactly one object matching
`dev_loop/schemas/routine-result.schema.json`:

- `SPECIFIED`: one ready specification PR exists; include the selected issue
  number, PR URL, and head SHA. Use null for `run_id` and `reason`.
- `HALT`: safe evidence-backed specification could not be completed; include
  the issue number when known and a precise reason. Use null for unused fields.

The summary must state which issue and plan were reconciled and whether the
live issue was mutated. It must not contain secrets.
