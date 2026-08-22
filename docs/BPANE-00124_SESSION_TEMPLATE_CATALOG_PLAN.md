# BPANE-00124 Session Template Catalog Plan

## Metadata

- Issue: `#124`
- State: Qualified
- Owner: BrowserPane maintainers
- Lane: Operator Product
- Target gate: Phase 1 Admin-New resource-catalog checkpoint
- Depends on: ordered protocol sequence through `#268`, unless that sequence is
  explicitly blocked or exhausted by the canonical roadmap
- Last verified commit/date: `7a2fb4135d8b` / 2026-08-22

## Business Outcome

Operators can manage reusable browser-session defaults from the standard
Admin-New console without relying on raw API requests. Obsolete templates can
be archived without breaking historical session references.

## Example Use Case

A support team uses a standard customer-debug template with a known viewport,
idle timeout, labels, integration context, network identity, and recording
policy. An operator duplicates it for a new support process, validates and
saves the changed defaults, launches a session with one explicit override, and
later archives the old template while existing sessions still show its name
and id.

## Current Evidence

- The owner-scoped API and OpenAPI provide create, list, get, and replace for
  session templates; Postgres and in-memory stores maintain a version counter.
- Session creation resolves `template_id`, applies template defaults first, and
  applies explicit caller values on top. Labels and structured integration and
  network identity fields use the existing merge rules.
- The CLI supports template create/list/get/update and its smoke covers those
  operations.
- Admin-New can select templates during session creation and project policy can
  allowlist template ids, but no template catalog/detail/create/edit route
  exists.
- The API has no active/archive state, delete operation, or historical version
  collection. The version field is the current revision number only.

## Scope

- Add owner-scoped `active` and `archived` template states through the gateway,
  stores, Postgres migration, OpenAPI, and CLI.
- Add Admin-New list, detail/edit, and create routes using the established
  project, browser-context, egress-profile, and file-workspace resource
  patterns.
- Support create, replace, duplicate-as-create, archive, and reactivate. Do not
  hard-delete templates.
- Edit exactly the template defaults supported by the current API: project,
  owner mode, viewport, idle timeout, labels, integration context, network
  identity, and recording policy.
- Show current version and timestamps, validation errors, effective API payload,
  and links from session creation plus session list/detail where a template is
  referenced.
- Exclude archived templates from primary create selectors while preserving
  explicit historical rendering and owner-scoped reads.

## Non-Goals

- No immutable version-history store, diff/restore UI, bulk operations,
  cross-owner sharing, organization catalog, approval workflow, import/export,
  hard delete, or new template defaults beyond the current session-create API.
- No redesign of project allowlists, session default precedence, or the legacy
  `/admin/` compatibility app.
- No protocol, workflow, MCP, runtime-broker, or browser transport change.

## Decisions And Dependencies

- `#124` is the canonical owner and final current engineering fallback after
  `#268`; it is independent of the protocol implementation itself.
- Archive is an explicit resource state, not a label convention or delete.
  Archived templates remain readable and can be reactivated, but new session
  creation rejects them with a stable conflict response.
- Duplicate is a client operation that posts a new resource and never mutates
  the source template.
- Existing template ids in project allowlists and historical sessions remain
  readable. Project editors identify archived allowlist entries instead of
  silently removing policy data.

## Contract Changes

- API/OpenAPI: add `SessionTemplateState` (`active`, `archived`) to resource
  schemas and an optional state field to upsert. A create request without state
  defaults to `active`; an update without state preserves the stored state so
  older clients cannot accidentally reactivate an archived template. Keep
  current routes and status-code conventions. Creating a session from an
  archived template returns a stable `409` conflict.
- Protocol/event schemas: N/A; session-template management uses the owner
  control API and does not affect the browser protocol.
- Database/migrations: add a non-null state column defaulted/backfilled to
  `active`; preserve ids, versions, timestamps, defaults, and owner uniqueness.
- Admin-new: add resource navigation, list, detail/edit, create, duplicate,
  archive/reactivate, payload preview, and session cross-links.
- CLI/SDK: extend existing `session-template` CLI options/output for state and
  archive/reactivate through the existing update operation. No new browser SDK
  surface.
- Deployment/configuration: no new service, secret, environment variable, or
  runtime topology. Existing Postgres migration ordering applies.
- README/ARCH/AGENTS/operator docs: document the catalog, current-version-only
  semantics, archive behavior, and test commands where user-visible.

## Security And Data Impact

All operations remain authenticated and owner-scoped. Validate names, labels,
structured defaults, ids, state transitions, body sizes, and referenced owner
resources at the existing API boundaries. UI payload previews and errors must
not expose bearer tokens, credentials, raw browser data, or other owners'
resources. Archive and reactivation must not bypass project allowlists or grant
session capabilities.

## Migration, Compatibility, And Rollback

