# BPANE-00154 Workflow Run Detail Plan

## Metadata

- Issue: [#154](https://github.com/ITmedes/browserpane/issues/154)
- State: Review
- Owner: `thebackplane`
- Lane: Operator Product
- Target gate: Admin-New Phase 1 Promotion
- Depends on: #153 shared admin-new transport, feedback, shell, and smoke patterns
- Branch: `feature/BPANE-00154`
- Last verified: 2026-08-07 on `feature/BPANE-00154`

## Business Outcome

Give operators one durable workflow-run URL for diagnosis and intervention.
The page must answer what ran, against which workflow/session/project, what the
executor reported, what evidence it produced, whether operator action is
pending, and which controls are currently valid without requiring raw API
calls or the compatibility admin.

This is the first route built on the consolidated #153 foundations. It should
demonstrate that the new admin can add a rich operational detail surface while
keeping API mapping, state rules, presentation, and orchestration separately
testable.

## Example Use Case

A business-process workflow reaches an approval step and enters
`awaiting_input`. An operator opens the stable run URL from the catalog,
reviews its input, source snapshot, recent events and logs, checks the linked
browser session, and submits structured approval input. The run resumes and
later succeeds with a generated report. From the same route the operator can
refresh the final state and download the produced report. If the run instead
fails, its terminal error and last intervention decision remain visible.

## Current Evidence And Gaps

- `/admin-new/runs` provides a useful catalog but rows link only to the related
  workflow and session; there is no run detail route.
- `WorkflowRunCatalogClient` currently supports list/create only. The gateway
  already exposes get, events, logs, produced files, cancel, submit-input,
  resume, reject, and produced-file download endpoints.
- The current admin-new mapper omits contract fields needed by a complete
  inspector: source snapshot, applied extensions, credential-binding metadata,
  workspace inputs, recordings, retention, and last intervention resolution.
- The compatibility admin has a run detail implementation and smoke fixture,
  but the new app must use its own #153 transport, feedback, component, and
  route patterns.
- Product requirements prefer `/admin-new/workflow-runs`; `/admin-new/runs`
  is already linked and must remain compatible during migration.

## Scope

- Extend admin-new workflow-run types and strict mappers to the complete safe
  owner-visible `WorkflowRunResource` contract.
- Add client methods for run detail, events, logs, produced-file metadata and
  content, cancel, submit input, resume, and reject.
- Add DOM-free detail/evidence/action view models with explicit state gating.
- Add canonical `/admin-new/workflow-runs` and
  `/admin-new/workflow-runs/[run_id]` routes.
- Keep `/admin-new/runs` and `/admin-new/runs/[run_id]` as compatible aliases
  backed by the same components; update navigation and catalog links to the
  canonical resource route.
- Show metadata, timestamps, project/admission, workflow/session references,
  source snapshot, applied extensions, credential bindings, workspace inputs,
  recordings, retention, labels, input, output, error, events, logs, and
  produced files.
- Provide session detail and popup-preview links plus the workflow detail link.
- Add cancel, submit-input, resume, and reject controls only when valid for the
  current run state. Preserve operator input after recoverable failures.
- Download produced files through a same-origin path constructed from the run
  and file identifiers; never send the bearer token to a metadata-provided
  absolute URL.
- Extend admin-new and compatibility-admin regression smoke coverage.

## Non-Goals

- No control API, OpenAPI, database, worker, or runtime lifecycle change.
- No workflow definition editor or run creation redesign.
- No generic inspector framework or dependency on compatibility-admin code.
- No source snapshot, workspace input, recording, or log deletion controls.
- No live event-stream subscription in this slice; explicit refresh is the
  stable baseline. Broader event-driven admin synchronization remains separate.
- No credential secret resolution or display. Only safe binding metadata may
  be shown.

## Contract And Security Decisions

- Treat `openapi/bpane-control-v1.yaml` and gateway responses as the resource
  contract. Invalid required fields fail visibly instead of being silently
  omitted.
- Use the #153 `AuthenticatedApiClient` for every request and retain the global
  401 recovery behavior.
- Construct operation and download paths from encoded identifiers. Do not trust
  response-provided paths as bearer-token destinations.
- Render arbitrary input, output, event data, provenance, and error content as
  text/JSON only. Do not interpret HTML.
- Evidence endpoints load independently after the core run. A failed logs,
  events, or produced-file request is local to that section and does not erase
  successfully loaded run metadata.
- Controls derive from the current run state and pending intervention request,
  not from optimistic UI assumptions. A backend 409 remains authoritative.
- Keep complete evidence available, but use bounded scroll regions and concise
  summaries so large logs and JSON values do not expand the whole page.

## Implementation Slices

### Slice 1: Complete Run Contract And Client

- Add the omitted safe workflow-run resource types and mappings.
- Add detail/evidence/control/download client methods.
- Test URL encoding, auth propagation, body contracts, binary content, 401,
  404/409/410/503, malformed payloads, nullable fields, and complete resources.

### Slice 2: Route And Detail View Models

- Add detail load/evidence/action state and pure state-gating/formatting models.
- Add the canonical and compatibility route wrappers.
- Link catalog rows to the canonical detail route and update shell route titles.
- Test direct deep links, refresh, missing IDs, and both route families.

### Slice 3: Inspector And Operations

- Build focused metadata, controls, structured data, event/log, produced-file,
  and related-resource components.
- Keep action feedback adjacent to the controls and evidence failures adjacent
  to the affected section.
- Add produced-file download and related session preview links.
- Verify desktop and narrow layouts, keyboard access, live-region behavior,
  long IDs/messages, empty evidence, and partial evidence failures.

### Slice 4: Smoke, Regression, And Documentation Evidence

- Extend the unified smoke to cover catalog-to-detail navigation and a direct
  detail refresh.
- Exercise successful, failed, and awaiting-input states, valid intervention,
  invalid JSON, invalid-state controls, evidence rendering, produced-file
  download, related links, and responsive overflow.
- Run the compatibility-admin detail smoke as a regression.
- Run focused and repository-wide validation; record README/ARCH impact and
  update roadmap/issue evidence.

## Test Strategy

### Unit And Component

- Complete resource, event, log, and produced-file mappers.
- Client methods and failure behavior through the shared transport.
- State gating for terminal, active, queued, cancelling, and awaiting-input
  runs with and without pending requests.
- JSON input validation, retained values after errors, local evidence failures,
  accessible feedback, links, download callbacks, and rendering of every
  inspector section.

### Integration

- Route orchestration loads the run and independent evidence endpoints.
- Mutations replace the run with the authoritative response and refresh
  evidence without losing successful sections.
- Deep links work after browser refresh for canonical and alias routes.
- The overview opens detail and preserves the persistent authenticated shell.

### Smoke And E2E

- Seed successful, failed, and awaiting-input runs through the real Compose API.
- Seed logs, events, and a produced file through automation access.
- Submit input on the awaiting run, observe resumed state, and verify terminal
  controls are disabled on successful/failed runs.
- Download the produced file and verify its exact bytes.
- Open the related workflow, session detail, and session preview targets.
- Check desktop and narrow viewports for body and panel overflow.
- Run the existing compatibility-admin workflow-run detail smoke unchanged.

## Post-Implementation Smoke Sequence

1. `cd code/web/bpane-admin-unified && npm ci`
2. `npm run check`
3. `npm test`
4. `npm run test:coverage`
5. `npm run build`
6. Start or verify Compose and log into `/admin-new/` through Keycloak.
7. Run `npm run smoke:admin-unified-workflow-runs -- --headless` from
   `code/web/bpane-client`.
8. Run the compatibility-admin workflow-run detail smoke script with
   `--headless`.
9. Open `/admin-new/workflow-runs/{run_id}` directly for successful, failed,
   and awaiting-input runs; verify metadata, evidence, controls, downloads,
   related links, refresh, and narrow layout.
10. Run `node scripts/validate.mjs --profile fast`.

## Definition Of Done

- A stable canonical workflow-run detail route exists and `/runs` remains
  compatible.
- The detail page exposes all safe current run evidence required by the API and
  product requirements without secret resolution.
- Controls are state-gated, backend-authoritative, accessible, and provide
  local feedback without layout shifts or lost operator input.
- Produced files download with authenticated same-origin requests and verified
  bytes.
- Client, view-model, component, route, negative, smoke, and compatibility
  regression tests pass with coverage ratchets intact.
- README, ARCH, admin status/coverage docs, roadmap, and issue evidence are
  checked and updated where required.

## Evidence Record

- Implemented the complete safe workflow-run resource mapper and authenticated
  detail/evidence/control/download client methods. Paths are constructed from
  encoded run/file ids and all evidence requests retain global 401 handling.
- Added canonical `/admin-new/workflow-runs` overview/detail routes and retained
  `/admin-new/runs` aliases, with canonical navigation, dashboard, launcher,
  and catalog links.
- Added independently testable detail view models and inspector components for
  metadata, source/runtime/intervention facts, input/output, logs, events,
  produced files, recordings, workspace inputs, extension/credential metadata,
  related links, local validation, and state-gated operations.
- Focused component/client/view-model tests passed: 28 tests across six detail
  files, in addition to client contract/error tests committed in Slice 1.
- Full admin-new test suite passed: 111 files and 351 tests.
- Admin-new coverage ratchet passed: 90.74% statements, 76.18% branches,
  93.05% functions, and 88.43% lines.
- Admin-new type check and production static build passed.
- Browser client type check, 86-file/661-test suite, coverage ratchet, and
  production build passed. Browser-client coverage was 92.88% statements,
  87.57% branches, 93.19% functions, and 92.88% lines.
- Rebuilt and deployed the Compose web image successfully.
- `smoke:admin-unified-workflow-runs -- --headless` passed against real Compose
  resources for awaiting-input, succeeded, and failed runs. It verified direct
  and catalog navigation, refresh, invalid JSON, input submission, terminal
  gating, events/logs, exact produced-file download bytes, related links,
  `/runs` compatibility, and 390px overflow.
- `smoke:admin-workflow-run-detail -- --headless` passed unchanged as a
  compatibility-admin regression.
- `node scripts/validate.mjs --profile fast` passed all 40 stages, including
  repository documents, dependency policy, Rust checks/tests/coverage, all web
  packages, integration packages, OpenAPI governance, and egress examples.
- README and admin-new requirement/status/roadmap documents were synchronized.
  `ARCH.md` was checked; no update is required because API ownership, runtime
  topology, and subsystem boundaries did not change.
