# BPANE-00251 Codex Loop Merge Gate Plan

## Metadata

- Issue: `#251`
- State: Implemented and locally validated
- Owner: BrowserPane maintainers / Codex
- Lane: Foundation
- Target gate: Safe local contributor automation
- Depends on: `#241` Codex-native delivery loop
- Last verified commit/date: `ab9dc564` / 2026-08-21

## Business Outcome

The Codex delivery loop distinguishes an implementation defect from an
intentional repository governance gate. Green, current pull requests that
still require review remain available for an authorized reviewer and do not
consume a model repair session for a conflict that does not exist.

## Example Use Case

PR `#250` has a head that contains `origin/main`, all checks pass, and GitHub
reports `MERGEABLE`. The protected branch still reports `REVIEW_REQUIRED` and
merge state `BLOCKED`. With `AUTO_MERGE=1`, the driver records
`review-required`, leaves the pull request open, and exits successfully. Once
an authorized reviewer approves, a later loop run can merge and perform the
existing post-merge validation.

## Current Evidence

- `dev_loop/loop.sh` verifies green checks and branch ancestry before calling
  `gh pr merge`.
- Any non-zero merge command currently becomes `merge-conflict` without
  inspecting the live review decision.
- The resulting repair session correctly proved that PR `#250` had no content
  conflict and halted because required review was the only blocker.
- GitHub exposes the necessary distinction through `mergeable`,
  `mergeStateStatus`, and `reviewDecision`.

## Scope

- Add a deterministic classifier for GitHub PR merge snapshots.
- Give content conflicts precedence when conflict and review signals coexist.
- Stop cleanly for required review, requested changes, and other branch-policy
  blocks.
- Fail closed for unavailable, unknown, or internally inconsistent merge
  state.
- Refresh the merge snapshot after a failed merge command to handle races.
- Add shell fixtures and document the new stop outcomes.

## Non-Goals

- Bypassing branch protection with `--admin`.
- Approving a Codex-authored pull request automatically.
- Weakening checks, review rules, or conversation-resolution policy.
- Changing PR `#250` or the Phase 0 Workflow Endpoint.
- Adding a hosted merge queue.

## Decisions And Dependencies

- `CONFLICTING` or `DIRTY` is a technical conflict and may enter repair.
- `REVIEW_REQUIRED` and `CHANGES_REQUESTED` are human review states and stop
  without repair.
- `BLOCKED` without a more specific review signal is a policy block and stops
  without repair.
- `BEHIND` is classified explicitly. After a successful local ancestry check
  it is inconsistent/stale state; after a failed branch update it is reported
  as an update failure rather than guessed to be a conflict.
- Unknown or unavailable GitHub state is not treated as a conflict.
- The driver remains the sole owner of polling and merging.

## Contract Changes

- API/OpenAPI: N/A.
- Protocol/event schemas: N/A.
- Database/migrations: N/A.
- Admin-new: N/A.
- CLI/SDK: contributor-loop journal gains explicit merge-gate outcomes.
- Deployment/configuration: N/A.
- Documentation: update `dev_loop/README.md`; root `README.md`, `ARCH.md`,
  runtime docs, and product claims are unaffected.

## Security And Data Impact

The change preserves branch protection and makes its effect explicit. It reads
only sanitized pull-request state already available to the authenticated
project identity. It does not expose credentials, alter product data, or use
administrator merge privileges.

## Migration, Compatibility, And Rollback

There is no persisted-data migration. Existing configurations remain valid.
`AUTO_MERGE=1` changes only when GitHub policy blocks the merge: the run now
stops with an accurate outcome instead of starting an ineffective repair.
Rollback is limited to reverting the driver, tests, and documentation.

## Implementation Slices

1. Introduce and unit-test the pure merge snapshot classifier.
2. Integrate pre-merge and post-failure classification into the driver.
3. Document operator outcomes and verify the live PR `#250` state read-only.
4. Run shell and repository-document validation and commit the bounded slice.

## Test Strategy

### Unit

- Classify ready, required-review, requested-changes, generic policy-blocked,
  content-conflict, stale-base, and unknown snapshots.
- Confirm content conflict takes precedence over a pending review signal.
- Confirm malformed snapshots fail instead of producing `ready`.

### Integration

- Source the production driver and feed the classifier realistic GitHub JSON.
- Read PR `#250` through `gh pr view` and confirm it classifies as
  `review-required` while its live state remains mergeable and blocked.

### Smoke And E2E

No BrowserPane runtime smoke is required because the slice changes only local
delivery orchestration. The post-implementation supervised smoke uses a
green/current review-protected PR and verifies no repair session is created.

### Coverage And Quality

- `bash -n dev_loop/loop.sh dev_loop/tests/loop_test.sh`
- `./dev_loop/tests/loop_test.sh`
- `node scripts/validate.mjs`
- `git diff --check`

## Manual Test Sequence

1. Start from a clean synchronized `main` after this fix is merged.
2. Ensure a Codex PR is green/current but still requires review.
3. Run one iteration with `AUTO_MERGE=1`.
4. Confirm the run records `review-required` and exits without a repair file.
5. Confirm the PR remains open and no admin merge was attempted.
6. Obtain an authorized approval and rerun the loop.
7. Confirm normal merge and exact-merge-SHA post-merge validation resume.

## Documentation And Claim Impact

This is contributor automation correctness. It does not change BrowserPane
runtime capabilities, API claims, Admin-New behavior, or deployment maturity.

## Definition Of Done

- Review and policy blocks cannot be mislabeled as merge conflicts.
- Only reproducible content conflicts enter the repair path.
- Merge-command races use a fresh snapshot.
- Explicit outcomes are documented and journaled.
- Tests and repository document validation pass.
- Issue `#251`, this plan, and the implementation remain aligned.

## Post-Implementation Smoke Sequence

1. Run the shell syntax and loop test suite.
2. Run repository document validation.
3. Classify live PR `#250` read-only and confirm `review-required`.
4. Confirm the last failed run's `MERGEABLE` / `BLOCKED` /
   `REVIEW_REQUIRED` tuple maps to a governance stop, not repair.
5. Review the diff for any admin-bypass or gate-weakening behavior.

## Evidence Record

- Issue: <https://github.com/ITmedes/browserpane/issues/251>
- Branch: `feature/BPANE-00251-dev-loop-merge-gate`
- Triggering PR: <https://github.com/ITmedes/browserpane/pull/250>
- `bash -n dev_loop/loop.sh dev_loop/tests/loop_test.sh` - passed.
- `./dev_loop/tests/loop_test.sh` - 77/77 checks passed.
- Repository baseline/document validation - passed.
- `git diff --check` - passed.
- Live PR `#250` snapshot - `MERGEABLE`, `BLOCKED`, `REVIEW_REQUIRED`;
  production classifier returned `review-required`.
- ShellCheck was not available on the local host; Bash syntax and the sourced
  production-driver suite provide the executed shell evidence.
