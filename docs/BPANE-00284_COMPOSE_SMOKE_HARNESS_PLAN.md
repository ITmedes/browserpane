# BPANE-00284 Compose Smoke Harness Plan

## Metadata

- Issue: `#284`
- State: Review
- Owner: BrowserPane maintainers
- Lane: Foundation
- Target gate: deterministic Compose qualification
- Depends on: `#283`; completed `#235`
- Last verified commit/date: `feadf86e1df0` / 2026-08-23

## Business Outcome

Stateful integration tests fail for product behavior, not because a prior stage
left BrowserPane resources or Docker state behind. Maintainers gain one shared,
bounded way to wait for every supported readiness boundary and to prove cleanup.

## Example Use Case

A gateway API stage creates and stops a session while runtime teardown is still
in progress. The following MCP stage receives a distinct namespace and starts
only after the harness proves the prior stage has no active CI-owned sessions,
containers, or temporary storage. A teardown timeout fails with focused evidence
instead of surfacing later as an unrelated MCP connection timeout.

## Current Evidence

- Compose smokes use several package-local wait loops and browser locators with
  different readiness semantics.
- Recent failures have included resources still in `starting`, missing MCP
  runtime containers, stale admitted clients, file chooser timeouts, and
  session-selection timeouts.
- The gateway distinguishes process readiness, runtime state, transport state,
  and worker/artifact lifecycle, but tests do not consume one shared model.
- `scripts/ci/cleanup-compose.sh` cleans the workflow boundary; it does not prove
  stage-level isolation within broad sequential smoke suites.

## Scope

- Add a shared harness for OIDC, control-plane, runtime, transport, workflow
  worker, recording worker, and artifact readiness.
- Replace arbitrary sleeps in promotion smokes with bounded state-aware waits.
- Allocate unique run/stage namespaces to CI-owned resources.
- Add stage teardown and leak assertions for active test sessions, dynamic
  containers, temporary volumes/files, and helper processes where applicable.
- Preserve primary failure evidence when teardown also fails.
- Add deterministic dependency-loss, timeout, stale-resource, and cancellation
  fixtures.

## Non-Goals

- No scenario reduction or product lifecycle behavior change.
- No global deletion of operator/developer resources.
- No image fan-out, path selector, retry policy, or evidence reuse.
- No requirement that independent hosted jobs share a live Compose stack.

## Decisions And Dependencies

- Consume the versioned result and failure taxonomy from #283.
- Readiness states are typed and ordered; HTTP reachability alone is not browser
  or worker readiness.
- Cleanup only targets resources carrying the current CI namespace.
- Timeouts are finite, stage-specific, and report the last observed safe state.
- Stage isolation is the prerequisite for meaningful affected-area execution in
  #286.

## Contract Changes

- API/OpenAPI: N/A; consume existing authenticated status APIs.
- Protocol/event schemas: N/A.
- Database/migrations: N/A; CI namespacing uses existing metadata fields or
  labels without persistence changes.
- Admin-new: no user-visible feature; test helpers may be consolidated.
- CLI/SDK: no public contract change.
- Deployment/configuration: additive test namespace and deadline variables.
- README/ARCH/AGENTS/operator docs: synchronize validation/contributor commands
  if helper entry points change.

## Security And Data Impact

- Cleanup must prove ownership by CI namespace and must fail rather than delete
  resources it cannot attribute.
- Diagnostics follow #283 redaction and size bounds.
- Test credentials remain scoped and short lived; names must not embed identity
  claims or secrets.
- Concurrent jobs must not share mutable credentials or resource namespaces.

## Migration, Compatibility, And Rollback

- Migrate one lane at a time while retaining existing assertions and scenario
  order until parity is demonstrated.
- Keep a bounded compatibility adapter for package-local helpers during the
  transition; remove it only after all call sites use the shared harness.
- Rollback restores previous wait helpers and workflow wiring. Product state and
  database schemas are unaffected.
- Cancellation/cleanup behavior must be validated before making the harness
  mandatory for every lane.

## Observability And Operator Feedback

