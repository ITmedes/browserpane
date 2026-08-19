# BPANE-00235 Compose Runner Reliability Plan

## Target

- Canonical issue: [#235 Restore deterministic Compose CI validation](https://github.com/ITmedes/browserpane/issues/235)
- Branch: `feature/BPANE-00235`
- Outcome: the GitHub Compose workflow provides deterministic integration evidence instead of failing because required fixtures are absent, the admin contract reader is stale, or a hosted package mirror is unavailable.

## Business case

The Compose workflow is the repository-level release signal for the supported
single-node deployment. A persistent red workflow hides real product
regressions and prevents maintainers from distinguishing an implementation
failure from missing test infrastructure.

### Example use case

A session-control change is ready for review. CI must prove that the frozen API,
the unified admin, compatibility workflows, browser integrations, and both
runtime profiles still operate together. The run should fail for an actual
contract or runtime regression, but it must not depend on an unstarted proxy
fixture or an external Ubuntu package mirror.

## Verified failure causes

1. The API companion rejects the generated operation inventory because the
   valid `recording-worker` authentication domain is absent from its parser and
   presentation model.
2. The compatibility egress smoke probes the plain, authenticated, and TLS
   observer endpoints without the Compose workflow starting those fixtures.
3. The gateway API job runs `apt-get update` even though the pinned hosted
   runner already supplies the native build tools. A stalled Ubuntu mirror can
   consume the complete job timeout before validation begins.

## Implementation sequence

### 1. Align API evidence authentication domains

- Add `recording-worker` to the unified admin contract model.
- Present it as an internal, capability-scoped worker credential rather than an
  owner or session-automation token.
- Extend parser, summary, filtering, and rendered component tests with the
  current committed inventory.
- Improve the API companion smoke failure output so an evidence-parser error is
  reported directly instead of appearing as a generic authentication timeout.

### 2. Make egress fixtures explicit CI dependencies

- Add a CI helper that validates the required CA material, prepares mitmproxy,
  starts the plain/authenticated Squid observers and TLS observer on the
  canonical Compose network, and waits for all three ports from inside the
  gateway network.
- Start those fixtures only for the compatibility promotion lane.
- Extend unconditional cleanup to remove both observer projects.
- Add shell and workflow contract tests for startup, readiness, and cleanup.

### 3. Remove hosted package-mirror dependency

- Replace the gateway job package installation with fail-fast checks for the
  compiler, CMake, pkg-config, Rust, and Docker Compose prerequisites already
  supplied by `ubuntu-24.04`.
- Add a workflow contract assertion that Compose jobs do not invoke a host
  package manager.

### 4. Validate locally and on GitHub

- Run focused Node, Svelte, shell, and workflow contract tests.
- Run the API companion and egress smokes against the local Compose stack.
- Push the branch and dispatch the complete Compose workflow.
- Require every matrix lane to pass before the pull request is merged.

## Post-implementation smoke sequence

1. Run `node --test scripts/ci/compose-workflow-contract.test.mjs`.
2. Run the unified admin unit, component, type, and build checks.
3. Start the canonical stack with `scripts/run-gateway-compose-e2e.sh --suite stack`.
4. Start and verify egress fixtures with the new CI helper.
5. Run `npm run smoke:admin-unified-api-companion -- --headless --connect-timeout-ms 60000` in `code/web/bpane-client`.
6. Run `npm run smoke:admin-egress-profiles -- --headless` in `code/web/bpane-client`.
7. Run both gateway API matrices through `scripts/run-gateway-compose-e2e.sh --suite all`.
8. Dispatch `.github/workflows/compose.yml` for `feature/BPANE-00235` and require all jobs to pass.

## Completion evidence

- [x] API evidence auth model and tests aligned.
- [x] Egress fixture lifecycle and readiness checks added.
- [x] Host package installation removed from Compose CI.
- [x] Focused local validation passed.
- [x] Complete branch Compose workflow passed.

Evidence collected on 2026-08-19:

- `node scripts/validate.mjs --stage validation-tool-tests`: 117 tests passed.
- Unified admin check/build and focused API contract tests passed.
- `smoke:admin-unified-api-companion`: 131 operations loaded and exercised.
- `smoke:admin-egress-profiles`: plain, authenticated, rejected-auth, TLS,
  diagnostics, and session runtime paths passed.
- Fresh mitmproxy CA/config bootstrap passed with an unprivileged runtime and
  no pre-existing runtime state.
- [Compose workflow run 32228964102](https://github.com/ITmedes/browserpane/actions/runs/32228964102):
  all five jobs passed for commit `77c9c4b5`.
