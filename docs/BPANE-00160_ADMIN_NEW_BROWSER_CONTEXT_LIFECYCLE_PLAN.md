# BPANE-00160 Admin-New Browser Context Lifecycle Plan

## Metadata

- Issue: `#160`
- State: In Progress
- Lane: Operator Product
- Target gate: admin-new browser-context lifecycle parity
- Depends on: `#148` merged through PR `#202`
- Branch: `feature/BPANE-00160`
- Last verified: `main` at `0b65cdeeb6aac17cee908ffa33a55dea89f93972`,
  2026-08-07

## Business Outcome

An authenticated operator can move a reusable browser profile through its
supported lifecycle without leaving admin-new: clone an inactive context for a
new scenario, export an inactive context as a portable BrowserPane archive, and
import a validated archive as a new context. The UI explains active-writer,
state, persistence, and storage constraints before an action and keeps API
failures next to the action that produced them.

This closes the remaining browser-context lifecycle parity gap without copying
the old admin implementation or weakening the bounded import contract delivered
by `#148`.

## Example Use Case

A support team has a reusable context containing an authenticated vendor-portal
profile. Before changing the profile for a new workflow, an operator opens its
admin-new detail route and clones it under a new project/name. The original can
also be exported for controlled transfer. On another environment, the operator
opens the collection-level import route, selects the BrowserPane zip, chooses
scope/retention/storage metadata, and creates a new context. If the source still
has an active writer, clone/export is disabled with a direct route to the active
session. If import exceeds configured limits or capacity, the form reports the
server's bounded `413` or recoverable `429` response without losing the selected
file or metadata draft.

## Current Evidence

- Admin-new provides browser-context catalog, create, detail, refresh, and
  delete behavior under `/admin-new/browser-contexts`.
- `BrowserContextCatalogClient` has no clone, export, or import methods even
  though the frozen API provides all three operations.
- `BrowserContextEditForm` and its view model already validate name, project,
  description, labels, retention, and storage limits, but they are hard-coded to
  the create heading, default draft, and submit label.
- The detail inspector exposes copy ID, refresh, and delete. It already receives
  authoritative usage evidence including active runtime id and storage-limit
  state.
- The compatibility admin implements lifecycle controls, but combines them in a
  dense catalog panel. Admin-new should preserve the API behavior while using
  its established route-backed create/detail pattern.
- The existing admin-new smoke covers catalog/create/detail/delete only. The
  compatibility smoke covers clone/export/import and provides reusable API/E2E
  expectations.
- `#148` now authenticates and bounds import requests, validates archive
  structure, rejects links/special entries, defends Docker extraction, and
  returns JSON `400`, `413`, and `429` outcomes.

## Scope

- Add typed client contracts and methods for clone, export, and import.
- Add a pure lifecycle eligibility model for clone/export blockers and storage
  warnings.
- Generalize the existing browser-context edit form only as far as needed for
  create, clone, and import metadata reuse.
- Add a route-backed clone flow at
  `/admin-new/browser-contexts/{context_id}/clone`.
- Add a collection-level import flow at
  `/admin-new/browser-contexts/import` with native zip file selection.
- Add clone and export actions to the detail inspector with stable disabled
  reasons and active-session navigation.
- Download exports as sanitized `.zip` filenames and release object URLs.
- Preserve form/file state after `400`, `409`, `413`, `429`, network, or auth
  failures so operators can correct or retry deliberately.
- Extend unit, component/integration, smoke, documentation, and manual evidence.

## Non-Goals

- No new gateway routes, archive format, body limits, or extraction behavior.
- No client-side archive parser, decompressor, malware scanner, or attempt to
  duplicate backend safety validation.
- No context overwrite, merge, restore-in-place, or live-process migration.
- No update endpoint for existing browser-context metadata.
- No arbitrary Chromium/vendor archive formats; import accepts BrowserPane
  export archives only.
- No bulk clone/export/import or cross-owner transfer workflow.
- No removal of compatibility-admin code in this slice.

## UX And Product Decisions