- Emit readiness boundary, attempt count, elapsed time, last state, deadline,
  namespace, and cleanup inventory into #283 evidence.
- Never emit bearer tokens, browser content, URLs, or raw unbounded API bodies.
- Distinguish setup, readiness, product action, and teardown failures.

## Implementation Slices

1. Inventory waits, sleeps, resource names, and teardown paths across all lanes.
2. Implement and unit-test shared deadline/readiness and namespace libraries.
3. Migrate gateway and browser integration stages, including negative fixtures.
4. Migrate Admin-New, compatibility-admin, MCP, workflow, and recording stages.
5. Enforce stage cleanup invariants and remove superseded helper duplication.

## Test Strategy

### Unit

- State transitions, deadlines, aborts, last-state reporting, namespaces,
  ownership checks, cleanup inventory, and dual primary/cleanup failures.

### Integration

- Delayed readiness, dependency loss/recovery, stale resources, worker failure,
  cancellation, timeout, and partial cleanup against controlled fixtures.
- Two concurrent namespaces cannot observe or remove each other's resources.

### Smoke And E2E

- Run every existing promotion lane through the shared harness.
- Run two consecutive full workflows and prove zero cross-run CI state.
- Cancel a running workflow and prove bounded cleanup and retained evidence.

### Coverage And Quality

- Preserve the full existing promotion inventory.
- Require changed-code coverage for harness libraries and shell/static checks for
  wrappers.
- Compare first-pass reliability and timing against the #283 baseline.

## Manual Test Sequence

1. Start a clean local Compose stack and run one namespaced gateway stage.
2. Inspect the readiness evidence and CI-owned resource inventory.
3. Force delayed runtime readiness and verify a bounded typed timeout.
4. Force teardown failure and verify the primary result remains visible.
5. Rerun the same stage with a new namespace and verify no collision.
6. Run the full hosted workflow twice and compare leak/cleanup summaries.

## Documentation And Claim Impact

Update `VALIDATION_MATRIX.md` and contributor/test guidance. This improves test
determinism only; it does not promote product maturity or deployment support.

## Definition Of Done

- All Compose lanes use shared typed waits and unique namespaces.
- Arbitrary readiness sleeps in the promoted paths are removed.
- Cleanup is ownership-safe, bounded, independently reported, and enforced.
- Existing positive, denial, recovery, workflow, recording, MCP, and admin
  scenarios remain covered.
- Two consecutive full runs show no cross-run CI-owned state.
- Evidence and timing comparison are linked in #284.

## Post-Implementation Smoke Sequence

1. Run harness unit and integration fixtures.
2. Run the canonical Compose profile locally with a unique namespace.
3. Exercise delayed readiness, stale resource, cancellation, and cleanup error.
4. Run all five hosted lanes twice consecutively.
5. Verify no CI-owned sessions, containers, volumes, or helpers remain.
6. Record reliability and timing deltas against #283.

## Evidence Record

Proposal evidence for implementation commit `feadf86e1df0`:

- `node scripts/validate.mjs --stage validation-tool-tests --stage
  repository-baseline --stage repository-documents`: PASS; 164 validation-tool
  tests, 48 tracked JSON files, 125 Markdown files, 19 YAML files, and three
  workflows.
- `cargo fmt --all -- --check`: PASS.
- `cargo clippy -p bpane-gateway --all-targets --locked -- -D warnings`: PASS.
- `cargo test -p bpane-gateway --locked`: PASS; 506 gateway unit tests plus
  the crate's non-ignored integration and documentation targets.
- `node scripts/run-rust-coverage.mjs`: PASS; workspace tests and the checked
  Rust coverage ratchet passed (64.98% regions, 64.15% functions, 61.16%
  lines).
- `npx tsc --noEmit && npm test && npm run test:coverage && npm run build` in
  `code/web/bpane-client`: PASS; 91 files / 695 tests and the 93.21% statement
  coverage ratchet passed before the production build.
- Four `node scripts/compose-harness.mjs wait-http` probes against the existing
  stack: PASS for OIDC, control, runtime, and MCP health with four bounded
  readiness events.
