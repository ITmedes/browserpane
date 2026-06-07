# BrowserPane Admin App Redesign Foundation

This document defines the foundation for redesigning the BrowserPane admin app
before adding more admin-heavy product surface. It is intentionally not an
implementation plan. The goal is to create one shared capability map and
information architecture for a new admin app that combines the current live view
and inspect routes into a single coherent product.

Canonical issue: https://github.com/ITmedes/browserpane/issues/142

## Recommendation

The best foundation is a task and resource model, not Redoc alone.

BrowserPane already has a canonical OpenAPI contract in
`openapi/bpane-control-v1.yaml`. A Redoc or Scalar API reference would help
developer onboarding, but it would not define how an operator moves between a
live browser, a session inspector, project governance, workflow execution,
files, recordings, network identity, identity mappings, and diagnostics.

Recommended order:

1. Define the new admin information architecture and capability map.
2. Keep OpenAPI as the source for raw API shape and add a rendered API reference
   as a companion, not as the UX foundation.
3. Build the redesigned app shell around resource routes, list/detail views,
   and session live/inspect tabs.
4. Migrate current live overlay features and inspect routes into the new shell
   without expanding product scope during the migration.

## Product and Design Orientation

Mature browser automation and cloud control-plane admin apps converge around a
few patterns that BrowserPane should adopt:

- Session inspector as the center of gravity: a session row opens a detail
  surface with status, configuration, live view, recordings, events, console,
  network, files, and metadata.
- Live view is a tab or mode of the session detail, not a separate app that
  loses context when the operator moves to inspect data.
- Recordings, logs, network summaries, and events are available both in the UI
  and through the API.
- Session creation exposes a compact default path plus advanced configuration
  for projects, contexts, proxies or egress profiles, extensions, metadata,
  timeout, screen size, and recording.
- Resource catalogs use predictable list/detail/action patterns instead of
  placing all controls in one overlay.
- Long-running resources use explicit lifecycle states, bulk cleanup where
  useful, and clear admission or quota feedback.

## Target App Shape

The redesigned admin app should be one app with route-backed state:

- `/admin`: operational dashboard and recent activity.
- `/admin/sessions`: session catalog with filters, selected project, state,
  owner, runtime, template, context, egress profile, recording, and workflow
  indicators.
- `/admin/sessions/:session_id`: session detail with tabs.
- `/admin/sessions/:session_id/live`: live browser tab for the selected session.
- `/admin/sessions/:session_id/files`: session files and mounted workspace
  bindings.
- `/admin/sessions/:session_id/recordings`: segments, playback, exports.
- `/admin/sessions/:session_id/network`: egress profile, diagnostics, sanitized
  usage.
- `/admin/sessions/:session_id/automation`: MCP, automation owners, workflow
  bindings, access descriptors.
- `/admin/sessions/:session_id/observability`: logs, metrics, events, page/tab
  activity as the API matures.
- `/admin/projects`: project catalog, quotas, admission policy, usage, and
  associated resources.
- `/admin/workflows` and `/admin/workflow-runs`: workflow definition and run
  catalogs with detail views.
- `/admin/files/workspaces`: reusable input workspaces and files.
- `/admin/browser-contexts`: reusable Chromium profile catalog.
- `/admin/egress`: egress profile catalog and diagnostics.
- `/admin/identity`: current principal, service principals, identity mappings,
  delegated automation.
- `/admin/api`: rendered API reference and copyable examples, backed by the
  OpenAPI contract.

## UI Principles

- One selected resource is always visible in the route, URL, and header.
- List/detail views replace dense multi-column overlay layouts.
- The live browser is part of the session detail, not a separate state island.
- Actions live next to the selected resource metadata and use consistent action
  bars.
- Advanced configuration is progressive: compact defaults first, detailed JSON
  or API payload previews behind explicit expansion.
- Notifications are centralized and route-aware, with durable feedback for
  long-running operations.
- Session, workflow, file, recording, egress, and identity actions should expose
  consistent success, failure, loading, disabled, and stale-state behavior.
- No new feature should require both the live overlay and a separate inspect
  route to understand the same resource.

## Current Admin Feature Inventory

### Global Shell and Auth

