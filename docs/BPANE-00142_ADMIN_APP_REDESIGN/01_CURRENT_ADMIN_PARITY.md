# Current Admin Parity

The new app must preserve current `/admin/` behavior while moving it into a
route-backed, easier-to-test structure.

## Current Surface

Current package:

- `code/web/bpane-admin`
- Static SvelteKit output through `@sveltejs/adapter-static`
- Base path from `BPANE_ADMIN_BASE_PATH`
- Svelte 5, SvelteKit, Vite, TypeScript, Tailwind 4, `lucide-svelte`, Vitest

Current package/config/static shell files to preserve or recreate:

- `package.json`
- `package-lock.json`
- `svelte.config.js`
- `vite.config.ts`
- `tsconfig.json`
- `src/app.html`
- `src/app.css`
- `static/browserpane-logo.png`

Do not copy generated or vendored output:

- `.svelte-kit/`
- `build/`
- `node_modules/`

## Route Parity

Current route files that must remain covered until the new app has equivalent
behavior:

- `src/routes/+layout.svelte`
- `src/routes/+page.svelte`
- `src/routes/sessions/+page.svelte`
- `src/routes/sessions/[session_id]/+page.svelte`
- `src/routes/sessions/[session_id]/+page.ts`
- `src/routes/browser-contexts/+page.svelte`
- `src/routes/files/workspaces/+page.svelte`
- `src/routes/files/workspaces/[workspace_id]/+page.svelte`
- `src/routes/files/workspaces/[workspace_id]/+page.ts`
- `src/routes/workflows/+page.svelte`
- `src/routes/workflows/[workflow_id]/+page.svelte`
- `src/routes/workflows/[workflow_id]/+page.ts`
- `src/routes/workflow-runs/+page.svelte`
- `src/routes/workflow-runs/[run_id]/+page.svelte`
- `src/routes/workflow-runs/[run_id]/+page.ts`

## Behavior Families

Preserve these current behavior families:

- Auth bootstrap, OIDC login completion, token refresh, logout, and global auth
  failure handling.
- Global messages, accessibility roles, and route-level feedback.
- Session selection, join/reconnect/disconnect, lifecycle actions, connection
  disconnect, queue cancel, stop, release, kill, and egress probe.
- Live browser attach state, browser SDK loading, local certificate hash and
  fingerprint handling, upload, microphone, camera, display controls, and
  viewport sizing.
- Session files, session file bindings, workspace files, downloads, uploads,
  mount-path validation, and provenance.
- Recording status, start/stop controls where supported, retained segments,
  playback/export downloads, and recording snapshot updates.
- MCP delegation, workflow run creation, run cancel/resume/reject/input submit,
  workflow visibility rules, produced files, run logs, and run events.
- Browser contexts: create, clone, import, export, delete, storage/retention
  visibility, and active-writer constraints.
- Egress profiles: local presets, create/edit/clone/disable/probe, TLS
  interception fields, proxy auth fields, diagnostics evidence.
- Identity: current principal, access review, service principals,
  identity mappings, delegated principals, unmapped signals, create/edit/disable.
- Metrics/logs: admin event stream status, session snapshots, workflow
  snapshots, recording snapshots, gateway logs, copy/clear behavior.
- Browser policy: local-file and File System Access guardrails, probe command,
  CDP endpoint evidence.

## Generated Parity Inventory

Before each route migration is complete, generate or update an inventory from
`code/web/bpane-admin/src` that covers:

- routes
- application helpers
- auth helpers
- security helpers
- browser-session helpers
- presentation components
- view models
- formatters
- API clients and mappers
- exported symbols
- tests

Every item must have one of:

- migrated destination
- replacement test
- explicit deferral with reason

Current audit baseline:

- 173 current admin source files
- 15 route files
- 45 current admin test files
- 102 current API client methods
- 48 current `controlClient` UI calls
- 17 current `workflowClient` UI calls
- 1 current browser connector UI call