- Live read-only cleanup inventory fixture against the existing Docker and
  Postgres dependencies: PASS in one attempt without changing foreign state.
- `node scripts/validate.mjs --stage compose-session-files`: PASS against the
  existing stack. The stage created and stopped its namespaced session; cleanup
  reported zero active sessions, owned containers, temporary volumes,
  unexpected containers, or unexpected volumes, with one intentional retained
  stopped-session data volume.

Deferred validation is explicit: the complete five-lane hosted Compose matrix
and the required consecutive-run comparison remain post-merge, scheduled, or
manual evidence because Compose is not a pull-request trigger. The canonical
local full-profile wrapper was not run because it would rebuild and ultimately
tear down a pre-existing operator stack; the ownership-safe live smoke above
was used instead. Record the two hosted run links, cancellation run, and timing
comparison in issue `#284` after that evidence exists.

## CI Convergence

Exact-head manual Compose run `32632472208` on
`f8d9d64a213dbe0d01ad4dd7f4285d9e4e4a564a` was classified as a PR regression.
The unified-admin lane exposed the injected `bpane_ci_namespace` label in its
project edit assertion. The compatibility-admin lane waited for a transient
busy state after upload instead of its deterministic success message. The
browser lane showed gateway runtime reconciliation taking about 40 seconds,
beyond the smoke helper's 15-second cap; the resulting exception skipped
browser closure and left two namespaced runtime containers. Compatibility
evidence finalization also resolved the old fixed egress project names after
fixture startup had moved to run-scoped names.

The bounded repair updates the affected admin and workflow smoke helpers,
namespaced egress identity/diagnostics resolution, and label-owned final
container cleanup. It adds or strengthens regression coverage in
`compose-harness-readiness.test.mjs`, `compose-evidence-identity.test.mjs`,
`compose-evidence-runner.test.mjs`, `compose-diagnostics-collector.test.mjs`,
and `compose-egress-fixtures-contract.test.mjs`. No product behavior, public
API, security policy, or promotion scenario changed, so no README, OpenAPI, or
product-security document update is required.

Changed files are the four affected smoke/helper modules under
`code/web/bpane-client/scripts/`; `scripts/ci/cleanup-compose.sh`; the Compose
diagnostics, identity, finalizer, plan-catalog, and new egress project-name
modules; their five focused test files; and this plan evidence record.

Local repair checks:

- `node --test scripts/ci/compose-harness-readiness.test.mjs
  scripts/ci/compose-evidence-identity.test.mjs
  scripts/ci/compose-evidence-runner.test.mjs
  scripts/ci/compose-diagnostics-collector.test.mjs
  scripts/ci/compose-egress-fixtures-contract.test.mjs`: PASS, 18 tests.
- `bash -n scripts/ci/cleanup-compose.sh
  scripts/ci/start-compose-egress-fixtures.sh`: PASS.
- `node --check` for every changed smoke, evidence, diagnostics, and project-name
  module: PASS.
- `node scripts/validate.mjs --stage validation-tool-tests --stage
  repository-baseline --stage repository-documents`: PASS, 166 tooling tests
  plus repository and document contracts.
- `npx tsc --noEmit && npm test` in `code/web/bpane-client`: PASS, 91 files and
  695 tests.
- `git diff --check`: PASS.

The shell driver owns the next exact-head hosted Compose run; this repair does
not treat the failed run as passing evidence and does not wait for that rerun.

Exact-head manual Compose run `32634536034` on
`99b172690c2be4a1fd869ef7ed3d3a918ff4172f` remained a PR regression. The
uploaded v1 evidence bound all five lanes to that commit. Both gateway lanes
passed. The unified-admin lane reproduced locally only with the new Playwright
namespace transport: a raw browser-context ZIP import was converted to text by
the generic `postData` override, so the import remained on its form while the
same smoke passed without the harness. The compatibility lane completed its
product assertions but then rejected an in-flight namespaced route after the
browser target closed. The run also reported a separate workflow cleanup
invariant timeout; this bounded transport repair does not claim to resolve or
reclassify that cleanup evidence before the next exact-head run.

