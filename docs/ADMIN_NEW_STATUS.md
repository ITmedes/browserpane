# Admin-New Implementation Status

Revalidated against current routes and package scripts: 2026-07-31

This file maps the current `code/web/bpane-admin-unified` app to the
consolidated redesign requirements. It is based on the current routes,
libraries, components, and smoke scripts.

## Current Route Coverage

| Area | Current route(s) | Status | Notes |
| --- | --- | --- | --- |
| Shell | `/admin-new/` layout and dashboard | Implemented | Route-backed shell, grouped navigation, auth bootstrap, dashboard overview. |
| Projects | `/admin-new/projects`, `/admin-new/projects/new`, `/admin-new/projects/[project_id]` | Implemented | Catalog, create, detail/edit, quotas, policy fields, selector catalogs. |
| Browser contexts | `/admin-new/browser-contexts`, `/new`, `/[context_id]` | Implemented | Catalog, create, detail/edit. Follow-up actions like clone/import/export remain broader context-resource operations. |
| Egress profiles | `/admin-new/egress`, `/new`, `/[profile_id]` | Implemented | Catalog, create/edit, profile metadata, diagnostics-oriented fields. |
| File workspaces | `/admin-new/files/workspaces`, `/new`, `/[workspace_id]` | Implemented | Catalog, create/edit/detail, file visibility and workspace flows. |
| Sessions catalog | `/admin-new/sessions` | Implemented | List, selected-session metadata, lifecycle actions, connect flow, MCP delegation visibility. |
| Session create | `/admin-new/sessions/new` | Implemented | Project, template, context, egress, capability, recording, labels, idle timeout, and payload-preview path. |
| Session detail | `/admin-new/sessions/[session_id]` | Partial | Overview and operational actions exist. Route-backed subareas are still missing. |
| Session preview | `/admin-new/sessions/[session_id]/preview` | Implemented | Popup preview, browser SDK loading, metrics drawer, connect/disconnect behavior. |
| Recordings | `/admin-new/recordings` | Partial | Top-level catalog/download view exists. Session-scoped recordings subroute and deeper playback management remain missing. |
| Workflows | `/admin-new/workflows`, `/admin-new/workflows/[workflow_id]` | Partial | Catalog, detail, source tree/code preview, workflow launch controls exist. Publishing/catalog management is still not complete. |
| Workflow runs | `/admin-new/runs` | Partial | Overview route exists. Detail route, logs/events/files/controls are still missing. |
| Identity | Navigation points to `/admin-new/identity` | Missing | No route-backed identity/access-review implementation in unified admin. |
| API reference | Navigation points to `/admin-new/api` | Missing | No API companion route yet. |
| Design memo/docs | Navigation points to `/admin-new/docs` | Missing | Navigation exists, route does not. |
| API coverage | Navigation points to `/admin-new/coverage` | Missing | Navigation exists, route does not. |

## Redesign Step Status

