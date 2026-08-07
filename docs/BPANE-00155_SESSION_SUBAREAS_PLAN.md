# BPANE-00155 Session Subareas Implementation Plan

## Target

- Issue: [#155 Add admin-new session subareas for live, files, recordings, and network](https://github.com/ITmedes/browserpane/issues/155)
- Branch: `feature/BPANE-00155`
- State: In progress
- Product surface: `/admin-new/sessions/[session_id]`

## Business Case

The unified admin currently puts lifecycle controls, MCP delegation, policy,
network, and connection evidence into one large session inspector. Operators
cannot deep-link a colleague to the files, recording, or network evidence for a
specific session. Stable subroutes make those investigations reviewable,
refresh-safe, and independently testable without changing the underlying
owner-scoped control API.

## Example Use Case

A support operator receives a link for a session whose browser download did not
reach a downstream workflow. They open the session files route, verify the
captured download and its digest, move to recordings to confirm the interaction,
and inspect network diagnostics for the effective egress profile. Refreshing or
sharing any route keeps the same session context, and moving to another session
does not retain evidence from the first one.

## Current Implementation Evidence

- `/admin-new/sessions/[session_id]` loads a session resource plus best-effort
  live status and renders the broad `SessionInspector`.
- `/admin-new/sessions/[session_id]/preview` is intentionally outside the admin
  shell and provides the resizable popup browser client.
- The gateway already exposes owner-scoped session files, file bindings,
  recordings, playback/export, and egress-diagnostics APIs.
- Admin-new already has authenticated session, recording, file-workspace, and
  egress clients, but does not yet compose their session-scoped evidence behind
  stable routes.

## Route Contract

| Route | Responsibility |
| --- | --- |
| `/admin-new/sessions/[session_id]` | Overview, lifecycle actions, MCP delegation, and summary evidence |
| `/admin-new/sessions/[session_id]/live` | In-shell connection status and explicit launch of the standalone preview |
| `/admin-new/sessions/[session_id]/preview` | Standalone browser popup; retained as the actual stream surface |
| `/admin-new/sessions/[session_id]/files` | Captured uploads/downloads and workspace file bindings |
| `/admin-new/sessions/[session_id]/recordings` | Recording policy, segments, playback/export availability, and downloads |
| `/admin-new/sessions/[session_id]/network` | Effective network identity, egress profile, diagnostics, and explicit probes |

The admin shell remains responsible for authentication recovery. Each subroute
must derive its session id from the URL, reload independently after refresh, and
clear prior evidence before a different session is loaded.

## Implementation Slices

### 1. Route Foundation And Live Area

1. Add a shared, accessible session subarea navigation component and pure route
   helpers.
2. Add the navigation to the existing session overview without moving lifecycle
   mutations in this first change.
3. Add `/live` as an in-shell status route that loads the selected session and
   opens the existing `/preview` popup explicitly.
4. Cover active-route resolution, encoded session ids, direct refresh, loading,
   missing-id, and API-error behavior.

Manual checkpoint: open a session overview, navigate to Live, refresh the page,
and launch the existing resizable preview popup.

### 2. Files And Bindings

1. Add strict session-file and file-binding types, mappers, and authenticated
   client methods.
2. Render captured files and workspace bindings with source, state, mount path,
   size, digest, timestamps, and same-origin downloads.
3. Make policy-blocked, empty, partial-load, and unavailable-content states
   explicit.
4. Add binding create/remove controls only when the existing API and project
   policy allow them; keep owner boundaries enforced by the gateway.

Manual checkpoint: bind a workspace file, capture an upload or download, refresh
the route, download the available content, and verify blocked/empty states.

### 3. Recording Evidence

1. Load session recording policy, retained segments, and playback/export
   summary independently from the global recording catalog.
2. Show active, finalizing, ready, failed, and expired-artifact states.
3. Preserve one user-facing download action: WebM for one retained media file or
   the backend playback/export bundle when multiple segments exist.
4. Keep recording policy mutation and action feedback consistent with the
   overview until lifecycle ownership is deliberately moved.

Manual checkpoint: enable recording, connect, interact, disconnect/stop, then
verify segment state and download from the session recording route.

### 4. Network Evidence

1. Render requested and effective network identity separately.
2. Show sanitized egress configuration, runtime launch evidence, observer
   correlation metadata, byte totals, and diagnostics without secrets or URLs.
3. Add an explicit active probe action only for already-ready runtimes; never
   start a stopped session as a diagnostic side effect.
4. Represent no-egress, proxy, TLS-intercept, policy-blocked, stale, and failed
   states.

Manual checkpoint: compare no-egress, proxy, and TLS-interceptor sessions and
run probes only against already-ready sessions.

### 5. Regression, Documentation, And Promotion Evidence

1. Extend the admin-new session smoke across direct subroute navigation,
   refresh, session switching, responsive layout, and authentication recovery.
2. Run old-admin session/files/recording/network smokes until its retirement
   criteria are met.
3. Synchronize `README.md`, admin-new status, manual checkpoints, architecture,
   and issue evidence where behavior changed.
4. Record exact unit, integration, smoke, and e2e results before PR creation.

## Test Strategy

- Unit: route builders, active-route matching, strict payload mappers, formatting,
  action gating, policy-state derivation, and stale-state prevention.
- Component integration: every route gets loading, empty, ready, partial-error,
  fatal-error, and action-feedback coverage using authenticated fetch fixtures.
- Browser smoke: real Compose resources, direct navigation and refresh, exact
  downloaded bytes, preview popup, recording artifact, egress diagnostics, and
  390px/desktop overflow checks.
- Regression: admin-new session catalog/detail/create, workflow-run links,
  recording catalog, egress profiles, file workspaces, and applicable old-admin
  smokes.

## Post-Implementation Smoke Sequence

1. Build the admin-new package and run its full unit/component suite with
   coverage.
2. Start Compose and create two sessions with different project, workspace,
   recording, and egress settings.
3. Open every canonical subroute directly and by navigation, refresh each one,
   and verify that encoded ids and authentication recovery preserve context.
4. Generate one browser file transfer, one workspace binding, one retained
   recording, and one egress diagnostic sample; verify exact evidence and
   downloadable bytes.
5. Switch repeatedly between both sessions and confirm no file, recording,
   network, action-message, or preview state leaks.
6. Run the expanded admin-new sessions smoke and the impacted old-admin
   session/files/recording/network regression smokes headlessly.

## Non-Goals

- Embedding the live browser stream inside the admin shell.
- Replacing gateway authorization or owner/project policy enforcement.
- Moving automation, policy, or observability into this issue; those remain in
  the subsequent route-completion slice.
- Inventing new file, recording, or egress backend contracts.
