# BPANE-00253 Codex Loop Admin Merge Plan

## Metadata

- Issue: `#253`
- State: Implemented and locally validated
- Owner: BrowserPane maintainers / Codex
- Lane: Foundation
- Target gate: Authorized unattended contributor automation
- Depends on: `#241` delivery loop and `#251` merge-gate classification
- Last verified commit/date: `d9aca33c` / 2026-08-21

## Business Outcome

An explicitly authorized BrowserPane project identity can operate the Codex
delivery loop end to end when repository policy permits administrators to land
changes directly. The safe default still leaves review-protected pull requests
open; privilege escalation requires a second, visible operator decision.

## Example Use Case

A Codex PR is current with `main`, all checks pass, and GitHub reports
`MERGEABLE`, `BLOCKED`, and `REVIEW_REQUIRED`. With only `AUTO_MERGE=1`, the
driver records `review-required`. With both `AUTO_MERGE=1 ADMIN_MERGE=1`, and
after preflight verifies `viewerPermission=ADMIN`, it uses GitHub's supported
`gh pr merge --admin` path. The loop then validates the exact merge commit on
`main` before selecting more work.

## Current Evidence

- PR `#250` proved that normal automatic merge stops correctly at required
  review after `#251`.
- The configured project identity reports repository permission `ADMIN`.
- GitHub CLI explicitly identifies `--admin` as the direct-merge path for this
  protected-branch state.
- The driver already waits for all checks and verifies base ancestry before
  evaluating the merge gate.

## Scope

- Add `ADMIN_MERGE`, default `0`, as a validated boolean configuration.
- Require `AUTO_MERGE=1` whenever admin merge is enabled.
- Require live GitHub repository permission `ADMIN` during preflight.
- Select admin merge only for required-review or generic policy-blocked merge
  gates.
- Keep requested changes, content conflicts, pending/red checks, and stale
  branches outside the admin merge path.
- Make admin merge attempts and failures explicit in console, journal, tests,
  and contributor documentation.

## Non-Goals

- Enabling admin bypass by default.
- Bypassing pending or failed checks.
- Merging a stale or conflicting branch.
- Overriding `CHANGES_REQUESTED`.
- Auto-approving pull requests.
- Changing BrowserPane runtime or product behavior.

## Decisions And Dependencies

- `AUTO_MERGE=1` expresses intent to merge; `ADMIN_MERGE=1` separately
  expresses intent to bypass an otherwise satisfied review/policy gate.
- Only GitHub `viewerPermission=ADMIN` is accepted for this path. `MAINTAIN`
  and `WRITE` remain sufficient for supervised loop use but not admin merge.
- Admin merge is considered only after the existing check and branch-currency
  gates pass.
- `CHANGES_REQUESTED` is a semantic review failure and is never bypassed.
- A failed admin merge is an operator/policy failure, not a code conflict.

## Contract Changes

- API/OpenAPI: N/A.
- Protocol/event schemas: N/A.
- Database/migrations: N/A.
- Admin-new: N/A.
- CLI/SDK: contributor loop adds `ADMIN_MERGE`.
- Deployment/configuration: local automation environment only.
- Documentation: update `dev_loop/README.md`; root `README.md`, `ARCH.md`,
  runtime docs, and product claims are unaffected.

## Security And Data Impact

The capability is privilege-sensitive and therefore disabled by default,
requires two explicit configuration flags, verifies the live project identity
and `ADMIN` permission, and preserves all technical checks. No credential or
product data is read beyond existing GitHub identity and PR metadata.

## Migration, Compatibility, And Rollback

Existing loop invocations are unchanged. Operators who deliberately need
direct landing add `ADMIN_MERGE=1` next to `AUTO_MERGE=1`. Removing the new
variable returns to the protected-review stop. There is no persisted-data or
API migration.

## Implementation Slices

1. Add configuration, permission, and merge-mode helpers with unit coverage.
2. Integrate the explicit admin path and distinct failure handling.
3. Update preflight/startup feedback and operator documentation.
4. Run shell and repository-document validation and commit the bounded slice.

## Test Strategy

### Unit

- Reject invalid `ADMIN_MERGE` values.
- Reject admin merge without automatic merge.
- Accept only repository `ADMIN` permission for admin merge.
- Select normal merge for a ready PR.
- Select admin merge for required-review/policy gates only when enabled.
- Never select admin merge for requested changes or conflicts.
- Verify `land` adds `--admin` only in admin mode.

### Integration

- Source the production driver in the shell suite.
- Mock GitHub merge snapshots and command arguments.
- Run read-only preflight under the live project identity and confirm `ADMIN`.

### Smoke And E2E

This is local delivery orchestration, so no BrowserPane runtime smoke is
required. A disposable green/current PR under the protected branch provides
the end-to-end smoke for normal stop versus explicit admin landing.

### Coverage And Quality

- `bash -n dev_loop/loop.sh dev_loop/tests/loop_test.sh`
- `./dev_loop/tests/loop_test.sh`
- `node scripts/validate.mjs --stage repository-baseline --stage repository-documents`
- `git diff --check`

## Manual Test Sequence

1. Run `./dev_loop/loop.sh --check` as the approved project identity.
2. Confirm preflight reports repository permission `ADMIN`.
3. Run a green/current protected PR with `AUTO_MERGE=1` only.
4. Confirm outcome `review-required` and no merge.
5. Rerun with `AUTO_MERGE=1 ADMIN_MERGE=1`.
6. Confirm the driver announces admin merge, GitHub lands the PR, and the loop
   waits for exact-merge-SHA workflows.
7. Confirm requested changes and conflicts still stop or repair respectively.

## Documentation And Claim Impact

This changes contributor automation governance only. It does not alter
BrowserPane capabilities, deployment maturity, or external product claims.

## Definition Of Done

- Admin merge is explicit, permission-checked, and disabled by default.
- Green/current/check-passing gates remain mandatory.
- Requested changes and conflicts cannot use admin merge.
- Admin failures have a distinct outcome.
- Tests and repository document validation pass.
- Issue `#253`, this plan, and implementation remain aligned.

## Post-Implementation Smoke Sequence

1. Run shell syntax and all loop tests.
2. Run repository baseline/document validation.
3. Confirm default required-review behavior remains unchanged.
4. Confirm the live project identity passes the admin permission gate.
5. Land a disposable green/current protected PR with both merge flags.
6. Confirm post-merge validation is still tied to the exact merge SHA.

## Evidence Record

- Issue: <https://github.com/ITmedes/browserpane/issues/253>
- Branch: `feature/BPANE-00253-admin-merge`
- Precondition: live project identity reports `viewerPermission=ADMIN`.
- `bash -n dev_loop/loop.sh dev_loop/tests/loop_test.sh` - passed.
- `./dev_loop/tests/loop_test.sh` - 91/91 checks passed.
- Repository baseline/document validation - passed.
- `git diff --check` - passed.
- Production preflight with both merge flags returned
  `identity=thebackplane permission=ADMIN admin_merge=1`.
- ShellCheck is unavailable on the local host; Bash syntax and the sourced
  production-driver suite provide the executed shell evidence.