- OIDC login/logout and token refresh handling.
  Purpose: ensure all admin API calls use the current operator identity.
  Example: a demo user signs in through Keycloak and gets redirected back to the
  admin app.

- Global authentication failure handling.
  Purpose: redirect or log out automatically when bearer tokens are invalid.
  Example: an expired admin token redirects the operator back to sign-in instead
  of silently failing API calls.

- Admin event stream subscription.
  Purpose: keep session, recording, MCP, workflow, and file snapshots current.
  Example: a session started by a workflow appears in the admin app without a
  manual page reload.

- Global feedback notifications.
  Purpose: surface state changes and errors for operator actions.
  Example: stopping a session shows a visible success or failure message.

### Live Browser Workspace

- Embedded live browser stream.
  Purpose: display and interact with the selected BrowserPane session.
  Example: an operator joins a running session and controls Chromium through
  the admin app.

- Connect and disconnect controls.
  Purpose: attach or detach the admin browser client from a selected session.
  Example: an operator disconnects from one session before switching to another
  session.

- Auto-connect follow behavior for workflow sessions.
  Purpose: connect the admin live view to the session used by a running workflow.
  Example: an operator starts a workflow and sees the browser it is driving.

### Sessions

- Session list and selected-session preview.
  Purpose: switch between owner-scoped sessions and see selected resource state.
  Example: an operator selects a queued or ready session and sees metadata before
  joining.

- Session creation configurator.
  Purpose: create sessions with structured options instead of raw JSON.
  Example: a support operator creates a session with a project, reusable context,
  egress profile, viewport, labels, and recording policy.

- Project selection during session creation.
  Purpose: attach sessions to governance, quotas, and policy.
  Example: a customer-support session counts against the support project budget.

- Session template selection.
  Purpose: reuse known launch defaults.
  Example: a recurring import reproduction uses a support template.

- Browser context selection and inline context creation.
  Purpose: attach reusable Chromium profile state to a session.
  Example: a workflow starts with a reusable login context instead of repeating
  authentication.

- Network identity and egress profile selection.
  Purpose: control locale, timezone, geolocation, proxy, and TLS interception
  settings.
  Example: an operator starts a German-region session through the local proxy
  observer profile.

- API payload preview.
  Purpose: show the exact create-session command represented by the UI.
  Example: an operator copies the payload to reproduce a launch through the CLI
  or raw API.

- Session detail link from the live overlay.
  Purpose: jump from live operation to inspect mode.
  Example: after seeing a workflow failure, the operator opens the session detail
  page.

### Session Lifecycle

- Refresh selected session and live status.
  Purpose: reconcile UI state with the gateway control plane.
  Example: an operator refreshes after a worker restart to see the current
  runtime assignment.

- Release runtime, stop, kill, disconnect all, and disconnect individual clients.
  Purpose: safely manage active and stuck sessions.
  Example: a session cannot stop because a recorder is still attached, so the
  detail view shows blockers before a force action.

- Live connection list.
  Purpose: see browser, viewer, recorder, and automation attachments.
  Example: an operator disconnects a stale browser client before reconnecting.

- Egress probe from selected session.
  Purpose: validate effective outbound path without starting stopped sessions.
  Example: a local TLS observer profile is probed from a ready runtime.

### Live Session Actions

- Start camera.
  Purpose: send browser-side camera input into the session where supported.
  Example: testing a web app that requests camera access.

- Start microphone.
  Purpose: send browser-side microphone input into the session where supported.
  Example: testing a web meeting join flow.

- Upload files.
  Purpose: send local files into the active browser session.
  Example: testing an import form with a local CSV.

### Display Controls

- Render backend selection.
  Purpose: switch browser render behavior used by the client.
  Example: compare WebGL and fallback rendering during a visual issue.

- HiDPI toggle.
  Purpose: improve rendering sharpness on dense displays.
  Example: an operator turns on HiDPI for text-heavy inspection.

- Scroll-copy toggle.
  Purpose: reuse moved pixels on scroll-heavy pages.
  Example: validate scrolling performance on a long article.

### Session Files and Workspace Bindings

- Session runtime file list and download.
  Purpose: inspect files produced by downloads or runtime activity.
  Example: a browser download is captured and downloaded from the admin UI.

- File workspace link.
  Purpose: move from session-level files to reusable input workspaces.
  Example: an operator opens the workspace that provided a mounted input.

