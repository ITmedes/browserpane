# BPANE-00244 Development Loop Disk Guard Plan

## Metadata

- Issue: `#244`
- State: In Progress
- Owner: BrowserPane maintainers / Codex
- Lane: Foundation
- Target gate: Safe local contributor automation
- Depends on: `#241` Codex-native delivery loop (complete through PR #243)
- Last verified commit/date: `8618d83c` / 2026-08-20

## Business Outcome

The local Codex delivery loop stops before low disk capacity can destabilize a
build, Docker, or an implementation session. Maintainers receive the measured
capacity and configured minimum instead of discovering the condition through
partial writes or unrelated test failures.

## Example Use Case

A workstation has 47 GiB available after repeated Docker-backed validation.
Starting the loop reports that the 50 GiB minimum is not met and exits before
qualification or implementation. A workstation with exactly 50 GiB available
may continue. An operator can explicitly adjust the threshold for a controlled
environment without changing the script.

## Current Evidence

- `dev_loop/loop.sh` already validates configuration, enforces a clean
  synchronized checkout, and separates qualification, proposal, and repair.
- The driver currently has no local capacity check.
- `df -Pk <repository>` provides available 1024-byte blocks for the filesystem
  that will receive repository, build, and local loop artifacts on supported
  macOS and Linux contributor environments.
- `dev_loop/tests/loop_test.sh` sources the production driver and can exercise
  capacity parsing and boundary behavior without GitHub or Codex mutation.

## Scope

- Add `MIN_FREE_DISK_GB`, defaulting to `50` binary GiB.
- Measure the filesystem containing the repository and fail closed when the
  available capacity cannot be determined.
- Report capacity during read-only preflight.
- Enforce the guard before synchronization and immediately before each local
  qualification, proposal, or repair phase.
- Cover below, equal, above, disabled, malformed-measurement, and command-error
  behavior in the shell suite.
- Document configuration, stop behavior, and recovery.

## Non-Goals

- Deleting Docker images, containers, volumes, caches, or user files.
- Monitoring GitHub-hosted runner capacity.
- Predicting the maximum storage that a future implementation will consume.
- Interrupting a currently running external build process between polling
  points.

## Decisions And Dependencies

- Capacity is binary GiB: one GiB is 1,048,576 KiB.
- Exactly the configured threshold passes; only lower capacity blocks.
- `MIN_FREE_DISK_GB=0` explicitly disables the threshold while preserving
  measurement/reporting. The default remains 50 GiB.
- Measurement is against the repository path rather than Docker's reported
  reclaimable space because filesystem exhaustion is the safety condition.
- The driver reports and stops; cleanup remains an explicit operator action.

## Contract Changes

- API/OpenAPI: N/A; no product API change.
- Protocol/event schemas: N/A.
- Database/migrations: N/A.
- Admin-new: N/A.
- CLI/SDK: contributor-loop configuration adds `MIN_FREE_DISK_GB`.
- Deployment/configuration: N/A for BrowserPane runtime; local contributors
  need POSIX `df` and `awk`.
- README/ARCH/AGENTS/operator docs: update `dev_loop/README.md`; root README,
  ARCH, OpenAPI, and runtime documentation are unaffected.

## Security And Data Impact

The guard reads aggregate free capacity for the repository filesystem only. It
does not enumerate files, inspect Docker content, delete data, or send capacity
to GitHub/Codex. No tenant, credential, browser, or workflow data is affected.

## Migration, Compatibility, And Rollback

The default introduces an intentional startup/phase gate for environments with
less than 50 GiB free. Operators can clean local storage or explicitly set a
lower non-negative threshold. Rollback is removal of the guard and its
configuration; no persisted product data requires migration.

## Observability And Operator Feedback

Preflight and execution report available GiB and the configured minimum. Low
capacity and measurement failures produce distinct actionable errors. The
journal records `low-disk` when an active iteration is stopped by the guard.

## Implementation Slices

1. Add measurement, formatting, configuration validation, and threshold logic.
2. Integrate read-only reporting and phase gates into the driver.
3. Add boundary/failure tests and operator documentation.
4. Run shell and repository-document validation, then commit the coherent
   contributor-tooling slice.

## Test Strategy

### Unit

- Reject invalid threshold configuration.
- Reject 49 GiB against a 50 GiB minimum.
- Accept exactly 50 GiB and capacity above the minimum.
- Accept an explicit zero threshold.
- Parse valid POSIX `df` output and reject malformed output.

### Integration

- Source the real driver with mocked `df` boundaries.
- Confirm `--check` reports live capacity under the approved project identity.
- Confirm low capacity returns a non-zero result before a Codex session.

### Smoke And E2E

No BrowserPane runtime smoke is required because the change affects local
contributor orchestration only. A supervised loop invocation with a threshold
above current free capacity proves the real fail-closed path without GitHub
mutation.

### Coverage And Quality

- `bash -n dev_loop/loop.sh dev_loop/tests/loop_test.sh`
- `./dev_loop/tests/loop_test.sh`
- `node scripts/validate.mjs --stage repository-baseline --stage
  repository-documents`
- `git diff --check`

## Manual Test Sequence

1. Synchronize a clean `main` checkout after this slice is merged.
2. Run `direnv exec . ./dev_loop/loop.sh --check` and inspect the capacity line.
3. Determine current free capacity from that line.
4. Run the loop with `MIN_FREE_DISK_GB` set above that value.
5. Confirm it exits before qualification/proposal and reports both values.
6. Run with the default 50 GiB threshold on a host with at least 50 GiB free.
7. Confirm the normal issue qualification/proposal path remains available.

## Documentation And Claim Impact

This is contributor safety tooling. It does not advance BrowserPane product
maturity or alter runtime, API, UI, deployment, or investor claims.

## Definition Of Done

- Default execution stops below 50 GiB and accepts exactly 50 GiB.
- Measurement failure stops execution before Codex work.
- Preflight and active-loop feedback are actionable.
- Qualification, proposal, and repair paths are guarded.
- Shell and repository-document validation pass.
- Issue `#244`, this plan, and operator documentation remain aligned.

## Post-Implementation Smoke Sequence

1. Run `./dev_loop/tests/loop_test.sh` and confirm all disk boundary cases pass.
2. Run the repository-local read-only preflight and confirm live capacity is
   reported.
3. Set `MIN_FREE_DISK_GB` above live capacity and run one iteration.
4. Confirm outcome `low-disk`, no Codex session, and no GitHub mutation.
5. Restore the default and confirm preflight passes with at least 50 GiB free.

## Evidence Record

- Issue: <https://github.com/ITmedes/browserpane/issues/244>
- Branch: `feature/BPANE-00244-disk-guard`
- `bash -n dev_loop/loop.sh dev_loop/tests/loop_test.sh` - passed.
- `./dev_loop/tests/loop_test.sh` - 51/51 checks passed, including 49/50/51
  GiB boundaries, explicit zero, malformed output, and command failure.
- Repository-local `--check` reported 198.5 GiB available against the 50 GiB
  default; it otherwise rejected the expected feature/dirty checkout.
- A real supervised run with `MIN_FREE_DISK_GB=10000` stopped before
  synchronization/Codex execution and journaled `low-disk`; no issue, branch,
  PR, or other GitHub mutation occurred.
- `node scripts/validate.mjs --stage repository-baseline --stage
  repository-documents` - passed with 47 tracked JSON files, 96 Markdown
  files, 19 YAML files, and 3 workflows.
- `git diff --check` - passed.
- `shellcheck` was not installed; Bash syntax and the sourced production
  function suite provide the available local shell evidence.
- Root README, ARCH, OpenAPI, product runtime, and product tests: no change
  required because this slice affects contributor orchestration only.