The database change is additive: existing rows become `active`; older creates
remain active and older updates preserve the stored state. New clients must
tolerate a server without state by treating returned templates as active during
a checked rolling overlap; archive controls remain disabled until state support
is detected. Rollback before archived rows are created is a normal binary
rollback. After archival is used, roll back forward by restoring a compatible
binary; do not drop the state column or reinterpret archived rows as active.

## Observability And Operator Feedback

Use the shared Admin-New notification pattern for load, create, update,
duplicate, archive, reactivate, validation, authentication, conflict, and
backend failures. Preserve actionable field-level validation. Gateway logs use
fixed operation/outcome wording without template names, labels, defaults,
tokens, or high-cardinality metric labels. No new metrics are required.

## Implementation Slices

1. Add state to the API/store/Postgres/OpenAPI/CLI contract with compatibility,
   archived-create rejection, and in-memory/Postgres parity.
2. Add typed Admin-New client/view models and reusable catalog/form components
   with unit and component integration tests.
3. Add list/create/detail routes, duplicate/archive/reactivate actions, session
   cross-links, and the real Compose Admin-New smoke.
4. Reconcile README, ARCH, API companion coverage, validation docs, issue, and
   recorded evidence.

## Test Strategy

### Unit

Cover parsing and rendering, every supported default, label/JSON validation,
payload generation, current-version display, state transitions, duplicate
semantics, field errors, auth recovery, conflict, and stale/missing resources.

### Integration

Run the shared in-memory/Postgres store contract and API/OpenAPI tests for CRUD,
owner isolation, duplicate names, malformed ids/defaults, version increments,
active/archive/reactivate, archived-create rejection, migration defaults, and
historical reads. Verify CLI parity and project allowlist behavior.

### Smoke And E2E

Run a headless Admin-New Compose flow that creates, edits, duplicates, selects,
launches from, archives, reactivates, and revisits historical references. Cover
invalid payloads, unauthorized access, backend failure messaging, explicit
override precedence, and cleanup. Re-run affected session-create, project,
CLI, OpenAPI, and authentication smokes.

### Coverage And Quality

Run gateway format/clippy/unit/contract coverage, OpenAPI lint/compatibility,
Admin-New test coverage/type/build, browser-client CLI unit/smoke, affected
Compose suites, dependency/repository checks, and `git diff --check`. Do not
lower existing coverage floors; record any environment-gated exclusion.

## Manual Test Sequence

1. Start local Compose, sign in to `/admin-new/`, and open Session Templates.
2. Create a template containing each supported default and verify detail,
   payload preview, version `1`, timestamps, and active state.
3. Submit invalid name, viewport, timeout, labels, JSON, referenced project, and
   recording values; verify field-level messages and no persisted mutation.
4. Edit the template and verify the version increments and list/detail refresh.
5. Duplicate it, change the name/default, and verify the source is unchanged.
6. Create a session from the duplicate, verify inherited defaults, then create
   another with one explicit override and verify precedence.
7. Archive the duplicate; verify it leaves primary selectors, remains readable
   from historical session views, and explicit new use fails with `409`.
8. Reactivate it, verify selection works again, then exercise an archived
   project-allowlist reference without silent policy mutation.
9. Expire authentication and simulate a backend error; verify global auth
   recovery and stable actionable notifications.
10. Remove disposable sessions/resources and run the targeted automated smoke
    and quality commands recorded by the implementation PR.

## Documentation And Claim Impact

Update README and ARCH only for the delivered catalog/state behavior, OpenAPI
for the exact contract, Admin-New coverage/manual-checkpoint docs, and the issue
map. This slice supports an operator-managed template catalog; it does not prove
template approvals, immutable revision history, organization sharing, or a
Production gate.

## Definition Of Done

- API/store/Postgres/OpenAPI/CLI state behavior and compatibility pass.
- Admin-New catalog, create/edit, duplicate, archive/reactivate, selectors, and
  historical references pass unit, integration, and real Compose smoke checks.
- Validation, authorization, conflict, backend-failure, rollback, and cleanup
  paths pass without weakening owner/project boundaries.
- Documentation and claims state current-version-only and archive semantics.
- Issue, plan, coverage, migration, and smoke evidence are linked and aligned.

## Post-Implementation Smoke Sequence

1. Run gateway template store/API/migration and OpenAPI compatibility tests.
2. Run Admin-New unit/component coverage, typecheck, and build.
3. Run CLI template unit and real Compose smoke coverage.
4. Run the Admin-New template catalog/create/edit/duplicate/archive smoke.
5. Run affected session-create, project-policy, auth-recovery, and historical
   session rendering smokes.
6. Verify rollback/forward-recovery, clean resources, and run repository
   validation plus `git diff --check`.

## Evidence Record

Record PR/commit, migration and old/new compatibility evidence, store parity,
OpenAPI/CLI/Admin-New results, coverage, real Compose smoke, negative/recovery
cases, screenshots where useful, documentation/claim review, and residual risk.