- Mounted workspace input bindings.
  Purpose: bind reusable workspace files into session mount paths.
  Example: a workflow receives `inputs/monthly-report.csv` from a project-owned
  file workspace.

- Binding create, read/download, and remove actions.
  Purpose: manage which files are accessible to a session or automation.
  Example: remove an outdated input file from a stopped session before rerun.

### Recording

- Start recording and stop/save WebM.
  Purpose: capture the composed browser session output.
  Example: record a reproduction of a customer issue.

- Auto-download local WebM.
  Purpose: immediately save the local recording when capture stops.
  Example: a tester records a short manual regression proof.

- Download last WebM.
  Purpose: retrieve the latest local recording artifact.
  Example: save the most recent recording after a browser flow.

- Recording segment list and segment download.
  Purpose: inspect retained gateway recording segments.
  Example: download the segment created before a gateway restart.

- Playback/export download.
  Purpose: package retained recording segments for playback.
  Example: hand off a replay bundle to a support engineer.

### Workflows

- Workflow template/definition selection.
  Purpose: choose a published workflow from the admin UI.
  Example: run the BrowserPane Tour workflow.

- Refresh workflow definitions and current run.
  Purpose: keep catalog and run state aligned with the control plane.
  Example: reload a run after a worker finishes.

- Create or connect a baseline session for a workflow.
  Purpose: ensure the workflow runs against a visible and selected session.
  Example: the admin app creates a new session before invoking a workflow.

- Invoke run and cancel run.
  Purpose: start and stop automation.
  Example: cancel a workflow that is waiting on an external page.

- Operator intervention submit and runtime-hold release.
  Purpose: handle workflow pauses requiring human input or approval.
  Example: submit a one-time input when a workflow requests operator action.

- Recent events and logs.
  Purpose: understand workflow progress without leaving the live app.
  Example: inspect the last worker log lines after a failed run.

- Produced file download.
  Purpose: retrieve workflow-generated artifacts.
  Example: download a CSV produced by a workflow.

- Links to workflow catalog, definition detail, and run detail.
  Purpose: move between live operation and inspect views.
  Example: open a run detail page for full input/output JSON.

### MCP Delegation

- Show session-scoped MCP endpoint.
  Purpose: expose the endpoint an automation client can use for the selected
  session.
  Example: copy `/sessions/{id}/mcp` into an external MCP client.

- Authorize and revoke MCP.
  Purpose: delegate or remove automation ownership for a selected session.
  Example: grant the local MCP bridge access to the session currently visible in
  the admin app.

- Set and clear default delegated session.
  Purpose: let local automation adopt a known session by default.
  Example: make the active session the default MCP target for manual testing.

- Refresh MCP health/status.
  Purpose: validate bridge availability and delegation state.
  Example: confirm the bridge adopted the intended session.

### Browser Contexts

- Context catalog with filter and selected-context detail.
  Purpose: manage reusable Chromium profile state.
  Example: find the reusable context that stores a login.

- Create, clone, import, export, and delete contexts.
  Purpose: manage browser profile lifecycle.
  Example: clone a clean support profile before a risky workflow run.

- Storage usage, retention, active-writer, and session reference visibility.
  Purpose: prevent unsafe context deletion or parallel writers.
  Example: delete only contexts that are inactive and not referenced by active
  sessions.

- Copy API examples.
  Purpose: teach operators how UI actions map to API calls.
  Example: copy a context export API command for scripting.

### Egress Profiles

- Egress profile catalog with filter and selected-profile preview.
  Purpose: manage approved outbound network profiles.
  Example: select the TLS observer profile before creating a session.

- Create, edit, clone, disable, and probe profiles.
  Purpose: manage proxy and TLS interception setup safely.
  Example: clone the local proxy preset and bind it to a project.

- Proxy, proxy-auth binding, TLS intercept CA ref, sensitive log sink, bypass
  rules, labels, and project scope fields.
  Purpose: encode network identity without leaking secrets.
  Example: configure a TLS intercept profile with a local CA bundle and approved
  sink reference.

- Profile diagnostics.
  Purpose: distinguish config-only proof, launch metadata, and active browser
  probe evidence.
  Example: see that proxy auth failed without exposing credentials.

### Identity and Access

