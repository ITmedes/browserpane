# BPANE-00286 Affected Compose Qualification Plan

## Metadata

- Issue: `#286`
- State: Qualified
- Owner: BrowserPane maintainers
- Lane: Foundation
- Target gate: proportional fail-closed Compose qualification
- Depends on: `#283`, `#284`, and `#285`
- Last verified commit/date: `e79164cc3a84` / 2026-08-22

## Business Outcome

Pull requests receive fast feedback proportional to the affected BrowserPane
surface while unknown or cross-cutting changes still receive complete
qualification. Documentation-only maintenance no longer starts an expensive
Compose stack after lightweight document policy succeeds.

## Example Use Case

A plan-only change under `docs/` runs Markdown links, repository-document policy,
and workflow policy without Compose. A gateway transport change runs the canary,
gateway, and browser integration plans. A new unmapped directory or a selector
error selects the full matrix instead of silently skipping tests.

## Current Evidence

- The Compose workflow runs all five jobs for every push to `main`, schedule, and
  manual dispatch, independent of changed paths.
- The full local profile contains 35 stages, including 24 admin promotion stages.
- Existing job sharding preserves broad coverage but does not expose a
  declarative capability map or stable no-Compose success path.
- #283-#285 provide the evidence, isolation, and immutable inputs required to
  make selective qualification auditable.

## Scope

- Define required canary, affected-area, and full qualification tiers.
- Add a versioned declarative path-to-capability/test-plan map.
- Add a stable required-check aggregator that succeeds only after the selected
  plan passes.
- Select lightweight document/workflow policy only for docs/plan/issue-only
  repository changes; do not start Compose for that class.
- Select full qualification for unknown paths, selector errors, workflow/build
  infrastructure, shared security/auth/protocol/runtime code, and broad changes.
- Preserve complete scheduled, release, and manual qualification.
- Keep compatibility-admin coverage for relevant changes and full scheduled/
  manual runs until a separate approved removal slice changes support.

## Non-Goals

- No test deletion, maturity promotion, or compatibility-admin removal.
- No probabilistic, AI-only, or changed-file-count-only selector.
- No automatic retry of deterministic product failures.
- No post-merge exact-tree reuse; #287 owns it.

## Decisions And Dependencies

- Selection is deterministic, reviewed, versioned, and fail closed.
- Required check names remain stable even when individual expensive jobs are not
  selected.
- Deletions, renames, empty/invalid comparisons, missing base refs, selector
  changes, and unmapped paths select full qualification.
- Docs-only means all changed paths are in the explicit low-risk allowlist and
  lightweight repository-document/workflow policy passes.
- Scheduled/manual full qualification is an independent backstop, not evidence
  that a PR selector may be permissive.

## Contract Changes

- API/OpenAPI: N/A.
- Protocol/event schemas: N/A; add an internal versioned test-plan selection
  schema compatible with #283 evidence.
- Database/migrations: N/A.
- Admin-new: no product change; selection maps admin-new and compatibility paths
  to their relevant promotion suites.
- CLI/SDK: no product change; CLI paths map to their smoke plans.
- Deployment/configuration: GitHub workflow triggers/jobs and selector config.
- README/ARCH/AGENTS/operator docs: document contributor qualification rules;
  update README only if public validation commands change.

## Security And Data Impact

- Changes to auth, authorization, secrets, protocol, runtime boundaries,
  deployment, dependencies, workflow definitions, or the selector itself select
  full qualification.
- Fork contexts cannot modify trusted selection evidence or required-check
  aggregation.
- Selection artifacts contain paths/test ids only and follow #283 bounds and
  retention; no code content, credentials, browser data, or raw logs.

## Migration, Compatibility, And Rollback

- Start in report-only mode and compare selected plans with the full matrix over
  a representative change corpus.
- Enable docs-only bypass first, then affected-area plans, while scheduled full
  runs remain active.
- Preserve stable required-check names during migration and rollback.
- A feature flag/manual input forces full qualification at any time.
- Rollback makes every change select full; it never makes a failing change pass.

## Observability And Operator Feedback

- Report comparison base/head, normalized changed paths, selector revision,
  selected tier, capability reasons, required lanes, and fallback reason.
- Stable summary distinguishes `not required` from `passed` and `failed`.
- Selector failure is visible as a full-qualification fallback, not hidden.

## Implementation Slices

1. Freeze tier definitions, capability graph, and path map with table-driven
   fixtures from recent representative changes.
2. Add deterministic selector, schema, and stable check aggregator in report-only
   mode.
3. Enable docs-only lightweight qualification and prove Compose never starts.
4. Enable affected-area qualification for narrow product surfaces.
5. Retain full scheduled/release/manual runs and record a 30-run comparison.

## Test Strategy

### Unit

- Every path family, multiple paths, deletions, renames, empty diffs, base
  failure, unknown paths, selector/workflow changes, and security-sensitive
  shared code.

### Integration

- Stable required-check aggregation for docs-only, canary, affected, full,
  cancelled, and failed selections.
- Changed test-plan/image manifests force appropriate qualification.

### Smoke And E2E

- Hosted fixtures for docs-only, admin-only, gateway-only, protocol, auth,
  deployment, multi-surface, and unknown changes.
- Full scheduled/manual run continues to execute every current lane.

### Coverage And Quality

- Require complete selector branch coverage and mutation-style fixtures for
  dropped or renamed map entries.
- Compare selected versus full outcomes in report-only mode before enforcement.
- Preserve all current scenario inventory in the union and full tier.

## Manual Test Sequence

1. Change one documentation file and verify only lightweight checks run.
2. Change one Admin-New file and verify canary plus unified-admin evidence.
3. Change gateway transport code and verify gateway plus browser evidence.
4. Add an unknown path and verify full qualification.
5. Break selector input resolution and verify full qualification.
6. Dispatch manual full qualification and verify all five lanes run.

## Documentation And Claim Impact

Update `VALIDATION_MATRIX.md`, `CURRENT_CONTEXT.md`, `DELIVERY_ROADMAP.md`,
`AGENTS.md` contributor guidance where necessary, and issue `#286`. Do not
reduce listed test coverage or claim production maturity from faster selection.

## Definition Of Done

- Selector and capability map are deterministic, versioned, and fail closed.
- Required-check aggregation is stable across every tier.
- Docs-only changes pass lightweight policy without starting Compose.
- Unknown/shared/high-risk changes select full qualification.
- Relevant compatibility-admin and all scheduled/manual coverage remain.
- Report-only comparison and hosted positive/negative evidence are linked.

## Post-Implementation Smoke Sequence

1. Run selector and workflow-contract tests.
2. Exercise docs-only and prove no Compose service or image setup begins.
3. Exercise admin, gateway, protocol, deployment, and unknown-path fixtures.
4. Verify stable required-check outcomes for success/failure/cancellation.
5. Dispatch the full workflow and compare the complete stage inventory.
6. Review 30-run selection, timing, and false-negative evidence.

## Evidence Record

Record the PR, commit, path-map revision, report-only comparison, hosted fixture
runs, full backstop run, branch-protection review, and timing results in #286.
