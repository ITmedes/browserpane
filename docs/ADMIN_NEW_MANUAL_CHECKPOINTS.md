# Admin-New Manual Checkpoints

This document preserves the route-level manual validation gates from the old
admin redesign workspace. Use it when deciding whether a migrated route can be
treated as complete.

## Baseline

1. Start the local stack.
2. Open `http://localhost:8080/admin/`.
3. Log in with `demo / demo-demo`.
4. Confirm the current live workspace loads.
5. Create or select a session.
6. Confirm connect, disconnect, reconnect, and route navigation still work.
7. Open `http://localhost:8080/admin-new/` and confirm the new app is separate
   from the old app.

## New App Shell

1. Open `http://localhost:8080/admin-new/`.
2. Confirm unauthenticated access redirects to Keycloak.
3. Sign in with `demo / demo-demo`.
4. Confirm Keycloak returns to the same `/admin-new/` route without stale
   callback query parameters.
5. Refresh a deep link such as `/admin-new/projects`.
6. Confirm the web server deep-link fallback works.
7. Open `/admin/` and confirm the old app is unchanged.

## Dashboard

1. Open `/admin-new/`.
2. Compare dashboard counts with old admin sessions and workflow routes.
3. Confirm visible sessions, active/ready/queued sessions, workflow-run state
   counts where available, visible projects, browser contexts, egress profiles,
   file workspaces, recordings, and workflow definitions are represented.
4. Click dashboard links to Sessions, Workflow runs, Recordings, Projects,
   Egress profiles, Browser contexts, File workspaces, and Workflows.
5. Refresh the dashboard and confirm counts repopulate without stale errors.
6. Confirm the dashboard is read-only and does not introduce hidden mutations.

## Projects

1. Open `/admin-new/projects`.
2. Confirm the Projects navigation item is highlighted under Resources.
3. Confirm loading, empty, and error states are visible.
4. Confirm the catalog table shows project, state, activity, runtime, egress,
   storage, policy, alert, and updated columns.
5. Use project lenses and search without opening a detail route.
6. Open an existing project detail.
7. Confirm metadata, usage, quotas, policy, generated alerts, and resource
   selectors load from the backend.
8. Edit name, description, labels, or state.
9. Toggle one policy gate and restrict one selector-backed allowlist.
10. Enable one quota, enter a positive value, save, and confirm the API response
    updates the form.
11. Enter an invalid quota such as `0` and confirm validation blocks save.
12. Confirm usage and generated alerts remain read-only evidence.
13. Expire or clear the token and confirm the next refresh redirects to
    Keycloak instead of silently failing.

## Sessions Catalog

1. Open `/admin-new/sessions`.
2. Verify all visible sessions from the old admin list appear.
3. Use lenses for all, active/joinable, queued, stopped, and attention states.
4. Use filters for state, runtime state, template, labels, and integration
   metadata where supported.
5. Confirm project, browser-context, and egress filters are not presented as
   server-backed if they are client-side only.
6. Open stopped, ready, active, and queued sessions where available.
7. Switch selected sessions and confirm stale live browser state is not reused.
8. Refresh the route after selecting filters.

## Session Detail Overview

1. Open `/admin-new/sessions/{session_id}`.
2. Confirm header state, session id, project, runtime, owner, and updated facts
   match the old detail view and API payload.
3. Refresh the detail URL directly.
4. Test Refresh.
5. On a queued test session, test Cancel queued.
6. On a disposable connected session, test single-connection disconnect and
   disconnect-all.
7. Test Stop, Release runtime, and Kill only when safe and eligible.
8. Confirm blockers and eligibility reasons are visible before the action.
9. Confirm errors and successes use the new feedback style.

## Live Browser

1. Open `/admin-new/sessions/{session_id}/live` directly and refresh it.
2. Confirm runtime, connection, and resolution facts match the session status
   API, then launch the standalone preview popup.
3. Attach and confirm the browser becomes visible.
4. Resize the popup/window and confirm browser height derives from the live
   container/canvas, not the outer window.
5. Navigate away while attached and confirm the attach/follow state is clear.
6. Return to live and confirm the same session is still attached when intended.
7. Disconnect and confirm live state is cleaned up.
8. Switch sessions and confirm the old canvas is not reused.
9. Simulate a WebTransport opening-handshake failure and confirm the error
   mentions the gateway URL, local QUIC trust guidance, and SPKI fingerprint
   helper.
10. Confirm upload, desktop audio, microphone, camera, clipboard, HiDPI,
   scroll-copy, and render-backend controls respect capabilities and policy.

## Session Files

1. Upload a small file through the live browser flow where policy allows it.
2. Open `/admin-new/sessions/{session_id}/files` directly and refresh it.
3. Confirm the session file appears and can be downloaded.
4. Bind a workspace file to a relative mount path.
5. Try an absolute or parent-traversal mount path and confirm validation blocks
   submission before the API call.
6. Bind files with `read_only`, `read_write`, and `scratch_output` where
   allowed, and confirm the resulting mode is visible.
