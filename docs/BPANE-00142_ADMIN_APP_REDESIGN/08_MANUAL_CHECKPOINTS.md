# Manual Checkpoints

Manual checkpoints keep the migration testable while the current `/admin/` app
and new `/admin-new/` app coexist.

## Baseline

1. Start the local stack.
2. Open `http://localhost:8080/admin/`.
3. Log in with `demo / demo-demo`.
4. Confirm the current live workspace loads.
5. Create or select a session.
6. Confirm connect, disconnect, reconnect, and route navigation still work.

## New App Shell

1. Open `http://localhost:8080/admin-new/`.
2. Confirm the shell shows the Keycloak sign-in state when no session is active.
3. Sign in with `demo / demo-demo`.
4. Confirm Keycloak returns to the same `/admin-new/` route without callback
   query parameters.
5. Refresh `http://localhost:8080/admin-new/projects`.
6. Confirm nginx deep-link fallback works.
7. Open `http://localhost:8080/admin/` and confirm the old app is unchanged.

## Projects Overview

1. Open `http://localhost:8080/admin-new/projects`.
2. Sign in through Keycloak if prompted.
3. Confirm the Projects navigation item is highlighted under Resources.
4. Confirm the overview shows loading, then either the project catalog or a
   clear catalog error.
5. Confirm the list uses one-column selection and the selected project metadata
   is shown beside or below it depending on viewport width.
6. Use Refresh and confirm the catalog reloads without changing the current
   `/admin/` application.
7. Expire or clear the token and confirm the next project refresh redirects to
   Keycloak instead of silently failing.

## Session Workflow

1. Create a session from `/admin-new/sessions/new`.
2. Open the session detail route.
3. Attach to the live tab.
4. Navigate away and return to live.
5. Disconnect.
6. Reconnect.
7. Stop or release only when action eligibility allows it.

## File Workflow

1. Upload a workspace file.
2. Bind it to a session.
3. Confirm mount path validation.
4. Download the bound file.
5. Remove the binding.

## Recording Workflow

1. Start or inspect recording state.
2. Stop where supported.
3. Refresh retained segments.
4. Download a segment.
5. Download playback/export where available.

## Workflow Workflow

1. Open workflow catalog.
2. Create or select a run.
3. Attach to the related session where applicable.
4. Inspect events and logs.
5. Download produced files.
6. Exercise cancel/resume/reject/input submit where safe.

## Egress Workflow

1. Open egress catalog.
2. Select a profile.
3. Edit/clone profile where safe.
4. Run reachability probe.
5. Open a session network tab and run session diagnostics.

## Identity Workflow

1. Open identity/access review.
2. Inspect current principal.
3. Create/edit/disable a service principal in a test scope.
4. Create/edit/disable an identity mapping in a test scope.
5. Confirm delegated-principal and unmapped-signal evidence remains visible.

## Promotion Gate

1. Run focused unit/check/build commands.
2. Run current `/admin/` smokes.
3. Run new `/admin-new/` smokes for migrated routes.
4. Run broader client smoke matrix where impacted.
5. Compare current and new app behavior manually.
6. Keep `/admin/` as the default until parity is accepted.
