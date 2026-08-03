# BPANE-00184 Compose Validation Performance Plan

## Metadata

- Issue: [#184](https://github.com/ITmedes/browserpane/issues/184)
- State: In Progress
- Owner: `thebackplane`
- Lane: Foundation
- Target gate: Foundation Gate
- Depends on: #151 minimal CI and validation baseline
- Last verified: 2026-08-03 on `dev/compose-validation-performance`

## Business Outcome

Reduce the feedback time of BrowserPane's hosted representative compose
validation without removing scenarios or weakening failure evidence. Developers
should receive API, runtime, admin, CLI, MCP, recording, and workflow feedback
in parallel instead of waiting for one 36-minute sequential job.

## Example Use Case

A gateway API change reaches `main`. The default API contract suite, the
docker-pool capacity and restart suite, and the browser-facing integration
smokes start on isolated runners. An admin-new regression is reported without
waiting for both API suites, while each unaffected lane still completes and
retains its own diagnostics. The local full compose profile remains available
for reproducing the complete sequence on one workstation.

## Current Evidence

Successful hosted Compose runs `30831720475` and `30835269073` took about 36
minutes each. The second run measured:

| Work | Duration |
| --- | ---: |
| Cold host/gateway compose image build and stack readiness | about 7.5 minutes |
| Rust compose-test compilation | about 2 minutes |
| 16 default gateway API scenarios | 8 minutes 7 seconds |
| 4 docker-pool gateway API scenarios | 5 minutes 21 seconds |
| 8 browser/admin/integration smoke stages | about 11 minutes |

The gateway API stage consumes about 24 minutes and currently serializes both
API suites ahead of every browser-facing smoke. Docker's cache is useful only
inside one ephemeral runner; the existing host `target` cache does not feed
the Docker builders. The runtime scenarios, rather than compilation alone,
are the largest repeatable cost.

## Scope

- Preserve all 20 gateway compose API tests and all eight browser-facing smoke
  stages currently selected by the compose validation profile.
- Add an explicit stack-only preparation mode to the gateway compose wrapper.
- Split hosted execution into isolated default API, docker-pool API, and
  browser/admin/integration lanes.
- Keep per-lane timeouts, redacted failure diagnostics, and unconditional
  compose cleanup.
- Keep `node scripts/validate.mjs --profile compose` and
  `scripts/run-gateway-compose-e2e.sh --suite all` as full sequential local
  entry points.
- Measure the hosted workflow after the change before deciding whether to add
  a maintained precompiled Rust dependency builder.

## Non-Goals

- Remove, sample, or shorten product scenarios solely to improve elapsed time.
- Run tests concurrently against one shared compose database or runtime.
- Publish production host/gateway images.
- Introduce an unscanned mutable builder image or widen workflow permissions.
- Make the scheduled representative compose workflow a required pull-request
  check in this slice.

## Decisions And Dependencies

- Isolation uses separate hosted jobs because the suites mutate session,
  gateway, worker, and database state and are intentionally single-threaded
  within one compose environment.
- Job sharding is the first optimization because it addresses the measured
  critical path without adding a package registry or changing runtime images.
- A dependency builder is a follow-up only if the measured post-shard critical
  path remains too high. It must be pinned and keyed by Rust toolchain, target
  architecture, Cargo manifests and lockfile, build profile and features,
  native libraries, and relevant `RUSTFLAGS`; coverage must use a separate
  cache domain.
- #151 remains the owner of the validation baseline. #184 changes execution
  topology and performance, not its coverage claim.

## Contract Changes

- API/OpenAPI: N/A; no product API behavior changes.
- Protocol/event schemas: N/A; no wire changes.
- Database/migrations: N/A; each hosted lane receives an isolated ephemeral
  compose database.
- Admin-new: no product change; its dashboard, projects, and sessions smokes
  move to the browser-facing hosted lane.
- CLI/SDK: no product change; CLI and browser-client smoke commands remain
  unchanged.
- Deployment/configuration: add a stack-only test-preparation option and split
  the hosted Compose workflow into isolated jobs.
- README/ARCH/AGENTS/operator docs: README and ARCH do not need product-facing
  changes because local commands and runtime topology stay unchanged. Update
  validation documentation if the hosted job names or usage contract changes.

## Security And Data Impact

- Retain `contents: read` workflow permissions and immutable action revisions.
- Do not expose repository or runtime credentials to pull-request code.
- Preserve token, credential, URL, recording, profile, and workspace-content
  redaction in collected diagnostics.
- Every lane cleans containers, networks, and test volumes even after failure.
- Parallel jobs use isolated GitHub runners, avoiding cross-lane tenant and
  fixture state.

## Migration, Compatibility, And Rollback

- Local full validation entry points remain backward compatible.
- Hosted job names change, but the workflow remains scheduled, manually
  dispatchable, and triggered on `main` pushes.
- Rollback is a workflow/script revert; no product data or runtime migration is
  involved.
- A failed lane must not cancel sibling lanes, so evidence remains available
  for all independently executing areas.

## Observability And Operator Feedback

- Give each hosted lane a stable descriptive name and bounded timeout.
- Preserve named validation-stage timings for the browser-facing lane.
- Publish redacted diagnostics with a lane-specific artifact name.
- Record total workflow wall time and each lane duration in the issue/PR
  evidence after the first hosted run.

## Implementation Slices

### Slice 1: Preparation Contract

- Add stack-only preparation to `scripts/run-gateway-compose-e2e.sh` without
  changing existing suite behavior.
- Add focused command-contract tests for valid and invalid invocation paths.
- Keep certificate generation, readiness checks, and optional teardown in one
  implementation.

### Slice 2: Hosted Lane Sharding

- Extract reusable workflow step sequences only where YAML clarity improves.
- Run `default` and `docker-pool` gateway suites as independent matrix jobs.
- Prepare one independent stack for the eight browser-facing stages and run
  those stages sequentially to preserve their current shared-state semantics.
- Keep diagnostics and cleanup attached to every lane.

### Slice 3: Validation And Measurement

- Run tooling unit tests and workflow/document policy checks.
- Dry-run and inspect the unchanged local full compose profile.
- Run focused local stack-preparation and representative smoke checks.
- Trigger the hosted workflow and compare its critical path with the 36-minute
  baseline.
- Decide from evidence whether a separate builder/cache issue is justified.

## Test Strategy

### Unit

- Test argument parsing for default, docker-pool, all, stack-only, teardown,
  help, missing suite value, and unknown arguments.
- Test the validation catalog still selects all nine compose stages in order.
- Test workflow policy and repository-document validation against the changed
  workflow.

### Integration

- Verify stack-only preparation generates certificates, builds/starts required
  services, and waits for Keycloak, gateway API, and MCP readiness without
  invoking Rust API tests.
- Verify the existing default, docker-pool, and all wrapper modes remain
  runnable.

### Smoke And E2E

- Hosted lane 1 runs the 16 default gateway compose API scenarios.
- Hosted lane 2 runs the four docker-pool compose API scenarios.
- Hosted lane 3 runs admin-new dashboard/projects/sessions, compatibility
  admin, CLI, MCP, recording, and workflow stages.
- Each lane exercises failure diagnostics and cleanup through the same guarded
  workflow steps.

### Coverage And Quality

- Run Node validation-tool tests for changed orchestration logic.
- Run shell syntax validation for changed scripts.
- Run repository document and GitHub workflow policy checks.
- No product coverage baseline changes are expected because product code is
  unchanged.

## Manual Test Sequence

1. Run `bash -n scripts/run-gateway-compose-e2e.sh`.
2. Run the focused orchestration tests.
3. Run `scripts/run-gateway-compose-e2e.sh --suite stack` and verify all
   readiness endpoints respond.
4. Run one browser-facing validation stage against the prepared stack.
5. Run `scripts/ci/cleanup-compose.sh` and verify test containers and volumes
   are removed.
6. Run `node scripts/validate.mjs --profile compose --dry-run` and verify the
   existing complete local sequence is unchanged.

## Documentation And Claim Impact

This slice changes CI feedback topology only. It does not raise product
maturity, production-readiness, or feature claims. Add #184 to the planning
workspace issue map and record hosted timing evidence before completion.

## Definition Of Done

- Existing compose coverage is preserved across three isolated hosted lanes.
- Validation-tool, shell, document, and workflow-policy checks pass.
- Redacted diagnostics and cleanup run independently for every lane.
- Local aggregate compose commands remain compatible.
- The first hosted run is green and its critical path is materially below the
  36-minute baseline, targeting 20 minutes or less.
- Issue #184 and this plan contain the measured result and PR evidence.

## Post-Implementation Smoke Sequence

1. `bash -n scripts/run-gateway-compose-e2e.sh`
2. `node --test scripts/validation/*.test.mjs scripts/ci/*.test.mjs`
3. `node scripts/validate.mjs --profile compose --dry-run`
4. `scripts/run-gateway-compose-e2e.sh --suite default --teardown`
5. `scripts/run-gateway-compose-e2e.sh --suite docker-pool --teardown`
6. Prepare a clean stack and run the eight non-gateway compose stages.
7. Trigger `.github/workflows/compose.yml`; verify all lanes, diagnostics
   behavior, cleanup, and total wall time.

## Evidence Record

- Baseline hosted runs: `30831720475`, `30835269073`
- Implementation PR: pending
- Post-change hosted run: pending
- Final measured critical path: pending