- **Import is a collection action.** Add an `Import` command beside `New browser
  context` on the overview and navigate to a dedicated route. Do not place an
  upload form permanently above the catalog.
- **Clone and export are resource actions.** Present them in the detail action
  bar. Clone navigates to a dedicated prefilled route; export downloads directly
  and reports progress/result in the detail action feedback area.
- **Reuse the metadata form.** Create, clone, and import use the same validated
  fields and project options. Clone/import lock persistence to `reusable`
  because the API creates reusable contexts only.
- **Respect API blockers.** Deleted, ephemeral, or actively written contexts
  cannot clone/export. Visible inactive session references do not block these
  operations because the gateway does not block them.
- **Storage over-limit is not an export blocker.** Export remains a recovery
  path. Clone/import show a warning and allow a larger target storage limit;
  the resulting context reports over-limit state and remains blocked from new
  reusable sessions until policy permits.
- **Server limits remain authoritative.** The UI shows selected file size but
  does not hard-code configurable gateway limits. JSON `413` and `429` messages
  remain actionable and the draft stays intact.
- **No modal-only workflow.** Route-backed forms preserve browser navigation,
  refresh behavior, testing stability, and deep links.

## Contract Changes

- Gateway/OpenAPI/database/protocol: N/A.
- Admin-new client:
  - `cloneBrowserContext(contextId, request)` -> context resource,
  - `exportBrowserContext(contextId)` -> archive payload plus safe filename,
  - `importBrowserContext(request)` -> context resource using raw zip body and
    `x-bpane-browser-context-*` metadata headers.
- Admin-new routes:
  - add `/browser-contexts/import`,
  - add `/browser-contexts/[context_id]/clone`.
- CLI: N/A; existing clone/export/import commands remain regression coverage.

## Security And Data Impact

- Never inspect or extract the selected archive in the browser.
- Never persist the selected `File`, archive bytes, manifest, browser state, or
  generated object URL outside the active route.
- Send metadata only through the defined `x-bpane-browser-context-*` headers;
  labels are JSON encoded and all values use the existing field validation.
- Do not expose raw local paths from imported resources or download handling.
- Sanitize `Content-Disposition`/context names before constructing a local
  filename and always revoke generated object URLs.
- Authentication failures continue through the shared global handler.
- Disabled reasons derive from authoritative context state/usage and server
  errors remain authoritative if state changes between render and action.

## Implementation Slices

1. **Client and lifecycle model**: add request/result types, clone/export/import
   client methods, filename handling, blocker/warning derivation, and unit tests.
   Commit boundary: typed lifecycle contract.
2. **Reusable metadata form**: accept an initial draft, operation copy, locked
   persistence, submit availability, and operation-specific submit label while
   preserving create behavior and validation. Commit boundary: shared form.
3. **Clone and export detail actions**: add detail eligibility feedback, direct
   export download, and prefilled route-backed clone flow. Commit boundary:
   selected-resource lifecycle.
4. **Import route**: add collection command, file selection/size evidence,
   metadata form, preserved retry state, bounded-error feedback, and navigation
   to the imported context. Commit boundary: collection import.
5. **Battle test and handoff**: component integration, admin-new smoke,
   compatibility/CLI/gateway regressions, responsive browser checks, docs, and
   issue/PR evidence. Commit boundary: validation and promotion evidence.

## Test Strategy

### Unit

- Client URL encoding, methods, accept/content-type headers, metadata headers,
  JSON label encoding, response parsing, and auth failure propagation.
- Export filename parsing/sanitization and fallback behavior.
- Clone/export eligibility for ready/deleted, reusable/ephemeral, active/inactive
  writer, visible inactive references, and storage-over-limit states.
- Create/clone/import initial drafts, locked persistence, target metadata
  conversion, validation errors, reset semantics, and submit availability.

### Component And Integration

- Overview renders `Import browser context` and navigates to the import route.
- Detail renders clone/export, stable blocker reasons, active-session link,
  progress, success, `409`, network, and auth states.
- Clone route loads source/projects, prefills target metadata, locks persistence,
  submits the typed request, and navigates to the new detail route.
