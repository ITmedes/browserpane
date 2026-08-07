# BPANE-00153 Admin-New Pattern Consolidation Plan

## Metadata

- Issue: [#153](https://github.com/ITmedes/browserpane/issues/153)
- State: In Progress
- Owner: `thebackplane`
- Lane: Operator Product
- Target gate: Admin-New Phase 1 Promotion
- Depends on: #179 control API conformance and compatibility governance
- Branch: `feature/BPANE-00153`
- Last verified: 2026-08-07 on `feature/BPANE-00153`

## Business Outcome

Make `/admin-new/` cheaper and safer to extend by giving its resource clients
and route actions one consistent, tested foundation. Operators should receive
the same useful loading, validation, conflict, authentication, and backend
failure behavior regardless of whether they manage a project, browser context,
egress profile, file workspace, workflow, session, recording, or workflow run.

This is an enabling slice for the remaining admin-new routes. It must reduce
proven duplication without replacing domain language, validation, or view
models with an abstract resource framework.

## Example Use Case

An operator edits a project and the gateway rejects the update with HTTP 409
because a referenced policy resource changed. The project page keeps its
layout stable, shows the gateway's safe conflict explanation next to the save
action, and leaves the entered values available for correction. The same
transport and feedback rules apply when an egress profile or file workspace is
edited, while each route keeps its own fields and domain-specific recovery
guidance. An expired login still enters the existing global OIDC recovery flow
exactly once.

## Current Evidence

- Nine admin-new API clients repeat access-token lookup, bearer-header
  injection, 401 handling, status checks, and local request types.
- The resource clients already keep payload mappers and validation close to
  their domains; those boundaries should remain independent.
- `AdminMessage` and `FieldFeedback` already provide the visual and accessible
  primitives. Route components still repeat action-state unions and rendering
  branches around them.
- Catalog routes share a load/refresh shape, but file workspaces and later
  routes have domain-specific enrichment. A generic catalog component would
  currently hide rather than remove complexity.
- The old admin's `authenticated-api` is useful behavioral evidence, but
  importing it would couple the new app to the app being replaced.

## Scope

- Add one admin-new authenticated HTTP transport with:
  - nullable asynchronous token-provider support,
  - bearer-header injection without URL credentials,
  - safe structured gateway error parsing,
  - explicit 401 authentication-failure notification,
  - JSON, raw-body, binary-response, and empty-response compatibility,
  - injected `fetch` support for deterministic tests.
- Migrate project, browser-context, egress-profile, file-workspace, workflow,
  workflow-run, session, recording, and MCP bridge clients to that transport.
- Preserve domain-specific error class names and invalid-payload errors while
  exposing normalized HTTP status, API code/category, and recovery guidance.
- Add shared DOM-free load/action state types and a focused action-feedback
  component built on `AdminMessage`.
- Migrate proven repeated create/detail action feedback without changing route
  URLs, test ids, field ownership, or resource-specific copy.
- Consolidate smoke helpers and selector data only where at least two current
  scripts perform the same operation.
- Document validation evidence and update admin-new status/coverage records.

## Non-Goals

- No universal resource client, schema mapper, table, form, or route component.
- No change to control API routes, payloads, OpenAPI, database schemas, or
  runtime topology.
- No new route or parity feature; #154 and later focused issues own those.
- No replacement of `bpane-admin-auth` or custom OIDC/token implementation.
- No removal or promotion of the compatibility admin app in this slice.
- No raw backend body, bearer token, credential, or secret in user-facing or
  diagnostic messages.

## Design Decisions

- Keep authentication lifecycle in `bpane-admin-auth`; the transport only asks
  the provided access-token function and reports an HTTP 401 through the
  existing failure callback.
- Use the platform Fetch, Headers, URL, Response, JSON, and AbortSignal APIs.
  A third-party HTTP client would add weight without replacing domain mapping
  or Svelte state behavior.
- Parse the gateway's documented error envelope defensively. Preserve a short
  plain-text fallback, but cap retained error-body size and never echo response
  headers or request data.
- Let each domain error extend a shared HTTP error so existing `instanceof`
  checks and resource-specific copy remain available.
- Keep feedback local to the action that produced it. Use fixed minimum space
  only where forms otherwise shift, and do not turn static notes into live
  regions.
- Extract components only after identical behavior is demonstrated by current
  tests. Similar-looking domain forms remain separate.

## Contract And Security Impact

- API/OpenAPI: none.
- Authentication: no token-storage or OIDC-flow change; missing tokens and 401
  responses retain the global recovery contract.
- Error data: only `error`, `code`, `category`, and `recovery_hint` string
  fields from bounded response bodies are retained.
- Browser security: authorization remains a request header and is never added
  to URLs, selectors, logs, or rendered payloads.
- Protocol/database/runtime: none.

## Implementation Slices

### Slice 1: Shared Transport And Error Contract

- Add the shared types, error parser, authenticated request function, and
  focused tests.
- Cover missing token, network failure, abort, 2xx success, 204, structured and
  plain errors, oversized bodies, 401 callback, and callback failure isolation.
- Migrate each domain client while retaining its exported client/error API.

### Slice 2: Shared Feedback And Route State

- Add typed DOM-free load/action state primitives.
- Add an action-feedback renderer that preserves current test ids and
  accessible roles.
- Migrate repeated create/detail action sections and test idle, running,
  success, validation/conflict, and unexpected errors.
- Verify selector/options load errors stay dedicated to their controls.

### Slice 3: Proven Catalog And Smoke Consolidation

- Audit catalog route constructors and smoke scripts after transport migration.
- Extract a client factory or helper only if it removes identical setup without
  hiding resource fetch/enrichment logic.
- Centralize shared admin-new routes/selectors used across multiple smokes and
  retain route-specific assertions.

### Slice 4: Evidence And Promotion Readiness

- Run admin-new unit, coverage, check, and production build gates.
- Run every current admin-new smoke against Compose, plus focused negative
  auth/conflict/backend-error checks.
- Inspect desktop and narrow layouts for stable local feedback.
- Update status, API coverage, roadmap, issue evidence, README/ARCH impact, and
  this plan's evidence record.

## Test Strategy

### Unit

- Shared transport request construction, response handling, safe errors,
  authentication notification, abort/network behavior, and body modes.
- Existing client mapper tests plus migrated client delegation tests.
- Shared action/load states and action-feedback accessibility/rendering.
- Every affected route keeps independent component and view-model tests.

### Integration

- Resource clients exercise representative GET, POST, PUT, DELETE, file
  upload/download, and empty response operations through the shared transport.
- Route tests exercise loading, success, validation, conflict, missing-token,
  and unavailable-backend paths using injected clients/fetch where available.
- Auth tests confirm one 401 reaches the existing global recovery callback and
  does not expose token or response headers.

### Smoke And E2E

- Run dashboard, projects, browser contexts, egress profiles, file workspaces,
  workflows, workflow runs, sessions, and recordings admin-new smokes.
- Exercise representative create/edit/refresh actions and preserve values after
  a conflict.
- Expire or replace the admin token and verify automatic authentication
  recovery instead of a silent panel failure.
- Stop the gateway for one manual refresh and verify local visible feedback and
  layout stability when the backend is unavailable.

### Coverage And Quality

- Keep admin-new statement/function/line coverage at or above the current
  ratchet and improve branch coverage around HTTP/error states.
- Run TypeScript/Svelte checks and the production build.
- Run dependency and repository validation stages affected by the package.
- Do not add a dependency exemption or duplicate token implementation.

## Post-Implementation Smoke Sequence

1. `cd code/web/bpane-admin-unified && npm ci`
2. `npm run check`
3. `npm test`
4. `npm run test:coverage`
5. `npm run build`
6. Start or verify local Compose and log into `/admin-new/` through Keycloak.
7. Run all `smoke:admin-unified-*` scripts from `code/web/bpane-client` with
   `--headless`.
8. Create and edit one project, browser context, egress profile, and file
   workspace; verify action-local loading/success feedback does not move or
   hide controls.
9. Exercise one 409/validation response, one unavailable-backend refresh, and
   one expired-token request; verify useful local feedback and global auth
   recovery.
10. Check the same representative forms at desktop and narrow viewport widths.
11. Run `node scripts/validate.mjs --profile fast`.

## Definition Of Done

- All admin-new clients use one tested authenticated transport and retain their
  domain-specific mapping/error surfaces.
- Missing-token, 401, common 4xx, 409, 410, 429, 5xx, network, abort, invalid
  payload, binary, and empty-response behavior is covered.
- Repeated route action feedback uses shared typed state and rendering without
  losing resource-specific messages or test selectors.
- Existing admin-new routes and smokes remain green; representative negative
  states are visible and accessible.
- No token/auth implementation, generic resource framework, or old-admin
  dependency is introduced.
- README, ARCH, AGENTS, admin status/coverage docs, roadmap, and issue evidence
  are checked and updated where required.

## Evidence Record

- Pending implementation.
