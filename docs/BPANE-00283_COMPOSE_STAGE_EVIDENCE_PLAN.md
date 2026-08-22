# BPANE-00283 Compose Stage Evidence Plan

## Metadata

- Issue: `#283`
- State: Ready
- Owner: BrowserPane maintainers
- Lane: Foundation
- Target gate: reliable Compose qualification
- Depends on: completed issues `#184`, `#185`, `#235`, `#273`, and `#277`
- Last verified commit/date: `e79164cc3a84` / 2026-08-22

## Business Outcome

Maintainers can identify where and why a hosted Compose qualification failed
without rerunning the complete workflow or reading unbounded interleaved logs.
This evidence baseline must exist before BrowserPane changes retry, image,
selection, or post-merge behavior.

## Example Use Case

A browser-integration lane times out while opening a managed session. Its
summary identifies the failed stage, elapsed time, last reached readiness
boundary, failure class, tested tree and image digests, retained Playwright
evidence, and cleanup result. The maintainer can distinguish a product defect
from a harness or runner failure and run the focused reproduction command.

## Current Evidence

- `.github/workflows/compose.yml` runs five parallel jobs: two gateway API
  variants, browser integrations, Admin-New promotion, and compatibility-admin
  promotion.
- The latest reviewed green run took about 18 minutes. Each job independently
  resolves the CI builder and prepares overlapping Compose images and services.
- The last 30 reviewed runs contained product defects, readiness races,
  selector/locator timeouts, and cancellations, but no common structured
  failure taxonomy or stage result artifact.
- `scripts/collect-compose-diagnostics.mjs` and
  `scripts/ci/cleanup-compose.sh` provide bounded diagnostics and teardown,
  while the local validation runner already has named stages and deadlines.
- #184 split the lanes, #185 added the deterministic Rust builder, #235 repaired
  earlier runner fixtures, #273 moved full evidence to the exact PR head, and
  #277 added one bounded same-run retry. None owns a shared evidence schema.

## Scope

- Define a versioned, bounded Compose stage-result schema.
- Record stage identity, elapsed time, outcome, failure class, reproduction
  command, tree/test-plan/image identity, retry attempt, and cleanup result.
- Classify failures as `product`, `harness`, `infrastructure`, or `unknown`.
- Publish lane summaries as JSON and JUnit plus bounded relevant traces,
  screenshots, and redacted diagnostics.
- Build a 30-run baseline and a reviewable flake ledger.
- Record engineering targets without presenting them as product SLOs.

## Non-Goals

- No test removal, path-based selection, build-once image fan-out, or
  post-merge evidence reuse.
- No automatic retry policy change.
- No product API, protocol, Admin-New, CLI, or runtime behavior change.
- No public telemetry or long-term observability backend.

## Decisions And Dependencies

- `unknown` is a terminal failure, never a reason to pass or retry.
- Cleanup failure is recorded independently and cannot replace the primary
  failure.
- Classification uses deterministic evidence and exit semantics, not log-text
  guesses alone.
- Initial engineering targets are: canary <=8 minutes, affected qualification
  <=12 minutes, full qualification <=15 minutes median, and >=95% first-pass
  success over 30 unchanged scheduled runs.
- #284 consumes this schema for readiness and cleanup evidence. #285-#287 use
  the same identity fields for images, selection, and reuse.

## Contract Changes

- API/OpenAPI: N/A; CI-only evidence.
- Protocol/event schemas: N/A; no product wire change. Add a versioned internal
  CI artifact schema.
- Database/migrations: N/A.
- Admin-new: N/A.
- CLI/SDK: N/A; reproduction commands may invoke existing scripts only.
- Deployment/configuration: additive GitHub workflow and script configuration.
- README/ARCH/AGENTS/operator docs: update only when commands or contributor
  expectations change; synchronize `VALIDATION_MATRIX.md` and roadmap docs.

## Security And Data Impact

- Artifacts must exclude credentials, bearer tokens, identity claims, browser
  content, requested URLs, proxy credentials, CA material, and unbounded logs.