| Redesign step | Requirement | Status | Consolidated note |
| --- | --- | --- | --- |
| Step 0: Baseline | Keep `/admin/` stable while building `/admin-new/`. | Done | `/admin/` remains present and smoke-covered. |
| Step 1: New app beside current admin | Scaffold and serve a static SvelteKit app at `/admin-new/` without changing `/admin/`, `/dist/`, auth config, cert metadata, or APIs. | Done | `bpane-admin-unified` exists and is served at `/admin-new/`. |
| Step 1A: API coverage baseline | Classify every frozen API operation and expose owner, worker, evidence, and compatibility surfaces clearly. | Partial | The audit exists in consolidated docs, but no route-backed `/admin-new/api` or `/coverage` companion exists. |
| Step 2: Projects overview | Route-backed project catalog, create, detail/edit, quotas, policy gates, usage, and alerts. | Done | Projects catalog/create/detail/edit are implemented. |
| Step 3: Resource foundation | Selector-grade browser context, egress profile, and file workspace catalogs/details. | Done for first pass | Core resource catalogs and detail/edit flows are implemented. |
| Step 4: Create session flow | Session creation form with project/template/context/network/egress/capabilities/recording/payload preview. | Done | Session creation is implemented with selectors and payload preview. |
| Step 5: Sessions catalog | Focused list plus selected-session metadata and explicit connect/disconnect/reconnect behavior. | Done | Catalog and selected-session metadata exist. |
| Step 6: Session detail overview | Route-backed overview preserving lifecycle actions, queue state, connections, disconnect controls, and evidence facts. | Partial | Overview/actions exist; route-backed detail subareas still need to be split out. |
| Step 7: Live tab | Route-backed or equivalent live browser surface with browser SDK, container sizing, attach/detach, upload, mic, camera, display controls, and trust guidance. | Partial replacement | Preview popup exists. Dedicated `/live` route is not implemented. |
| Step 8: Session files | Session-specific file and file-binding surface with workspace-file binding and download behavior. | Missing in admin-new session subroute | File workspace app exists, but session-specific files route is missing. |
| Step 9: Recordings | Session recording status, retained segments, downloads, playback manifest, and playback export. | Partial | Top-level recording catalog exists; session recordings subroute and deeper playback controls remain. |
| Step 10: Network | Session network identity, effective egress, diagnostics, probes, and egress profile link. | Missing as session subroute | Egress profile resource exists, but session network/diagnostics route is missing. |
| Step 11: Automation | MCP delegation, workflow associations, automation owner/delegate state, and worker-route separation. | Partial | MCP delegation is implemented in session detail; workflow associations and automation subroute remain incomplete. |
| Step 12: Browser policy | Local-file and File System Access guardrails, probe command, and CDP endpoint evidence. | Missing as session subroute | Old admin/browser-policy smoke exists; unified route does not. |
| Step 13: Observability | Logs, metrics summaries, admin event stream state, workflow/recording snapshots, and future placeholders. | Missing as session subroute | Metrics drawer exists in preview, but logs/events/admin stream route is missing. |
| Step 14: Remaining resource catalogs | Workflows, workflow runs, templates, extensions, credential bindings, event subscriptions, operation counters, identity/access review. | Partial | Workflows/runs exist; templates, extensions, credentials, event subscriptions, identity remain. |
| Step 15: Dashboard | Read-only resource counts, recent operational activity, and links to active work. | Done for first pass | Dashboard overview exists and has smoke coverage. |
| Step 16: Command palette | Global navigation/session join/common creation actions without a second hidden state model. | Missing | No implemented command palette route/component. |
| Step 17: API reference/coverage companion | Copyable API examples, operation classification, OpenAPI link, compatibility separation. | Missing | Navigation planned but routes absent. |
| Step 18: Promotion decision | Compare parity, pass smoke/manual gates, keep `/admin/` side by side until accepted. | Not ready | `/admin/` must remain default until parity/security gates pass. |

## Implemented Admin-New Smoke Coverage

Current `bpane-client` scripts include:

- `smoke:admin-unified-dashboard`
- `smoke:admin-unified-projects`
- `smoke:admin-unified-browser-contexts`
- `smoke:admin-unified-egress-profiles`
- `smoke:admin-unified-file-workspaces`
- `smoke:admin-unified-sessions`
- `smoke:admin-unified-workflows`
- `smoke:admin-unified-workflow-runs`

These should remain mandatory for PRs that touch the unified admin app.

## Promotion Assessment

The unified app is useful for manual testing and feature development, but it is
not ready to become default because:

1. Navigation advertises routes that are not implemented.
2. Workflow-run detail is not route-backed in the new app.
3. Session detail is still too broad; planned subareas are missing.
4. Identity/access-review is absent from the new app despite being a core
   enterprise control-plane surface.
5. API companion and coverage routes are absent.
6. Some security cleanup slices still affect admin trust and production safety.
