# BPANE-00260 Ordered Qualification Fallback Plan

## Metadata

- Issue: `#260`
- State: In Progress
- Owner: BrowserPane maintainers
- Lane: Foundation contributor tooling
- Target gate: reliable bounded development-loop execution
- Depends on: `#257` external-decision deferral contract
- Last verified commit/date: `5f022af7e374` / 2026-08-21

## Business Outcome

The Codex development loop continues useful requirements and implementation
work when the highest-priority Pilot or governance candidates are waiting for
real stakeholder decisions. It follows a finite maintainer-owned order instead
of stopping too early or choosing arbitrary backlog work.

## Example Use Case

Issue `#174` cannot begin until a real Pilot activity and accountable owners are
selected. Issue `#180` cannot implement a license posture until a legal/business
reviewer records the decision. The roadmap therefore permits the loop to inspect
`#175` next. Its accepted protocol ADR, Rust wire types, TypeScript client, and
existing interoperability tests provide enough evidence for a separate
requirements session to create the missing focused plan without inventing the
Pilot or license decisions.

## Current Evidence

- PR `#258` taught qualification to defer one externally blocked candidate and
  inspect an explicitly documented parallel candidate.
- PR `#259` created the focused `#180` governance plan while preserving its
  external approval gate.
- Run `dev_loop/runs/20260821-184923` then stopped because both `#174` and `#180`
  were externally gated and no later fallback was explicitly authorized.
- Issue `#175` has an accepted product direction in ADR 0003, a concrete live
  issue contract, and no focused `docs/BPANE-00175_*_PLAN.md`.
- Issue `#124` is a bounded Admin-New catalog gap and is suitable only after
  `#175` is blocked, exhausted, or completed.

## Scope

1. Define the finite ordered fallback queue
   `#174 -> #180 -> #175 -> #124` in canonical execution documents.
2. Require qualification to inspect the queue in order and retain every
   deferred issue and blocker in its result evidence.
3. Permit the first evidence-backed candidate with a missing focused plan to
   return `NEEDS_SPECIFICATION`.
4. Return `NO_QUALIFICATION` only when every documented candidate is blocked or
   no longer eligible.
5. Add prompt-contract regression tests and synchronize loop documentation.

## Non-Goals

- Inferring the Pilot selection or open-source legal decision.
- Automatically reprioritizing labels or issues outside the documented queue.
- Making `#175` or `#124` a Phase 0 external-Pilot dependency.
- Implementing protocol conformance or session-template UI behavior in this
  contributor-tooling slice.
- Generalizing the qualifier into an unbounded backlog scheduler.

## Decisions And Dependencies

- The canonical roadmap, not issue labels, owns fallback eligibility and order.
- `#174` and `#180` remain visible and Qualified while their external decisions
  are missing.
- `#175` is first because the custom remote protocol is a strategic Production
  contract and ADR 0003 already accepts the direction.
- `#124` follows as a bounded Operator Product gap with established Admin-New
  patterns and no external target decision.
- A future queue change requires a reviewed canonical-doc update; the qualifier
  cannot extend the queue itself.

## Contract Changes

- API/OpenAPI: N/A; no product API behavior changes.
- Protocol/event schemas: N/A; only the order for later protocol planning is
  documented.
- Database/migrations: N/A.
- Admin-new: N/A; `#124` remains future work.
- CLI/SDK: N/A.
- Deployment/configuration: N/A; existing loop environment variables are
  unchanged.
- README/ARCH/AGENTS/operator docs: update `dev_loop/README.md` and canonical
  execution docs. Root `README.md`, `ARCH.md`, and `AGENTS.md` do not need a
  change because product/runtime behavior is unchanged.

## Security And Data Impact

The qualifier remains read-only until one issue passes the readiness contract.
It must not infer legal, security, deployment, customer, or data-acceptance
decisions. Deferred issue bodies, labels, and comments remain unchanged. The
result may name public issue blockers but must not expose credentials, local
tokens, private run logs, or sensitive environment data.

## Migration, Compatibility, And Rollback

This is an additive prompt and documentation contract. Existing Ready-first
behavior is unchanged. Rollback is a revert of the prompt/docs commit; no
repository data, issue labels, schema, or runtime state requires migration.
Older loop runs remain historical evidence and need no conversion.

## Observability And Operator Feedback

Qualification summaries must list deferred candidates and explain why the
selected fallback is the first eligible item. Exhaustion must name the finite
queue and blockers. Existing run JSONL, structured result, console summary, and
journal files remain the operator evidence surfaces.

## Implementation Slices

1. Commit the issue-aligned plan and finite roadmap queue.
2. Tighten the qualification prompt and contributor documentation.
3. Add static prompt/roadmap contract tests and run focused validation.
4. Execute one real loop iteration from clean synchronized `main` after merge.

## Test Strategy

### Unit

- Assert the qualification prompt requires ordered traversal.
- Assert deferred blockers must be retained.
- Assert missing plans may route to specification.
- Assert only the first eligible fallback may be selected.
- Assert queue exhaustion is terminal and cannot authorize arbitrary backlog
  work.

### Integration

- Run the complete loop shell test suite under `set -euo pipefail`.
- Validate repository documents and Git diff integrity.

### Smoke And E2E

- Run one live qualification iteration with no Ready work.
- Confirm `#174` and `#180` are not mutated.
- Confirm `#175` routes to one fresh specification session and PR.
- Confirm product implementation does not run in the same iteration.

### Coverage And Quality

- `bash -n` covers shell syntax.
- The loop test suite covers the prompt/roadmap contract and driver regression
  surface.
- Repository document validation covers Markdown links and policy.
- Product coverage is N/A because no product code changes.

## Manual Test Sequence

1. Check out clean synchronized `main` after this slice merges.
2. Verify no issue has `state:ready` and no `codex/BPANE-*` PR is open.
3. Run
   `direnv exec . env ITERATIONS=1 AUTO_QUALIFY=1 AUTO_MERGE=0 ./dev_loop/loop.sh`.
4. Verify the qualification result records `#174` and `#180` as externally
   deferred.
5. Verify it selects `#175` with `NEEDS_SPECIFICATION` because the focused plan
   is missing.
6. Verify a fresh specification session creates one `#175` plan and PR.
7. Verify `#174` and `#180` remain open with `state:qualified` and unchanged
   scope.
8. Inspect the run journal and confirm the iteration stops after specification.

## Documentation And Claim Impact

This changes contributor-loop behavior and execution order only. It does not
change BrowserPane product capability, maturity, API, deployment, or investor
claims. Canonical roadmap and current-context documents must show the same
fallback order.

## Definition Of Done

- Issue `#260`, this plan, and the canonical roadmap agree.
- The qualifier traverses only the finite documented queue in order.
- External decisions remain fail-closed and deferred issues remain untouched.
- Prompt/roadmap regression tests and repository document validation pass.
- A live post-merge iteration selects `#175` for specification.
- Root README/ARCH non-impact is recorded in the PR handoff.

## Post-Implementation Smoke Sequence

1. Run `bash -n dev_loop/loop.sh dev_loop/tests/loop_test.sh`.
2. Run `./dev_loop/tests/loop_test.sh`.
3. Run
   `node scripts/validate.mjs --stage repository-baseline --stage repository-documents`.
4. Run `git diff --check`.
5. Execute the bounded live iteration from the manual sequence after merge.

## Evidence Record

Record the implementation commit, PR, loop-test count, document-validation
result, and post-merge live run/PR here or in the closing issue comment.
