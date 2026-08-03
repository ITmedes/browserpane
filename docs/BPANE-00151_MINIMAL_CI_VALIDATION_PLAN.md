# BPANE-00151 Minimal CI And Validation Plan

Issue: [#151](https://github.com/ITmedes/browserpane/issues/151)

State: In Progress

Owner: `thebackplane`

Lane: Foundation

Target gate: Foundation Gate

Depends on: #173 delivery-governance baseline

Last verified: 2026-08-03 on `feature/BPANE-00151` through the Slice 3 coverage ratchet

## Business Outcome

Make every BrowserPane change pass a reproducible, enforced validation floor
before merge. The first result is not generic Production Readiness; it is a
trusted development baseline that detects vulnerable dependencies, compilation
and type failures, unit regressions, contract drift, and selected end-to-end
breakage without requiring a reviewer to reconstruct local commands.

## Example Use Case

A pull request changes gateway authentication and the unified admin client.
GitHub automatically validates the Rust workspace, relevant Node packages,
admin-new, dependency locks, coverage baselines, and documentation paths. A
wrong-purpose token test or a vulnerable runtime dependency makes a named check
fail with an actionable command. The author can run the same entry point
locally before pushing, and a reviewer can see which heavier compose smoke was
executed without treating an unrun smoke as green.

## Current Evidence

- The default branch currently has no required status checks. This branch adds
  separate fast-validation and representative compose workflows; hosted runs
  and branch-protection enforcement remain pending.
- Rust workspace tests pass locally. The latest recorded `cargo llvm-cov`
  baseline is 56.25% line coverage across the workspace.
- `code/web/bpane-client` has unit, build, CLI, and extensive compose smoke
  scripts. Its maintained-source coverage is now ratcheted at 92.8% lines and
  statements, 93.1% functions, and 87.5% branches.
- `code/web/bpane-admin-unified` has 279 passing tests plus `check`, coverage,
  and build; its `src/lib` coverage now has a checked-in four-metric floor.
- The MCP bridge, recording worker, and workflow worker build independently;
  the recording and workflow workers do not yet have meaningful unit suites.
- Gateway compose suites exist behind ignored tests and
  `scripts/run-gateway-compose-e2e.sh`; they run only when explicitly selected.
- The live Dependabot inventory on 2026-08-03 contains 92 open alerts: one
  critical, 24 high, 50 medium, and 17 low. Critical/high alerts span
  `Cargo.lock` and four of the six committed Node lockfiles. Dependabot will
  retain the default-branch alerts until the remediated locks merge.
- Slice 1 updates the affected Rust and Node dependency groups to patched
  compatible versions. The branch-local npm audits report zero critical/high
  findings across all six lockfiles. `cargo audit` reports only
  `RUSTSEC-2023-0071`, a medium, no-fix RSA advisory retained through SQLx's
  disabled optional MySQL graph; it has an explicit exception through
  2026-11-01.
- `scripts/check-dependency-safety.mjs` scans every committed lockfile, fails
  all RustSec vulnerabilities and npm critical/high advisories unless an exact
  exception applies, and rejects stale or expired exceptions. Its parser,
  command-boundary, and policy behavior has 12 focused tests with 91.99% line
  and 100% function coverage under Node's built-in coverage runner.
- `scripts/validate.mjs` is now the canonical local entry point. It exposes 30
  fast stages and nine compose stages through `fast`, `compose`, and `full`
  profiles, with stable names, bounded execution, first-failure exit-code
  preservation, focused reruns, dry runs, and stage discovery.
- The expanded 31-stage fast profile and an uninterrupted compose profile pass
  on this branch. The compose run includes all 16 default gateway API cases,
  all four
  docker-pool cases, admin-new and compatibility-admin session journeys, CLI,
  two simultaneous session-scoped MCP routes, two retained recording segments,
  workflow worker/project admission, and restoration of normal compose limits.
- The first composed run exposed a real session-cleanup race after gateway
  reconciliation. Shared smoke cleanup now requires a stable quiet window,
  renews its bound only when removal makes progress, and has focused tests for
  delayed reconciliation, malformed catalogs, deduplication, and timeout.
- Checked-in coverage floors now gate fresh Rust workspace coverage at 56.2%
  lines, browser-client maintained sources at 92.8% lines/statements, 93.1%
  functions, and 87.5% branches, and admin-new `src/lib` at 88.0% lines, 90.1%
  statements, 92.7% functions, and 74.3% branches. Each command emits a local
  Markdown summary for later CI artifact publication.

## Scope

### 1. Dependency Safety Baseline

- Refresh every committed Rust and Node lockfile against the live advisory set.
- Remediate patched critical/high findings without silently dismissing alerts.
- For a finding that cannot be removed in this slice, record package,
  dependency path, runtime reachability, compensating control, owner, and
  expiry in a checked-in exception file.
- Add reproducible Rust and Node advisory commands that fail on unapproved
  critical/high findings and validate exception expiry.
- Keep broader SBOM, signing, provenance, release promotion, and continuous
  dependency governance under #75.

### 2. One Local Validation Entry Point

- Add a repository-level script that runs named stages and returns the first
  failing stage with its original exit code.
- Support a fast required profile and explicit heavier compose profiles.
- Keep package commands in their owning manifests; the wrapper orchestrates
  them instead of duplicating test logic.
- Make stage selection and prerequisites discoverable through `--help`.

### 3. Required GitHub Actions Checks

- Add least-privilege GitHub Actions workflows with pinned action revisions.
- Run Rust formatting, clippy, workspace tests, and the agreed coverage check.
- Run clean-install, type/check, unit, and build jobs for the browser client,
  old admin while retained, admin-new, MCP bridge, recording worker, and
  workflow worker.
- Run dependency and documentation/path checks in dedicated jobs so failures
  are attributable.
- Add a bounded compose/API smoke job with explicit setup, timeout, artifact
  collection, and cleanup. Keep expensive or host-specific suites scheduled or
  manually dispatched until they are reliable enough to become required.
- Document the exact required-check names and configure branch protection for
  `main`; workflow presence without an enforced merge rule is incomplete.

### 4. Coverage Ratchet

- Reproduce and record Rust and browser-client baselines on a clean checkout.
- Add coverage reporting for admin-new and meaningful worker logic introduced
  by this slice.
- Ratchet changed code and critical auth/store/protocol paths rather than
  adopting a misleading repository-wide percentage alone.
- Fail unexplained baseline regressions while allowing reviewed generated,
  platform-only, and process-entry exclusions.
- Publish human-readable summaries as CI artifacts; do not upload source or
  private artifacts to an unapproved external service.

### 5. Documentation And Contract Checks

- Validate local Markdown paths and the documented command/package paths.
- Verify that canonical README, AGENTS, ARCH, and validation commands still
  exist.
- Run parse/validation checks for committed JSON, YAML, and package manifests.
- Leave full OpenAPI implementation conformance and breaking-change policy to
  #179, but provide the CI stage that #179 can extend.

## Non-Goals

- Complete production release governance, SBOM/signing, or provenance (#75).
- Implement the full control API compatibility contract (#179).
- Make every docker, camera, GPU, cloud, or destructive recovery suite a merge
  check in the first iteration.
- Raise all subsystem coverage to an arbitrary common percentage.
- Dismiss alerts solely because a dependency is currently development-only.
- Refactor product behavior unrelated to dependency-compatible remediation.

## Decisions And Dependencies

- #151 is the single next Ready product slice after #173.
- #145 follows because token-domain changes need trusted required checks.
- #179 may add its P0 OpenAPI checks to the CI framework but remains a separate
  contract-governance outcome.
- #75 consumes this baseline for release governance rather than duplicating it.
- GitHub branch protection is repository state, so the PR evidence must include
  the configured required-check list in addition to workflow files.

## Contract Changes

- API/OpenAPI: no product API change; add parse/lint hooks only. Full
  conformance remains #179.
- Protocol/event schemas: no wire-format change.
- Database/migrations: none.
- Admin-new: clean install, check, unit, coverage baseline, build, and selected
  smoke become named validation stages.
- CLI/SDK: browser-client CLI unit/smoke coverage is included where it can run
  deterministically in CI.
- Deployment/configuration: CI gets a bounded compose profile; runtime defaults
  must not change to make tests pass.
- README/ARCH/AGENTS/operator docs: document the local entry point, required
  jobs, prerequisites, and which compose suites remain explicit.

## Security And Data Impact

- GitHub workflows use minimal permissions and no pull-request code receives
  production secrets.
- Actions are pinned to immutable revisions and third-party coverage upload is
  disabled unless separately approved.
- Dependency exceptions are reviewable, expiring, and cannot contain secrets.
- Logs and uploaded failure artifacts must redact bearer tokens, OIDC material,
  proxy credentials, recordings, browser profiles, and workspace file content.
- Compose jobs use fixture identities/data and always clean credentials and
  containers after completion.

## Migration, Compatibility, And Rollback

- Dependency upgrades are committed in small ecosystem/package groups with
  package-specific regression evidence.
- CI is introduced additively. A flaky job may be made non-required only with a
  linked defect, owner, expiry, and preserved scheduled execution.
- Rollback means reverting the incompatible dependency group or CI stage, not
  disabling the entire validation floor.
- Branch-protection changes and their reversal are recorded in PR evidence.

## Observability And Operator Feedback

- Every CI stage has a stable name, timeout, concise summary, and retained
  failure logs.
- Compose failures retain bounded service logs and test reports after redaction.
- Coverage output identifies the package and baseline delta.
- Dependency failures identify ecosystem, manifest, package, severity, patched
  version, and approved-exception status.
- The local wrapper prints the failing stage and exact rerun command.

## Implementation Slices

### Slice 1: Inventory And Dependency Remediation

Status: Implemented and locally validated on `feature/BPANE-00151`.

- Freeze the live advisory and package/test inventory.
- Upgrade critical/high dependency groups with focused regressions.
- Add the bounded exception schema/checker if any alert remains.

Evidence:

- `node scripts/check-dependency-safety.mjs` passes for `Cargo.lock` plus all
  six committed npm lockfiles with one exact, unexpired RSA exception.
- `cargo test --workspace --locked` passes after the Rust dependency updates.
- Clean install, type/check, unit tests, and production builds pass for the
  compatibility admin, admin-new, browser client, and MCP bridge packages.
- Package totals exercised by the focused regressions include 202
  compatibility-admin tests, 279 admin-new tests, 661 browser-client tests,
  and 12 MCP bridge tests.

Commit boundary: dependency changes and evidence are reviewable independently
from workflow plumbing.

### Slice 2: Local Validation Runner

Status: Implemented and locally validated on `feature/BPANE-00151`.

- Add fast, compose, and full profiles with stable stage names.
- Add self-tests for selection, failure propagation, cleanup, and `--help`.
- Align AGENTS and README commands.

Evidence:

- `node scripts/validate.mjs --profile fast` passes all 28 stages.
- `node scripts/validate.mjs --profile compose` passes all nine stages in one
  uninterrupted run and leaves the compose stack running with default runtime
  limits restored.
- Validation-tool tests cover argument rejection, stage selection, repository
  drift, child exit-code propagation, timeout process-group termination,
  cancellation, first-failure behavior, and session cleanup stabilization.
- The repository baseline verifies canonical files, package-owned commands,
  required package scripts, documentation references, and every tracked JSON
  file before expensive stages run.

### Slice 3: CI And Coverage

Status: Implemented locally; hosted execution remains Slice 4 evidence.

- Add pinned, least-privilege workflows and caching keyed by lockfiles.
- Establish package coverage baselines and publish summaries.
- Add bounded compose/API smoke and failure artifacts.

Evidence:

- `.github/workflows/validation.yml` defines stable repository, dependency,
  Rust, and per-package Node jobs on pinned `ubuntu-24.04` runners.
- All third-party actions use immutable commit revisions, checkout credentials
  are not persisted, and root permissions are limited to `contents: read`.
- `node scripts/check-repository-documents.mjs` parses committed YAML, checks
  local Markdown targets, and enforces workflow permissions, pins, timeouts,
  cache keys, and artifact boundaries.
- Node `22.23.2` and Rust `1.93.1` are pinned for local and CI parity.
- `.github/workflows/compose.yml` runs the representative nine-stage compose
  profile on `main`, weekdays, or manual dispatch with a 60-minute job bound.
- Failure evidence is limited to selected control-plane status and log tails,
  redacted before a seven-day artifact upload. Cleanup always removes dynamic
  BrowserPane runtime/worker containers and compose volumes.
- Redaction and collection tests cover bearer/basic authorization, cookies,
  OIDC query material, environment/CLI secrets, authenticated proxy URLs,
  JWTs, UUIDs, PEM blocks, selected commands, and oversized-log truncation.

### Slice 4: Enforcement And Evidence

Status: Pending.

- Configure `main` required checks.
- Demonstrate controlled failures for each major stage.
- Run the clean full profile, update validation/maturity/risk docs, and capture
  the Foundation-gate evidence that is actually satisfied.

## Test Strategy

### Unit

- Validation-runner argument parsing and stage selection.
- First-failure exit-code propagation and cleanup traps.
- Dependency-exception schema, severity policy, and expiry handling.
- Markdown/path and manifest-check fixtures.
- Coverage-baseline comparison, including allowed and rejected deltas.

### Integration

- Execute each package stage from a clean dependency install.
- Verify Rust/Node advisory tools against controlled vulnerable and approved
  exception fixtures.
- Verify CI workflow syntax, pinned actions, permissions, cache keys, timeouts,
  and artifact redaction.
- Run gateway in-memory/Postgres tests selected by the fast profile without
  changing their semantic owner under #152.

### Smoke And E2E

- Run the bounded gateway compose API suite.
- Run one admin-new auth/session/resource path and one old-admin regression path
  while both apps remain supported.
- Run one CLI flow, MCP session endpoint flow, workflow run, and recording flow
  in the explicit compose/full profile.
- Confirm startup failure, timeout, and cleanup leave no leaked test container
  or credential artifact.

### Coverage And Quality

- Rust: reproduce the 56.25% line baseline and add critical-path/changed-code
  ratchets.
- Browser client: reproduce the 91.26% core line baseline and prevent
  regression.
- Admin-new: establish and enforce an initial checked-in baseline from the 279
  existing tests.
- Workers and MCP bridge: cover deterministic parsing, validation, and
  lifecycle helpers; record process-boundary exclusions.
- Run rustfmt, clippy, TypeScript/Svelte checks, unit tests, builds, dependency
  scans, and `git diff --check`.

## Manual Test Sequence

1. Clone a clean checkout and run the validation runner with `--help`.
2. Run the fast profile and verify Rust, all maintained Node packages,
   dependency checks, coverage ratchets, and docs/manifests are named.
3. Introduce a controlled unit-test failure and confirm the local runner and CI
   report the same stage and non-zero result.
4. Restore it, introduce a controlled expired dependency exception, and verify
   dependency safety fails without running a product runtime.
5. Run the compose profile and verify session/API, admin-new, CLI/MCP,
   workflow, and recording selections plus bounded failure artifacts.
6. Interrupt a compose stage and confirm the active process group terminates,
   no later stage starts, the stack remains inspectable, and a rerun restores
   any smoke-specific runtime overrides.
7. Push a test branch, verify all documented required checks execute, and
   confirm `main` cannot merge while one required check is failing.
8. Restore the branch and verify the same checks pass from a clean run.

## Documentation And Claim Impact

- Update `AGENTS.md`, `README.md`, and `docs/VALIDATION_MATRIX.md` with the
  exact runner profiles and required checks.
- Update `CAPABILITY_MATURITY_MATRIX.md` and the Foundation risk entries only
  with measured evidence.
- Do not promote BrowserPane, admin-new, workflows, or the remote protocol to
  Production-ready based on CI alone.
- Update investor material only to say that an enforced engineering validation
  baseline exists after branch protection and all required checks are verified.

## Definition Of Done

- All patched critical/high findings are remediated or covered by an approved,
  expiring, technically justified exception.
- One local command reproduces every required merge check.
- GitHub Actions workflows are least-privilege, pinned, green, and configured as
  required checks on `main`.
- Rust, maintained Node packages, admin-new, workers, dependencies, coverage,
  and docs/manifests have an explicit validation floor.
- Controlled failures prove that each major stage blocks merge.
- Bounded compose smoke executes with timeout, redacted evidence, and cleanup.
- README, AGENTS, validation, roadmap, maturity, risk, issue, and PR evidence are
  synchronized.

## Post-Implementation Smoke Sequence

1. On a clean checkout, run the local fast validation profile and verify every
   required GitHub check has a matching local stage.
2. Run the full dependency scan and confirm zero unapproved critical/high
   findings across `Cargo.lock` and every committed Node lockfile.
3. Run Rust fmt, clippy, tests, and coverage; browser-client type/tests/coverage
   and build; old-admin/admin-new checks/tests/builds; and all integration
   package build/test stages.
4. Run the bounded compose API/admin-new/CLI/MCP/workflow/recording profile.
5. Exercise controlled unit, dependency-policy, coverage-regression, and
   compose-timeout failures and confirm each named CI check blocks merge.
6. Restore the fixtures, rerun all required checks, and verify they pass.
7. Verify branch protection lists the documented required checks and rejects a
   PR with any one check failing.
8. Inspect retained logs/artifacts and confirm secrets, browser data, and user
   files are absent or redacted.

## Evidence Record

Record the implementing PR, dependency-alert before/after counts, lockfile
changes, package test output, coverage baselines/deltas, compose smoke result,
required-check names, branch-protection evidence, exceptions with expiry, and
the resulting Foundation gate decision.
