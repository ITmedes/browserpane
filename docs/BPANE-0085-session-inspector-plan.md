# BPANE-0085 Session Inspector Plan

Date: 2026-05-11

GitHub issue: https://github.com/ITmedes/browserpane/issues/85

## Goal

Add route-level admin session inspection so operators can find and inspect
sessions without relying on the live browser overlay.

The first slice should create durable URLs and reusable route/client plumbing,
while staying inside the backend APIs that already exist after PR #83.

## Existing State

- `code/web/bpane-admin/src/routes/+page.svelte` owns the current single-page
  admin workspace.
- `ControlClient` already exposes session list, session get, status, stop,
  kill, connection disconnect, session files, and recording library methods.
- `SessionDetailPanel`, `SessionTable`, `SessionLifecycleSurface`, files,
  recordings, logs, workflow, and MCP surfaces already provide reusable view
  model and UI patterns.
- Authentication already supports global API auth failure handling through the
  authenticated API helper.

## Scope

Implement the route-level session inspector:

- Add `/admin/sessions` with an owner-scoped session table.
- Add `/admin/sessions/{session_id}` with a session detail inspector.
- Extract shared route bootstrap for auth state, token store, OIDC redirect
  completion, global auth failure handling, and typed admin clients.
- Show session facts: lifecycle, owner mode, runtime assignment, compatibility
  mode, labels, created/updated/stopped timestamps, idle timeout where
  available, and last refreshed timestamp.
- Show live status facts: browser/viewer/automation/recorder counts, recording
  summary, join/full-refresh telemetry, and runtime availability where present.
- Show connection rows with the fields currently available from the status
  payload and wire disconnect-one/disconnect-all actions.
- Show compact related-resource sections for runtime files, recordings, and
  workflow runs when existing clients can load the data without adding backend
  API shape.
- Link from the current live workspace selected session to the session detail
  route.
- Add stale, empty, loading, permission/auth failure, and not-found states.

## Non-Goals

- New backend session filtering or pagination.
- New per-connection fields beyond the current status payload.
- Full workflow run detail routes.
- File workspace, credential, extension, operations, audit, or signed-handoff
  routes.
- Persisted unified session timelines or support bundles.
- Replacing the live workspace overlay.

## Implementation Slices

### 1. Shared Admin Route Bootstrap

- Extract auth/client initialization from `+page.svelte` into reusable route
  state or helper modules.
- Keep same-origin auth metadata loading and OIDC redirect completion behavior.
- Keep automatic logout or reauth on global authentication failures.
- Add unit coverage for the extracted route bootstrap behavior where practical.

### 2. Session List Route

- Add `/admin/sessions`.
- Reuse existing session list mapping and view-model patterns.
- Add client-side search/filter for the first slice only if it stays small and
  does not imply server-side pagination behavior.
- Link each row to `/admin/sessions/{session_id}`.

### 3. Session Detail Route

- Add `/admin/sessions/{session_id}`.
- Load session resource, status, recordings, session files, and related
  workflow-run snapshot data where already available.
- Render operator facts in dense, scannable sections rather than a long overlay
  stack.
- Wire stop, kill, disconnect-one, and disconnect-all through the existing
  client methods.
- Keep refresh explicit and show last successful refresh time.

### 4. Live Workspace Integration

- Add a detail link from the selected-session area in the current workspace.
- Keep session creation and live browser connection in the existing workspace
  for this slice.
- Preserve all existing admin smokes.

### 5. Smoke Coverage

- Add an admin session-detail smoke that:
  - authenticates through the local admin flow
  - creates or selects a session
  - opens `/admin/sessions/{session_id}`
  - verifies session facts and status are visible
  - exercises refresh
  - verifies at least one operator action is correctly disabled or succeeds
    based on connection state

## Validation

- `cd code/web/bpane-admin && npm test`
- `cd code/web/bpane-admin && npm run check`
- `cd code/web/bpane-admin && npm run build`
- `cd code/web/bpane-client && npm run smoke:admin-session -- --headless`
- New `smoke:admin-session-detail` or equivalent Playwright smoke.

## PR Notes

The implementation PR should reference #85 and #82, but should only close #85
if the route-level session list/detail scope and smoke coverage are complete.
