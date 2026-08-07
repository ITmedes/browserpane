# BPANE-00152 Session Store Contract Plan

## Metadata

- Issue: [#152](https://github.com/ITmedes/browserpane/issues/152)
- State: Done
- Owner: `thebackplane`
- Lane: Foundation
- Target gate: Foundation Gate
- Depends on: #150 gateway lifecycle and dependency readiness
- Branch: `feature/BPANE-00152`
- Last verified: 2026-08-07 on `main` at `a0c67ea`

## Business Outcome

Make the persisted Postgres control plane behave like the in-memory reference
store for the resource and lifecycle operations consumed by the API,
admin-new, workers, and automation integrations. A change must fail in CI when
one backend silently changes ownership, project scope, conflict, or state
transition behavior without an explicit contract decision.

## Example Use Case

An owner creates a project, session template, reusable browser context, egress
profile, session, workflow and run, recording, service principal, and identity
mapping. The same scenario runs against the in-memory store and an isolated
Postgres schema. Both stores must expose the resources to the owner, hide them
from another owner, enforce project boundaries, reject invalid transitions,
and return equivalent domain errors. This protects admin-new from working in
unit tests while failing against the Postgres-backed local or deployed stack.

## Current Evidence

- Production-shaped Compose configures the gateway with Postgres.
- `SessionStore` dispatches every operation to separate in-memory and Postgres
  implementations.
- The `session_control/tests` tree contains 24 direct
  `SessionStore::in_memory()` setups and no shared Postgres runner.
- Compose API tests exercise representative HTTP flows against Postgres, but
  they do not apply identical behavioral vectors to both store backends.
- Existing migrations and database access already use `sqlx`,
  `tokio-postgres`, and the configured Postgres URL; no additional database
  test framework is required.

## Scope

- Add a reusable asynchronous contract suite at the `SessionStore` facade.
- Run the same contracts against a fresh in-memory store and a schema-isolated
  Postgres store.
- Cover representative create/read/list/update/state/conflict behavior for:
  projects, session templates, browser contexts, egress profiles, sessions,
  workflows and runs, recordings, service principals, and identity mappings.
- Cover owner visibility, project-bound resource isolation, lifecycle
  transitions, duplicate/idempotent behavior where promised, and sanitized
  domain-error classes.
- Add a deterministic command for the Postgres contract runner to the
  validation and CI path that already owns a Postgres service.
- Fix backend divergence found by the shared suite only when the intended
  contract is clear; document intentional differences explicitly.

## Non-Goals

- No new control-plane resource, API route, database migration, admin-new
  screen, CLI command, or product capability.
- No load, query-count, or catalog pagination benchmark; #164 owns scalability.
- No broad `session_control` structural refactor; #170 owns domain extraction.
- No replacement of the existing API and Compose E2E suites.
- No external Postgres provisioning abstraction or testcontainers dependency.

## Decisions And Dependencies

- The public `SessionStore` facade is the contract boundary. Tests must not
  compare backend-private row or state representations.
- Contract functions accept an already-created store and use unique owner and
  resource identities, so the vectors remain backend-neutral.
- The Postgres runner creates a unique schema and supplies it through the
  connection `search_path`. Existing migrations initialize that schema and
  cleanup drops it with `CASCADE`.
- The runner is explicit and ignored in ordinary unit-only execution when no
  Postgres URL is configured. CI and the Compose validation wrapper invoke it
  against the already-supported Postgres service.
- #150 is merged and provides the stable dependency-readiness baseline needed
  before persistent behavior becomes the next Foundation gate.

## Contract Changes

- API/OpenAPI: N/A; behavior is pinned without changing routes or schemas.
- Protocol/event schemas: N/A; no wire change.
- Database/migrations: N/A unless a confirmed parity defect requires a
  forward-compatible correction; isolated schemas run the current migrations.
- Admin-new: N/A; existing views gain backend regression protection only.
- CLI/SDK: N/A; no command surface changes.
- Deployment/configuration: add a test-only Postgres URL input and validation
  command; runtime configuration remains unchanged.
- README/ARCH/AGENTS/operator docs: update only the runnable validation command
  and store-test ownership where useful.

## Security And Data Impact

- Contracts explicitly exercise owner and project isolation, which are the
  principal multi-tenant boundaries in this store.
- Test schemas use generated safe identifiers and are dropped after execution.
- Database URLs and backend error details must not be printed in assertion or
  cleanup failures.
- Tests use synthetic metadata only and do not access Vault, browser profiles,
  file artifacts, recordings, or external egress.
- No new secret, URL, retention, or user-data surface is introduced.

## Migration, Compatibility, And Rollback

- The test harness is additive and does not alter persisted production data.
- A parity correction must preserve the documented API behavior and use an
  additive migration if schema changes become necessary.
- Removing the harness reverts only validation coverage. Any discovered store
  correction must be reviewed and rolled back independently.
- A failed or interrupted Postgres test can leave only a uniquely prefixed
  test schema; the fixture and documented cleanup command remove such schemas.

## Observability And Operator Feedback

- The runner reports the backend and contract group that failed without
  printing credentials or full connection strings.
- CI exposes the Postgres contract suite as a named validation step.
- No runtime metrics, traces, alerts, admin messages, or health semantics
  change in this slice.

## Implementation Slices

### Slice 1: Contract Harness And Isolated Postgres Fixture

- Add backend-neutral contract runner helpers.
- Add in-memory and schema-isolated Postgres store factories.
- Add deterministic cleanup and missing-configuration errors.
- Commit the harness before adding broad contract vectors.

### Slice 2: Core Resource And Isolation Contracts

- Add projects, templates, contexts, egress, and sessions.
- Verify owner visibility, project binding, conflict, and lifecycle behavior.
- Correct and document any confirmed backend divergence.

### Slice 3: Automation And Identity Contracts

- Add workflows/runs, recordings, service principals, and identity mappings.
- Verify idempotency/state transitions and cross-owner denial.
- Correct and document any confirmed backend divergence.

### Slice 4: Validation Integration And Evidence

- Add the Postgres contract command to the supported validation path.
- Run focused, gateway, workspace, coverage, Compose, and impacted admin-new
  regression checks.
- Update roadmap, validation matrix, issue, and this evidence record.

## Test Strategy

### Unit

- In-memory contract runner executes in the ordinary gateway test suite.
- Fixture URL/schema parsing rejects missing, malformed, and unsafe input.
- Contract comparison helpers classify expected conflict, not-found, and
  invalid-request outcomes without relying on backend error strings.

### Integration

- The identical contract groups run against a migrated isolated Postgres
  schema.
- Tests cover successful lifecycle paths plus wrong owner, wrong project,
  duplicate resource, missing resource, and invalid transition cases.
- Schema setup and cleanup are repeatable across consecutive runs.

### Smoke And E2E

- Run the existing authenticated default and docker-pool Compose API suites.
- Run admin-new project/session/workflow/recording catalog smokes because they
  consume the protected Postgres behavior.
- Verify `/healthz` and `/readyz` recover after the contract runner without a
  gateway restart.

### Coverage And Quality

- Run Rust formatting and changed-code Clippy.
- Run gateway and workspace tests plus the Rust coverage ratchet.
- Run dependency and repository-document checks.
- Do not lower coverage floors or introduce an audit exception.

## Manual Test Sequence

1. Start the local Compose Postgres service and wait until it is healthy.
2. Run the in-memory contract test command and verify every contract group
   passes.
3. Run the Postgres contract command with the local test database URL.
4. Run the same Postgres command a second time to prove schema isolation and
   cleanup are repeatable.
5. Open admin-new and create/list/update a project and session against the
   Postgres-backed gateway.
6. Log in as or mint a principal for another owner and verify those resources
   are not visible.
7. Run the default Compose API suite and confirm gateway `/readyz` remains 200.
8. Inspect Postgres and confirm no schema with the contract-test prefix remains.

## Documentation And Claim Impact

- Mark #150 Done and #152 Review in the canonical roadmap.
- Record the exact Postgres contract command in `VALIDATION_MATRIX.md` and
  contributor guidance if it becomes a maintained command.
- Do not change external product or production-readiness claims; this slice
  increases evidence for existing behavior.

## Definition Of Done

- One shared suite validates both stores.
- All scoped resource, ownership, project, lifecycle, and negative contracts
  pass against in-memory and Postgres.
- Intentional backend differences, if any, are documented.
- The Postgres suite is part of a maintained CI/full-validation path.
- Existing gateway, Compose, admin-new, worker, and coverage gates remain green.
- Issue #152, roadmap, validation matrix, and this plan contain final evidence.

## Post-Implementation Smoke Sequence

1. `cargo test -p bpane-gateway session_store_contract`
2. Start local Postgres with `docker compose -f deploy/compose.yml up -d postgres`.
3. Run the documented Postgres contract command twice with the local test URL.
4. `cargo test -p bpane-gateway`
5. `cargo test --workspace`
6. Run the default and docker-pool Compose API suites.
7. Run impacted admin-new project, session, workflow, and recording smokes.
8. Run the full validation profile and verify repository coverage ratchets.
9. Confirm `/healthz` and `/readyz` return 200 and no contract-test schema remains.

## Evidence Record

- Branch: `feature/BPANE-00152`
- PR: [#193](https://github.com/ITmedes/browserpane/pull/193), merged
- Contract implementation:
  - `208bbbf` adds the backend-neutral harness and isolated Postgres fixture,
  - `dddfa8b` covers core resources and tenant boundaries,
  - `6ac0b0b` corrects persisted session lifecycle parity,
  - `a7ff52b` covers workflow/run and recording behavior,
  - `0ac522c` preserves fixture cleanup on contract failures,
  - `bbb8e71` integrates the Postgres suite into Compose validation,
  - `17cd550` removes an unnecessary OIDC reload from the impacted workflow
    responsive smoke,
  - `7532888` preserves the full session contract during stale runtime
    assignment recovery and adds the recovery path to both store contracts,
  - `dabf59e` makes the project-quota workflow dispatch test wait for worker
    capture content instead of racing shell file creation under coverage.
- Contract results:
  - in-memory contract: passed,
  - Postgres contract: passed repeatedly against local Compose Postgres,
  - isolated schema cleanup: passed; no prefixed schema remained,
  - compose-wrapper shell and Node contract tests: 5 passed.
- Gateway and workspace results:
  - gateway: 396 passed, one intentionally ignored Postgres contract,
  - workspace: gateway, host, protocol, integration, property, wire-fixture,
    and documentation tests passed,
  - Clippy: passed with 15 pre-existing gateway warnings.
- Compose and admin-new results:
  - persisted-store contract passed inside the maintained Compose wrapper,
  - default API E2E: 17 passed,
  - docker-pool E2E: 4 passed,
  - an injected stale runtime assignment was cleared on gateway restart while
    the project-scoped session remained ready,
  - admin-new projects, sessions, workflows, workflow runs, and recording
    smokes passed,
  - recording smoke finalized and downloaded two valid WebM segments plus the
    playback export.
- Full validation:
  - canonical fast profile: all 36 stages passed,
  - canonical compose profile: all 10 stages passed in one uninterrupted run,
  - together these cover the same 46-stage catalog as the full profile,
  - Rust workspace line coverage: 56.88%, floor 54.80%,
  - admin-new: 278 tests and all coverage floors passed,
  - browser client: 661 tests and all coverage floors passed.
- Runtime cleanup and health:
  - gateway `/healthz` and `/readyz` returned healthy/ready after validation,
  - no `bpane_store_contract_*` schema remained.
- Documentation impact: contributor validation guidance and this matrix were
  updated. README, ARCH, OpenAPI, CLI, and runtime manifests require no change
  because the public product and deployment contracts are unchanged.
- Reviewed risks: isolated schema cleanup, credential redaction, owner/project
  isolation, backend error classification, and stateful smoke cleanup.