- Resource identifiers must be omitted or transformed into run-local bounded
  correlation values where diagnosis requires them.
- Artifact paths, names, sizes, and retention must be allowlisted and bounded.
- Fork/untrusted jobs must not gain write access to trusted artifact or registry
  namespaces.

## Migration, Compatibility, And Rollback

- Add evidence alongside existing job output before making it a dependency of
  later qualification logic.
- Preserve current job names, required checks, scenarios, and exit behavior.
- Rollback removes artifact publication and wrappers without changing product
  code or the existing Compose scenario inventory.
- Schema changes require an explicit version bump; readers reject unsupported
  versions instead of inferring fields.

## Observability And Operator Feedback

- Each stage reports fixed-cardinality lane, stage, outcome, failure class,
  attempt, elapsed milliseconds, cleanup status, and artifact references.
- Human summaries identify the first primary failure and focused rerun command.
- Workflow summaries show total setup, test, diagnostics, and cleanup time.
- The baseline report separates cancellations from first-pass failures and
  identifies successful and unsuccessful reruns.

## Implementation Slices

1. Freeze the evidence schema and inventory every current hosted Compose stage.
2. Add tested result/timing writers around the shared scripts and workflow jobs.
3. Add bounded deterministic failure classification and cleanup accounting.
4. Publish JUnit/JSON and selected redacted artifacts on success, failure, and
   cancellation.
5. Collect the 30-run baseline, document findings, and synchronize issue/plan
   evidence without changing gate selection.

## Test Strategy

### Unit

- Schema validation, timing serialization, size limits, path allowlists,
  redaction, summary aggregation, and every failure class.
- Missing, malformed, and unknown evidence remains failing.

### Integration

- Controlled product, harness, infrastructure, unknown, timeout, cancellation,
  and cleanup failures publish the expected bounded result.
- Primary and cleanup failures remain separately visible.

### Smoke And E2E

- Run all five hosted Compose lanes once and verify each summary/artifact.
- Trigger one controlled non-product failure and one product fixture failure;
  verify distinct classifications and focused reproduction guidance.

### Coverage And Quality

- Preserve all existing Rust, Node, workflow-policy, and Compose coverage.
- Add changed-code tests for all scripts and serializers.
- Run repository document/workflow policy, shell/static checks, and the affected
  hosted workflow contract tests.

## Manual Test Sequence

1. Dispatch the full Compose workflow for a clean exact tree.
2. Open every lane summary and verify stage timings, identity, and cleanup.
3. Download JSON/JUnit artifacts and validate them against the checked-in
   schema.
4. Trigger the controlled harness failure and verify no sensitive data appears.
5. Use the reported focused command and confirm it reproduces the failing stage.
6. Remove the fixture and verify the unchanged full inventory passes.

## Documentation And Claim Impact

Update `VALIDATION_MATRIX.md`, `DELIVERY_ROADMAP.md`, `CURRENT_CONTEXT.md`, and
`OPEN_ISSUES_CONTEXT.md`. Do not change capability maturity or investor claims:
this is delivery reliability evidence, not a new product capability.

## Definition Of Done

- All current lanes emit schema-valid bounded evidence.
- Every failure class and cleanup combination has deterministic test coverage.
- Existing scenario coverage, required names, and fail-closed outcomes remain.
- One full hosted run and controlled negative evidence are linked in #283.
- The 30-run baseline and targets are recorded without claiming an SLO.
- #284 can consume the evidence contract without redefining it.

## Post-Implementation Smoke Sequence

1. Run workflow/static and schema unit tests.
2. Dispatch full Compose against the exact branch head.
3. Verify all five lane artifacts and summaries.
4. Exercise controlled product, harness, unknown, and cleanup failures.
5. Confirm secrets/browser content are absent and bounds are enforced.
6. Restore fixtures and obtain one complete green exact-head run.

## Evidence Record

Record the PR, commit, workflow run, five lane artifacts, controlled-negative
run, redaction review, coverage result, and 30-run baseline in issue `#283`.
