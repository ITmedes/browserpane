# BPANE-00179 Control API Conformance Plan

## Metadata

- Issue: [#179](https://github.com/ITmedes/browserpane/issues/179)
- State: Review
- Owner: `thebackplane`
- Lane: Foundation
- Target gate: Foundation Gate
- Depends on: #152 persisted session-control behavior baseline
- Branch: `feature/BPANE-00179`
- Last verified: 2026-08-07 on `feature/BPANE-00179` through `4671a8c`

## Business Outcome

Turn `openapi/bpane-control-v1.yaml` from a manually maintained promise into an
enforced compatibility boundary for admin clients, the CLI, workers, workflow
integrations, and future generated connectors. Pull requests must fail when the
document is structurally invalid, loses an implemented operation, carries an
invalid example, or introduces an unapproved breaking change.

## Example Use Case

A developer renames a required workflow-run response field while improving the
gateway. Existing admin-new code, a BPM connector, and a customer integration
still consume the frozen v1 field. The pull request must fail before merge,
identify the breaking response-schema change, and require either an additive
correction or an explicit versioned compatibility decision. A valid additive
optional field should pass without a manual exception.

## Current Evidence

- The frozen OpenAPI 3.0.3 contract contains 131 operations. A generated,
  deterministic inventory records every path, method, operation id, tag,
  security class, and consumer classification.
- A gateway test materializes every documented path and proves that the Axum
  router recognizes both the path and its documented method.
- Pinned Redocly lint, reference, operation-id, security, response, and example
  checks run from a clean lockfile-backed package.
- Fourteen representative success and shared-error request/response cases are
  validated across sessions, projects, workflows, recordings, identity, file
  workspaces, and egress.
- The pinned semantic compatibility engine compares against an explicit git
  baseline. Controlled tests prove additive operations pass and removed
  operations fail.
- Fast validation and CI contain named OpenAPI install, test, check, and
  compatibility stages. Pull requests compare with the actual base SHA.
- The compatibility and deprecation policy is published in
  `CONTROL_API_COMPATIBILITY_POLICY.md`.

## Scope

- Add a dedicated, lockfile-backed OpenAPI tooling package.
- Use Redocly CLI for OpenAPI parsing, structural validation, reference
  resolution, linting, and example validation.
- Use an established semantic OpenAPI diff engine for breaking-change
  detection; do not implement compatibility rules locally.
- Add a machine-readable operation inventory generated from the canonical
  contract, including path, method, operation id, tags, security class, and
  admin-new classification.
- Validate that every operation id appears exactly once and has exactly one
  supported classification.
- Exercise every documented route-method pair against the in-memory Axum
  router and fail when a documented operation is not recognized.
- Add representative schema conformance tests for success and error responses
  across sessions, projects, workflows, recordings, identity, files, and
  egress.
- Add an explicit compatibility policy covering additive changes, breaking
  changes, deprecation, support windows, emergency corrections, and baseline
  selection.
- Integrate deterministic OpenAPI checks into local fast validation and CI.

## Non-Goals

- No generated server implementation or broad handler annotation migration.
- No automatic mutation or repair of the canonical contract.
- No claim that schema-driven tests replace authorization, lifecycle, worker,
  Compose, browser, or domain tests.
- No new API companion UI; #158 owns that route and will consume the generated
  evidence.
- No OpenAPI 2.0 connector export or BPM endpoint design; #172 owns those
  integration profiles.
- No compatibility promise for legacy routes intentionally excluded from v1.

## Tooling Decisions

- Keep API lifecycle tooling in `scripts/openapi/` with its own exact lockfile;
  do not attach repository governance dependencies to an application package.
- Pin Redocly CLI rather than invoking `npx ...@latest`.
- Pin the breaking-change engine and execute it through a small process adapter
  that owns base-ref resolution and diagnostics, not compatibility semantics.
- Keep Node orchestration deterministic and independently unit tested.
- Use Rust only for the router-level contract because the live Axum router is
  the authoritative implementation surface.
- Compare pull requests with the GitHub base SHA. Local validation defaults to
  the merge base with `origin/main` and accepts an explicit base ref.

## Contract Changes

- API/OpenAPI: add examples and descriptions only where conformance tooling
  confirms a gap; no intentional route or wire behavior change.
- Protocol/event schemas: none.
- Database/migrations: none.
- Admin-new: no route change; generated operation evidence becomes input for
  #158 rather than a second handwritten API truth.
- CLI/SDK: no command behavior change; current clients become regression
  consumers of the protected contract.
- Deployment: no runtime topology change.
- CI: add named OpenAPI install, lint, inventory, compatibility, and router
  conformance stages.

## Security And Data Impact

- Security schemes and per-operation overrides are validated as contract data.
- Worker/session-automation operations remain distinct from owner bearer
  operations in generated evidence.
- Tests use synthetic identifiers and local in-memory or Compose resources.
- Compatibility tooling must not fetch remote references or transmit the
  contract to hosted services.
- Diagnostics must not print tokens, credentials, or authenticated payloads.

## Compatibility Policy

- Additive optional fields, operations, response codes, and enum handling are
  reviewed under the documented v1 policy.
- Removed or newly required fields, removed operations or responses, narrowed
  schemas, incompatible security changes, and content-type removal fail.
- A breaking v1 change requires a new versioned contract or a documented,
  time-bounded emergency exception approved in review.
- Deprecation is additive first and must include replacement guidance and a
  support window before removal from a future version.
- The pull-request base SHA is the compatibility baseline; release governance
  may additionally compare against the latest supported release artifact.

## Implementation Slices

### Slice 1: Governance Baseline And Standard Tooling

- Add the dedicated package, exact lockfile, Redocly config, and tested command
  adapters.
- Add lint, bundle/reference, operation-id, example, and classification checks.
- Add controlled valid, invalid, and breaking fixtures.

### Slice 2: Router And Schema Conformance

- Add an OpenAPI-derived Rust route recognition test for every operation.
- Add representative response-schema conformance for high-use resource
  families and shared error envelopes.
- Make excluded compatibility/internal routes explicit and tested.

### Slice 3: Compatibility Ratchet And CI

- Add semantic breaking-diff execution against an explicit base ref.
- Integrate OpenAPI stages into fast validation and a named CI job.
- Prove a controlled removed-field fixture fails while an additive fixture
  passes.

### Slice 4: Documentation And Evidence

- Publish the compatibility/deprecation policy and runnable commands.
- Regenerate API counts/classification evidence from the canonical contract.
- Update roadmap, validation matrix, risk state, issue, and this evidence
  record.
- Check README, ARCH, AGENTS, CLI, and admin-new documentation impact.

## Test Strategy

### Unit

- Base-ref selection, subprocess failures, and redacted diagnostics.
- Inventory generation, duplicate/missing operation ids, classification
  completeness, security-class derivation, and stable output ordering.
- Valid, structurally invalid, unresolved-reference, invalid-example,
  additive-change, and breaking-change fixtures.

### Integration

- Redocly lint and bundle the complete canonical document from a clean install.
- The semantic diff tool compares controlled base/revision documents.
- The Rust contract iterates all OpenAPI operations and proves the Axum router
  recognizes each method/path pair.
- Representative live router responses validate against documented status,
  content type, success schema, and shared error schemas.

### Smoke And E2E

- Run the in-memory API conformance suite.
- Run authenticated default and docker-pool Compose API suites.
- Execute representative documented requests for sessions, projects,
  workflows, recordings, identity, files, and egress.
- Keep admin-new, CLI, MCP, recording, and workflow admission smokes green.

### Coverage And Quality

- Add tooling unit tests to `validation-tool-tests`.
- Run Rust format, Clippy, gateway/workspace tests, and coverage ratchet.
- Run dependency safety against the new lockfile.
- Do not lower coverage floors or add an unbounded dependency exception.

## Post-Implementation Smoke Sequence

1. `npm ci --ignore-scripts --prefix scripts/openapi`
2. Run the documented OpenAPI lint and inventory commands.
3. Run compatibility checks against the merge base with `origin/main`.
4. Run the controlled additive and breaking fixtures and verify only the
   breaking fixture fails.
5. `cargo test -p bpane-gateway openapi_contract`
6. `node scripts/validate.mjs --profile fast`
7. `node scripts/validate.mjs --profile compose`
8. Execute representative documented requests for sessions, projects,
   workflows, recordings, identity, files, and egress against Compose.
9. Verify CLI and admin-new clients still pass without handwritten contract
   changes.

## Manual Test Sequence

1. Change a response schema by adding one optional field and run the
   compatibility command; verify it passes.
2. Remove one documented response field in a temporary worktree and run the
   same command; verify it identifies the operation and breaking rule.
3. Remove one route registration in a temporary worktree and run the gateway
   OpenAPI contract; verify route recognition fails.
4. Add an invalid example and unresolved `$ref`; verify lint fails with source
   locations.
5. Start Compose and execute one positive and one authorization/validation
   failure from each representative API family.
6. Confirm the observed statuses, content types, and bodies conform to the
   canonical contract.

## Definition Of Done

- The canonical OpenAPI document passes pinned structural and semantic lint.
- Every operation has a unique id, implementation route, security class, and
  admin-new classification.
- Representative success and error responses conform in-memory and in Compose.
- Breaking changes fail against an explicit supported baseline; additive
  changes pass.
- OpenAPI checks are maintained commands in fast validation and CI.
- Compatibility and deprecation policy is published.
- Existing Rust, Node, Compose, admin-new, CLI, worker, smoke, and coverage
  gates remain green.
- Issue #179 and all affected roadmap/evidence documents reflect the final
  result.

## Evidence Record

- Branch: `feature/BPANE-00179`
- PR: pending
- Implementation:
  - `7c35572` adds pinned contract tooling and generated operation evidence.
  - `b1cd228` adds semantic baseline comparison and controlled compatibility
    fixtures.
  - `eb7c1f7` adds OpenAPI-derived Axum route recognition for all 131
    operations.
  - `764aece` adds 14 representative request/response schema cases.
  - `e825a76` and `9efe1d6` add local/CI stages and exact workflow-base
    selection.
  - `4671a8c` completes governance failure-path coverage.
- Validation on 2026-08-07:
  - `node scripts/validate.mjs --profile fast`: all 40 stages passed.
  - Rust workspace tests and coverage ratchet passed; the focused OpenAPI
    gateway contract passed for all 131 operations.
  - OpenAPI tooling: 19 unit/fixture tests, structural lint, deterministic
    inventory, 14 examples, and semantic compatibility passed.
  - `node scripts/validate.mjs --profile compose`: all 10 stages passed,
    including 17 default-runtime and 4 docker-pool gateway API tests plus
    admin-new, compatibility admin, CLI, MCP, recording, and workflow smokes.
  - The recording smoke produced two non-empty WebM segments and a playback
    export; workflow capacity and project-quota runs queued and completed.
- Documentation impact:
  - README, AGENTS, validation matrix, API coverage, risk register, delivery
    roadmap, docs index, and the v1 compatibility policy are aligned.
  - `ARCH.md` is intentionally unchanged because runtime ownership, topology,
    persistence, protocol, and deployment behavior did not change.
  - The standard semantic diff dependency currently emits Node's transitive
    `DEP0169` deprecation warning; its clean audit and compatibility result are
    unaffected. Treat an upstream replacement as dependency maintenance, not
    a suppressed conformance result.
