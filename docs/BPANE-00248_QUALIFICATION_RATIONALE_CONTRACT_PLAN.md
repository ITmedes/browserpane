# BPANE-00248 Qualification Rationale Contract Plan

## Metadata

- Issue: `#248`
- State: Review
- Owner: BrowserPane maintainers / Codex
- Lane: Foundation
- Target gate: Reliable bounded contributor delivery
- Depends on: `#241` and `#246` (complete)
- Last verified commit/date: `0bad8f3f` / 2026-08-21

## Business Outcome

A successful qualification result is not reported as a failed Codex session
solely because it contains a useful non-empty rationale. The shell contract,
prompt, and shared JSON Schema agree on the allowed result shape, while live
GitHub lifecycle verification remains the authority for successful promotion.

## Example Use Case

On run `20260821-063412`, the qualifier audited `#172`, posted the required
readiness comment, moved the issue from Qualified to Ready, and verified the
live state. It returned `QUALIFIED` with issue `172`, null PR/commit/run fields,
and a concise `reason` explaining why the issue passed. The generic JSON Schema
accepted the string, but `validate_result` required `reason == null`, so the
driver stopped after the authorized mutation had already succeeded.

After this fix, that exact result is accepted. The qualification gate still
verifies that the issue was in the original Qualified queue and is now the sole
Ready issue before any proposal session starts.

## Scope

- Permit `QUALIFIED.reason` to be either null or a non-empty string.
- Continue rejecting empty strings, non-string values, missing issue numbers,
  PR/commit/run fields, and invalid live lifecycle state.
- Add the observed result shape as a regression fixture.
- Add safe invalid-result diagnostics that report status and field shape,
  without echoing rationale or other potentially sensitive content.
- Clarify the qualification routine's final-result contract.

## Non-Goals

- Recovering arbitrary malformed or incomplete JSON.
- Relaxing `NEEDS_SPECIFICATION`, `SPECIFIED`, proposal, repair, or halt
  contracts.
- Skipping live Ready-queue and issue-label verification.
- Changing `#172` requirements or implementing the Workflow Endpoint.
- Re-running the already successful `#172` qualification mutation.

## Contract Impact

- Product API/OpenAPI/protocol/events/database/Admin-New/CLI/SDK: N/A.
- Contributor result schema: no type change; `reason` already permits string or
  null. The shell status predicate is aligned with that schema for
  `QUALIFIED`.
- Contributor prompt: explicitly permits a concise non-empty qualification
  rationale or null.
- Deployment/configuration: N/A.
- README/ARCH/product claims: N/A; only `dev_loop` operator documentation may
  need a short result-contract clarification.

## Security And Failure Behavior

- Diagnostics print only status, issue number, and whether fields are null or
  their JSON types. They do not print reason text, URLs, commit contents,
  secrets, or raw result payloads.
- A rationale does not authorize promotion. `qualification_gate` still checks
  the original Qualified queue, exact Ready count, and selected issue lifecycle
  label after the model session.
- Invalid output still fails closed before proposal.

## Migration, Compatibility, And Rollback

The change is backward compatible: existing `QUALIFIED` results with null
reason continue to pass, and previously rejected non-empty rationales become
valid. Rollback restores the null-only predicate but would reintroduce the
observed post-mutation false failure. No persistent data migration is involved.

## Implementation Steps

1. Add the exact observed `QUALIFIED` result and invalid empty-rationale fixture
   to the isolated shell suite.
2. Relax only the `QUALIFIED.reason` predicate to null or non-empty string.
3. Add a redacted field-shape diagnostic for rejected final results.
4. Align the qualification routine and loop README.
5. Run shell syntax, all loop tests, diff checks, and repository document
   validation.

## Test Strategy

### Unit And Integration

- Existing null-reason `QUALIFIED` fixture passes.
- Observed non-empty-rationale `QUALIFIED` fixture passes.
- Empty-string rationale fails.
- Missing issue, non-null PR/commit/run, wrong status, and other existing
  negative fixtures remain rejected.
- Diagnostic output identifies safe field shape without containing the
  rationale text.
- Existing qualification-gate mocks continue to verify live issue state.

### Smoke

After merge, start the loop from clean synchronized `main`. Because `#172` is
already Ready from the successful qualification mutation, the driver must skip
qualification and enter a fresh proposal session. No issue label should be
mutated again by this fix.

## Post-Implementation Smoke Sequence

1. Run `bash -n dev_loop/loop.sh dev_loop/tests/loop_test.sh`.
2. Run `dev_loop/tests/loop_test.sh` and confirm all fixtures pass.
3. Run repository baseline and document validation.
4. Confirm live `#172` has exactly `state:ready` among lifecycle labels.
5. Merge the fix and synchronize clean `main`.
6. Run one supervised loop iteration with `AUTO_MERGE=0`.
7. Confirm the loop starts proposal for `#172` without another qualification
   comment or label mutation.

## Definition Of Done

- The issue and this plan agree on the observed failure and bounded fix.
- The exact rejected result is covered and accepted.
- Harmless rationale acceptance does not weaken lifecycle verification.
- Invalid-result diagnostics are actionable and redacted.
- Shell and document validation pass.
- No product documentation or claim is changed.

## Implementation Evidence

- `bash -n dev_loop/loop.sh dev_loop/tests/loop_test.sh` passed.
- `dev_loop/tests/loop_test.sh` passed all `69/69` fixtures.
- The actual
  `dev_loop/runs/20260821-063412/01-qualify.result.json` artifact passes the
  patched `validate_result` contract.
- `node scripts/validate.mjs --stage repository-baseline --stage
  repository-documents` passed both stages.
- Live GitHub verification confirms `#172` is open with exactly
  `state:ready`; its latest readiness comment records the successful audit.
- README and ARCH are unchanged because this fix affects contributor tooling,
  not BrowserPane product or runtime behavior.