7. Refresh the route and confirm bindings remain visible.
8. Disable project file bindings and confirm existing evidence remains visible
   while create/remove controls are blocked with a policy message.
9. Make either the binding or workspace catalog unavailable and confirm the
   other section remains usable without a route-wide failure.

## Recordings

1. Create or select a session whose recording policy allows the intended mode.
2. Start or inspect recording state.
3. Perform a short browser action.
4. Stop where supported.
5. Confirm the artifact is non-empty and downloadable.
6. Refresh retained segments and confirm metadata is preserved.
7. Download a retained segment.
8. Download playback/export where available.
9. Confirm failed/unavailable artifacts explain the state instead of appearing
   as broken download buttons.

## Network And Egress

1. Create or open sessions using no egress, forward-proxy egress, and
   TLS-intercept egress profiles.
2. Confirm effective network identity, profile, mode, and project scope are
   visible.
3. Run egress diagnostics/probe against a ready session.
4. Confirm diagnostics do not expose requested URLs, credentials, CA material,
   headers, payloads, or decrypted traffic.
5. Confirm stopped sessions are not implicitly started by diagnostics.
6. Open the related egress profile detail from the session network area.

## Automation And MCP

1. Open a selected session automation/MCP area.
2. Confirm the session-scoped endpoint uses `/sessions/{session_id}/mcp`.
3. Authorize MCP.
4. Set default for compatibility clients.
5. Verify bridge health and managed-session alignment.
6. Connect a Streamable HTTP MCP client to the copied endpoint.
7. Run `tools/list`.
8. Call `browser_navigate` and confirm the selected BrowserPane preview
   navigates.
9. Clear default, revoke, and confirm the delegate state updates.

## Browser Policy

1. Open a selected session policy area.
2. Confirm local-file and File System Access policy state matches the old
   Policy panel.
3. Copy the local-file CDP probe command and run it manually against a
   docker-backed ready session.
4. Confirm probe output can be interpreted without exposing local file content.
5. For a project-scoped session, confirm template, egress, extension, context,
   file-workspace, upload/download, session-file, and manual-recording policy
   constraints are visible.

## Observability

1. Attach to a session.
2. Start and stop a metrics sample.
3. Run a workflow or recording action.
4. Confirm local events are visible and copyable.
5. Confirm event stream status, reconnect/auth-probe state, session snapshots,
   workflow snapshots, file snapshots, recording snapshots, MCP snapshots, and
   admin errors are represented where available.
6. Clear logs and confirm only local log state is cleared.
7. Confirm local UI diagnostics are distinct from persisted gateway logs.

## Create Session

1. Open `/admin-new/sessions/new`.
2. Create a basic default session.
3. Create a project-scoped session.
4. Create a session with an egress profile.
5. Create a session with a reusable browser context.
6. Create a session with an explicit viewport.
7. Create a session with an approved extension when an extension fixture is
   available.
8. Configure API-backed recording policy where accepted.
9. Expand/collapse API payload and confirm it stays open.
10. Verify invalid viewport, invalid labels, disallowed template, disallowed
    extension, disallowed context, disallowed egress profile, and disallowed
    project-policy cases are blocked with field-level feedback.
11. Confirm create does not auto-start a session before operator submission.

## Browser Contexts

1. Open `/admin-new/browser-contexts`.
2. Create an owner-scoped reusable context with labels, retention, and profile
   storage limit.
3. Create a project-scoped reusable context and confirm project binding is
   persisted and displayed.
4. Reopen a context through the details link.
5. Confirm validation blocks malformed labels, blank names, and non-positive
   retention/storage values.
6. Clone an inactive context when clone UI is available.
7. Export/import an archive when those UI actions are available.
8. Confirm delete is blocked for unsafe contexts and succeeds for unused ones.

## Egress Profiles

1. Open `/admin-new/egress`.
2. Confirm metrics for total, ready, TLS-intercept, and attention profiles.
3. Search by name, health, project, or badge.
4. Create an owner-scoped direct metadata-only profile.
5. Reopen it and change it to a proxy-backed profile.
6. Change observation mode to `tls_intercept` and confirm inline validation
   requires proxy URL, custom CA reference, and sensitive log sink reference.
7. Complete TLS fields and save.
8. Probe the profile.
9. Confirm disabled profiles cannot be selected for new sessions.

## File Workspaces

1. Open `/admin-new/files/workspaces`.
2. Create a project-scoped workspace with labels and description.
3. Reopen it from the catalog.
4. Confirm validation blocks blank names and malformed labels.
5. Upload a small text or CSV file.
6. Confirm name, size, hash, content type, and provenance are displayed.
7. Download the file and verify content matches upload.
8. Delete the file and confirm the empty state returns.
9. Bind it to a session through the session files area.

## Workflows

1. Open `/admin-new/workflows`.
2. Confirm the BrowserPane Tour workflow appears when registered.
3. Confirm hidden/smoke workflows do not appear in the normal Invoke Run
   selection.