- Current principal summary.
  Purpose: show who is operating the admin app.
  Example: confirm the signed-in Keycloak user and mapped claims.

- Project access review.
  Purpose: show visible projects and usage counts for the current principal.
  Example: verify the support operator can see the support project.

- Service principal catalog and selected-principal detail.
  Purpose: manage automation principals.
  Example: create a service principal for the workflow worker.

- Create, edit, disable, and re-enable service principals.
  Purpose: control automation identity lifecycle.
  Example: disable a compromised service principal without redeploying.

- Identity mapping catalog and selected-mapping detail.
  Purpose: map external identity claims to project access and scopes.
  Example: map a Keycloak group to project-scoped session creation.

- Create, edit, disable, and re-enable identity mappings.
  Purpose: manage project access rules from the UI.
  Example: grant a support group access to a new customer project.

- Unmapped signals and delegated automation principals.
  Purpose: make access gaps and delegation state visible.
  Example: see that an incoming claim is not mapped to a project.

### Browser Policy

- Local file and File System Access policy visibility.
  Purpose: show whether browser-local file access is blocked.
  Example: confirm `file:///etc/passwd` navigation is blocked in docker-backed
  sessions.

- Copy policy probe command.
  Purpose: let operators reproduce policy checks outside the UI.
  Example: copy a local probe command for a support note.

### Metrics

- Start and stop samples.
  Purpose: measure runtime and transport behavior during a live session.
  Example: sample tile/video throughput while scrolling a page.

- Copy metrics and reset samples.
  Purpose: export diagnostic data and clear local state.
  Example: copy metrics into a bug report.

- Surface throughput, tile, video, and transport summaries.
  Purpose: identify rendering or streaming regressions.
  Example: compare tile updates before and after a reconnect.

### Logs

- Local admin event timeline.
  Purpose: track auth, transport, selection, session, recording, workflow, MCP,
  and file events.
  Example: see why a session selection changed.

- Copy and clear logs.
  Purpose: export diagnostics and reset the local view.
  Example: copy log output into an issue after a failed workflow run.

### Inspect and Catalog Routes

- Session inspector list.
  Purpose: list sessions outside the live overlay with filters and catalog
  metadata.
  Example: find stopped or queued sessions without opening the live browser.

- Session inspector detail.
  Purpose: inspect lifecycle, runtime files, file bindings, recordings,
  playback, and related resources.
  Example: debug why a session cannot be stopped.

- Browser context route.
  Purpose: manage reusable browser profiles in a full-page catalog.
  Example: export an inactive profile archive.

- File workspace list route.
  Purpose: create and inspect reusable project-owned input workspaces.
  Example: create a workspace for workflow CSV inputs.

- File workspace detail route.
  Purpose: upload, download, delete, and inspect workspace files.
  Example: upload a monthly report and later bind it to a workflow session.

- Workflow catalog route.
  Purpose: inspect visible workflow definitions/templates.
  Example: open the BrowserPane Tour definition.

- Workflow definition detail route.
  Purpose: inspect definition metadata, versions, source, policy, schemas, and
  recent runs.
  Example: verify a git-backed workflow version is pinned to a commit.

- Workflow run list route.
  Purpose: inspect workflow run history.
  Example: find the latest failed run for a support case.

- Workflow run detail route.
  Purpose: inspect run facts, input, output, logs, events, produced files, and
  controls.
  Example: download an artifact or release a runtime hold from a specific run.

## Redesign Acceptance Criteria

- The live browser and inspector for a session are in the same route-backed
  resource area.
- All current admin capabilities listed above have an explicit destination in
  the new information architecture.
- The old operations overlay no longer owns broad catalog management. It may
  survive only as a focused quick-action drawer for the active live session.
- Session selection, connection state, workflow-follow state, and detail-route
  state are represented by URLs and shared stores, not independent local islands.
- Project scope and selected project are visible across session creation,
  workflows, file workspaces, browser contexts, egress, identity, and usage.
- Long-running actions have consistent loading, success, error, stale, and
  disabled states.
- Route-level smoke tests cover the migrated live and inspect flows before
  removing the existing overlay implementation.
- API documentation is rendered from `openapi/bpane-control-v1.yaml` as a
  companion surface, but the app UX remains task-oriented.