The second repair keeps non-JSON request bodies on Playwright's original byte
transport, namespaces browser-context import metadata through its supported
labels header, explicitly forwards intercepted response bytes, registers clone
and import action results, and ignores only Playwright's target-closed teardown
error. Other route and network failures continue to propagate. Changed files
are `scripts/compose-harness/namespace-transports.mjs`,
`scripts/compose-harness/resource-registry.mjs`, the focused namespace contract
test, and this evidence record. No product behavior, public API, security
policy, promotion scenario, or `dev_loop/` file changed.

Second-repair local checks:

- `node --test scripts/ci/compose-harness-*.test.mjs
  scripts/ci/compose-evidence-*.test.mjs
  scripts/ci/compose-workflow-contract.test.mjs`: PASS, 46 tests.
- `node scripts/validate.mjs --stage compose-admin-new-browser-contexts`: PASS;
  the previously deterministic harness-only failure completed in 7.1 seconds.
- `node scripts/validate.mjs --stage compose-admin-compat`: PASS in 129.5
  seconds with no target-closed route rejection.
- `node --check` for both changed harness modules and the focused test: PASS.
- `node scripts/validate.mjs --stage validation-tool-tests --stage
  repository-baseline --stage repository-documents`: PASS, 169 tooling tests
  plus repository and document contracts.
- `git diff --check`: PASS.

The local full workflow stage was not run because it force-recreates the
operator's existing gateway configuration. The shell driver owns the next
exact-head hosted run and must re-evaluate its workflow cleanup inventory; this
record does not treat run `32634536034` as passing evidence.

Exact-head manual Compose run `32636333318` on
`494363001a62c4c3b2d03d73e3472bf5bbfb1042` was classified as a PR regression.
Both gateway lanes passed. The compatibility, browser-integration, and unified
admin lanes completed their product assertions and then failed the new cleanup
invariant at its fixed 15-second deadline. Uploaded diagnostics retained one
compatibility runtime, two workflow runtimes across gateway reconciliation, and
one unified-admin runtime. The browser diagnostics showed stale assignment
reconciliation continuing for about 40 seconds. The workflow-run request
interceptor also added ownership only where a nested `labels` object already
existed, so explicit `session.create_session` overrides without labels could
create project-run sessions that label-owned cleanup would not select.

The third bounded repair gives runtime teardown up to 60 seconds and requires
eight consecutive clean observations at 250-millisecond intervals before a
stage can pass. This preserves the cleanup invariant while rejecting transient
clean snapshots. It also creates the supported nested workflow-run session
labels object when needed and injects the stage namespace; existing-session
bindings remain unchanged. Changed files are
`scripts/compose-harness/cleanup-verifier.mjs`,
`scripts/compose-harness/resource-registry.mjs`, their two focused contract
tests, and this evidence record. No product behavior, public API, security
policy, promotion scenario, or `dev_loop/` file changed.

Third-repair local checks:

- `node --test scripts/ci/compose-harness-*.test.mjs
  scripts/ci/compose-evidence-*.test.mjs
  scripts/ci/compose-workflow-contract.test.mjs`: PASS, 48 tests.
- `node scripts/validate.mjs --stage validation-tool-tests --stage
  repository-baseline --stage repository-documents`: PASS, 171 tooling tests
  plus repository and document contracts.
- `node scripts/validate.mjs --stage compose-admin-new-api-companion`: PASS in
  6.3 seconds against the existing stack.
- `node scripts/validate.mjs --stage compose-admin-compat`: PASS in 160.8
  seconds. Its cleanup stayed bounded and reached a stable clean inventory
  after 98 observations and 36.285 seconds, directly reproducing the hosted
  timing that exceeded the old deadline.
- `node --check scripts/compose-harness/cleanup-verifier.mjs
  scripts/compose-harness/resource-registry.mjs`: PASS.
- `git diff --check`: PASS.

The local workflow-admission stage was not run because it force-recreates the
operator's existing gateway configuration. The shell driver owns the next
exact-head hosted run; this record does not treat run `32636333318` as passing
evidence.
