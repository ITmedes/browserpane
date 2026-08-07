# BPANE-00158 Admin API Companion Plan

## Issue

- Canonical issue: [#158 Add admin-new API companion, coverage, and docs routes](https://github.com/ITmedes/browserpane/issues/158)
- Delivery lane: Operator Product
- Dependency: #179 control API conformance and compatibility governance

## Business Case

BrowserPane exposes a broad owner-scoped control API, but operators and
integrators should not have to infer authentication boundaries, task sequences,
or UI coverage from a large OpenAPI file. The unified admin currently links to
API, coverage, and docs routes that do not exist. This slice turns the existing
#179 contract artifacts into an operator-facing companion without creating a
second API truth.

The result is documentation and evidence, not an in-browser generic API
console. It must not retain bearer tokens, mint worker credentials, or make
internal-worker routes look like normal operator actions.

## Example Use Case

An integration engineer needs to create a project, create a session in that
project, mint a short-lived browser connect ticket, launch a workflow run, and
inspect a file workspace. In `/admin-new/api`, they select the relevant task
flow and copy commands derived from validated contract examples. In
`/admin-new/coverage`, they verify which operations have first-class UI,
evidence-only UI, API-companion ownership, or worker-only ownership. In
`/admin-new/docs`, they confirm the owner-bearer and session-automation token
boundaries and download the exact OpenAPI contract used by CI.

## Current State

- `openapi/bpane-control-v1.yaml` is the frozen owner-scoped v1 contract.
- `openapi/bpane-control-v1.operations.json` contains all 131 recognized
  operations with method, path, family, auth, classification, and responses.
- `openapi/bpane-control-v1.classifications.json` independently enumerates the
  four classifications enforced by #179.
- `openapi/bpane-control-v1.examples.json` contains schema-validated examples.
- Redocly lint, example validation, Axum route recognition, semantic
  compatibility, and classification coverage already run in repository CI.
- `/admin-new/api`, `/admin-new/coverage`, and `/admin-new/docs` are advertised
  but missing.
- Non-OpenAPI compatibility surfaces are documented in
  `ADMIN_NEW_API_COVERAGE.md`, but are not yet represented as a machine-readable
  companion manifest.

## Source-Of-Truth Decision

1. Publish the committed OpenAPI YAML and generated JSON evidence as immutable
   web assets in the local/packaged web image.
2. Add a small checked compatibility-surface manifest under `openapi/` for
   routes intentionally outside the frozen v1 contract. Validate its shape and
   ensure none of its entries collide with frozen operation paths.
3. Load and strictly validate these artifacts in an admin-new contract client.
4. Derive families, counts, filters, badges, and copyable commands from loaded
   artifacts. Do not duplicate the 131-operation inventory in TypeScript.
5. Keep the existing OpenAPI governance scripts authoritative for schema,
   implementation, example, and compatibility enforcement.

## Implementation Plan

### 1. Publish And Validate Contract Evidence

- Publish YAML, operation inventory, classifications, examples, and the new
  compatibility manifest under `/openapi/` in `deploy/Dockerfile.web`.
- Extend repository/OpenAPI checks to require and validate the compatibility
  manifest.
- Add validated request examples for representative project, session,
  workflow-run, and file-workspace operations where the current example set is
  insufficient.
- Keep secrets, live tokens, and environment-specific identifiers out of all
  examples.

### 2. Contract Client And View Models

- Add strict types and parsers for operations, classifications, examples, and
  compatibility surfaces.
- Reject unsupported versions, duplicate operation ids, unknown
  classifications/auth modes, malformed methods/paths, classification drift,
  example references to unknown operations, and frozen/compatibility path
  collisions.
- Add view models for:
  - family/classification/auth summaries,
  - operation filtering and stable sorting,
  - task-oriented command generation,
  - compatibility-surface grouping,
  - repository enforcement evidence.
- Generate shell commands with `$BPANE_BASE_URL`, `$BPANE_OWNER_TOKEN`, and
  explicit placeholder ids. Never insert the current browser token.

### 3. API Companion Route

- Add `/admin-new/api`.
- Show high-value task groups for projects, sessions/connect, workflows, and
  file workspaces.
- Provide copy controls for contract-derived curl examples.
- Explain owner bearer auth, short-lived session connect tickets, and
  session-automation access as separate credential domains.
- Link each operation to the coverage route and each relevant first-class
  operator area to its existing admin-new route.
- Expose loading, unavailable, malformed-artifact, empty, and copy-feedback
  states without shifting the surrounding layout.

### 4. Coverage Route

- Add `/admin-new/coverage`.
- Show exact generated totals by classification, auth mode, and API family.
- Provide search plus classification/auth/family filters over all operations.
- Keep internal-worker operations visible as documentation while clearly
  marking them unavailable as normal operator actions.
- Show classification integrity and #179 repository-enforcement evidence
  without claiming that a historical/local page load is a live CI result.

### 5. Docs Route

- Add `/admin-new/docs` as a concise integration guide, not a duplicate README.
- Explain contract scope, authentication domains, request/error conventions,
  compatibility boundaries, local endpoints, and the route ownership model.
- Link the raw local OpenAPI YAML and generated evidence downloads.
- Represent compatibility, OIDC, MCP, certificate-helper, and legacy gateway
  surfaces separately from frozen v1 operations.

### 6. Shell And Navigation Alignment

- Register `/identity`, `/api`, `/coverage`, and `/docs` in the shell route
  resolver with correct active navigation and titles.
- Preserve the existing grouped navigation and direct-refresh behavior.
- Add all three routes to the admin-new smoke route manifest/auth recognizer.

### 7. Documentation And Issue Evidence

- Update `README.md`, `ADMIN_NEW_STATUS.md`, `ADMIN_NEW_REQUIREMENTS.md`,
  `ADMIN_NEW_API_COVERAGE.md`, manual checkpoints, implementation order,
  delivery roadmaps, domain requirements, and issue context with delivered
  behavior.
- Post exact validation evidence on #158 and close it through the PR.

## Test Strategy

### Unit

- Parser success plus malformed version, collection, method, path, auth,
  classification, response, example, and compatibility cases.
- Duplicate ids, cross-artifact drift, unknown operation references, and path
  collision failures.
- Family/count derivation, stable filtering, command generation, placeholder
  substitution, shell escaping, and secret-exclusion tests.
- Independent component tests for API task cards, copy feedback, coverage
  filters/table, classification summaries, docs sections, compatibility list,
  and route error/loading/empty states.

### Integration

- Route deep-link and refresh tests for `/api`, `/coverage`, and `/docs`.
- Artifact-client success, unavailable asset, malformed payload, and retry
  behavior.
- Shell resolver/navigation active state for identity and all new routes.
- OpenAPI governance tests validate every example and compatibility entry.
- Web image contract verifies all published artifact URLs and security headers.

### Compose Smoke / E2E

1. Start local compose and authenticate through Keycloak.
2. Deep-link and refresh `/admin-new/api`, `/admin-new/coverage`, and
   `/admin-new/docs`.
3. Verify exact operation/classification totals and every expected API family.
4. Filter owner, evidence, companion, and internal-worker operations; confirm
   no class is presented as another.
5. Copy and execute representative project, session, connect-ticket,
   workflow-run, and file-workspace commands using smoke-owned test resources.
6. Verify invalid authentication receives structured 401 behavior and worker
   access is not presented as an owner token.
7. Verify raw OpenAPI/evidence links return the committed content and
   compatibility surfaces are separate.
8. Verify desktop/mobile layout has no horizontal page overflow.
9. Clean up all smoke-owned resources.
10. Run OpenAPI checks, repository document checks, admin-new unit/check/build/
    coverage, focused browser smoke, and impacted compatibility regressions.

## Acceptance Criteria

- All three advertised routes load directly and after refresh.
- The UI consumes committed #179 artifacts and detects cross-artifact drift.
- Every frozen operation appears exactly once with its generated
  classification, auth mode, family, method, path, and response set.
- Representative task commands are copyable, schema-validated, token-free,
  and executable with explicit environment variables/placeholders.
- Compatibility/internal/worker surfaces cannot be mistaken for normal
  owner-scoped v1 operator APIs.
- OpenAPI and generated evidence downloads are available from the packaged web
  runtime.
- Loading, empty, unavailable, malformed, copy-success, and copy-failure states
  are tested and visible.
- Unit, integration, OpenAPI governance, compose smoke, and impacted regression
  suites pass.

## Out Of Scope

- A browser-based generic request executor or persisted API-token input.
- Replacing Redocly/OpenAPI governance with frontend logic.
- Promoting compatibility or internal-worker routes into the frozen contract.
- Implementing missing resource catalogs (#159/#124).
- Organization/project RBAC (#176), API keys/audit/retention (#70), Workflow
  Endpoints (#172), or Teach Mode (#171).
- Switching `/admin-new/` to the default admin console (#163).