4. Open workflow detail.
5. Confirm latest version, executor, entrypoint, source repository/ref/commit,
   schemas, default session, credential references, extension references, and
   file-workspace references are visible without exposing secret values.
6. Browse the source tree and confirm the entrypoint opens by default.
7. Select another source file and confirm the preview updates.
8. Validate source when publishing a new immutable version.
9. Start a run through schema-backed inputs.
10. Use Start and connect where available and confirm the session preview opens.

## Workflow Runs

1. Open canonical `/admin-new/workflow-runs`; confirm `/admin-new/runs` remains
   a compatible alias.
2. Confirm loading, empty/error, metrics, lenses, search, and table rows render
   without layout shifts.
3. Trigger at least one workflow run.
4. Confirm state, workflow id, session id, project/admission, file count, and
   updated time are visible.
5. Open the run detail from the catalog and reload its stable URL directly.
6. Confirm metadata, input, output, logs, events, produced files, errors,
   recordings, source snapshots, workspace inputs, and links to session and
   workflow are visible.
7. Exercise cancel, resume, reject, input submit, and runtime-hold release
   where applicable and safe.

## Session Templates

1. Create a test template.
2. Edit it and confirm the version increments.
3. Create a session from the template.
4. Confirm project policy blocks a disallowed template.
5. Confirm template defaults appear in the API payload preview.

## Extensions

1. Register a test extension reference.
2. Publish a version.
3. Disable and re-enable it.
4. Confirm disabled extensions cannot be selected for new templates, sessions,
   or workflows.
5. Confirm `static_single` is not presented as supporting session extension
   sets.

## Credential Bindings

1. Create a local test credential binding.
2. Use it in an egress proxy-auth profile or workflow definition where safe.
3. Confirm list/detail never expose raw secret material.
4. Confirm invalid references produce visible validation errors.
5. Confirm project-scoped sessions/workflows cannot use credential bindings
   from another project.

## Workflow Event Subscriptions

1. Create a local test subscription pointing at a safe local receiver.
2. Run a workflow.
3. Confirm delivery attempts appear with status, last error, retry/backoff
   state, and linked workflow/run.
4. Delete the subscription and confirm no further delivery attempts are shown.
5. Confirm signing secret material is write-only.

## Identity And Access

1. Open `/admin-new/identity`.
2. Inspect current principal.
3. Create/edit/disable/re-enable a test service principal.
4. Create/edit/disable/re-enable a test identity mapping.
5. Confirm access review updates.
6. Confirm delegated-principal and unmapped-signal evidence remains visible.
7. Confirm token claims are safe and do not expose raw tokens.

## API Companion And Coverage

1. Open `/admin-new/api`.
2. Confirm the OpenAPI link works.
3. Copy a session-list example and run it locally with an authenticated token
   if available.
4. Confirm API docs do not replace task-oriented UI routes.
5. Confirm every OpenAPI operation has one coverage classification.
6. Confirm compatibility endpoints are listed separately from OpenAPI routes.
7. Confirm certificate helper endpoints are listed as runtime helpers, not
   owner-scoped API resources.

## Command Palette

1. Press `Cmd+K` or `Ctrl+K`.
2. Navigate to Sessions.
3. Join a running session.
4. Create a new session.
5. Press Escape and confirm focus returns safely.

## Promotion Gate

1. Run focused unit/check/build commands.
2. Run old `/admin/` smokes for migrated behavior.
3. Run new `/admin-new/` smokes for migrated routes.
4. Run broader client smoke matrix where impacted.
5. Compare old and new app behavior manually.
6. Confirm every visible navigation route exists or is intentionally hidden.
7. Refresh every new-app route and confirm state restoration.
8. Log out and log in again from both apps.
9. Confirm route state, selected session, and live-attach state do not leak
   between `/admin/` and `/admin-new/`.
10. Keep `/admin/` as default until parity and security gates are accepted.

## Final Regression Sequence

1. Fresh local stack startup.
2. Admin login/logout.
3. Session create with default settings.
4. Session create with project, context, egress profile, and recording policy.
5. Live attach, navigate away, return to live, detach.
6. Reconnect a stopped or released session.
7. Upload/download files and bind workspace inputs.
8. Start/stop recording and download playback/export.
9. Run workflow, cancel one run where safe, inspect run detail.
10. Delegate and revoke MCP.
11. Probe egress profile and session egress diagnostics.
12. Create/edit/disable identity mapping or service principal in a test scope.
13. Browser context clone/export/import/delete safe cases.
14. Create/edit a session template and create a session from it.
15. Register/publish/disable/enable an approved extension reference.
16. Create a credential binding and verify secret material is never displayed
    after creation.
17. Create/delete a workflow event subscription and inspect delivery attempts.
18. Inspect workflow and recording operation counters.
19. Confirm automation-task executor routes are visible only as read-only
    evidence/API companion entries.
20. Refresh every new-app route and confirm state restoration.
21. Run the full focused admin smoke suite.