- Import route requires a file and valid metadata, shows file size/name, submits
  raw bytes/headers, preserves its draft on `400`/`413`/`429`, and navigates only
  after success.
- Export triggers one zip download with sanitized filename and revokes the object
  URL in success/failure-safe cleanup.
- Existing create/detail/delete component tests remain unchanged in behavior.

### Smoke And E2E

- Start compose and authenticate to admin-new.
- Create an inactive reusable context, open detail, clone it, and verify the new
  detail/catalog resource.
- Export the original through admin-new and verify a non-empty zip download.
- Import that download through the import route and verify the new ready context.
- Start a session with the imported context and confirm the gateway accepts the
  binding; backend compose coverage remains responsible for profile continuity.
- While a context has an active writer, verify clone/export are disabled and the
  active-session route is visible.
- Submit malformed and over-limit fixtures through the import route and verify
  inline bounded errors with no catalog residue.
- Re-run compatibility-admin and CLI browser-context lifecycle smokes.

## Manual Test Sequence

1. Start compose and sign into `/admin-new/browser-contexts` as `demo`.
2. Create a reusable owner-scoped context and open its detail route.
3. Select `Clone`, change the target name/scope or storage limit, submit, and
   confirm navigation to a distinct ready context.
4. Return to the original detail, select `Export`, and confirm a non-empty zip
   with a safe BrowserPane filename downloads.
5. Return to the catalog, select `Import`, choose the exported zip, define a new
   name/project/retention/storage limit, and submit.
6. Confirm navigation to the imported context and verify no local path or
   archive internals appear in its metadata.
7. Start a session with one context, revisit its detail route, and verify
   clone/export are disabled with the active-session reason/link.
8. Stop the session, refresh detail, and verify both operations become available.
9. Select a malformed zip and verify inline `400` feedback without losing the
   file/name draft or creating a context.
10. Exercise configured oversize and saturated-capacity cases and verify inline
    `413`/`429` feedback remains retryable.
11. Resize desktop and narrow mobile viewports; verify action bars, file input,
    field messages, and sticky submit controls do not overflow or overlap.

## Documentation And Claim Impact

- Update `README.md` because admin-new gains visible browser-context lifecycle
  controls.
- Update `ADMIN_NEW_STATUS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md`,
  `ADMIN_NEW_API_COVERAGE.md`, `RESOURCE_LIFECYCLE_REQUIREMENTS.md`,
  `DELIVERY_ROADMAP.md`, and `OPEN_ISSUES_CONTEXT.md` after validation.
- `ARCH.md` should not change unless implementation introduces a boundary not
  described here; client-side controls do not change runtime architecture.
- OpenAPI should not change because this slice consumes the frozen operations.

## Definition Of Done

- Admin-new can clone, export, and import reusable browser contexts through
  route-backed, responsive, authenticated flows.
- Operation eligibility and storage warnings match gateway behavior and remain
  race-safe through server error handling.
- Import consumes the bounded `#148` API without duplicating archive parsing or
  configurable limits in the browser.
- Download handling is filename-safe and releases object URLs.
- Unit, component/integration, admin-new smoke, compatibility smoke, CLI smoke,
  gateway browser-context, build/typecheck, and hosted validation pass.
- README/docs, `#160`, and PR evidence agree.

## Post-Implementation Smoke Sequence

1. Run browser-context client, lifecycle-model, form, clone-route, import-route,
   detail, and overview unit/component tests.
2. Run admin-new typecheck, unit suite, coverage, and production build.
3. Run admin-new browser-context smoke through create -> clone -> export ->
   import -> session binding -> cleanup.
4. Exercise active-writer blockers and malformed/oversized/backpressured import
   feedback in the browser.
5. Run compatibility-admin and CLI browser-context smokes.
6. Run focused gateway browser-context tests and compose API surface.
7. Verify desktop/mobile screenshots and no console/network errors.
8. Run repository/OpenAPI consistency checks and hosted PR validation.

## Evidence Record

- PR: pending
- Commits: pending
- Unit/component results: pending
- Smoke/E2E results: pending
- Coverage/build results: pending
- README decision: required
- ARCH decision: expected not required
