# Admin-New Implementation Status

This file maps the current `code/web/bpane-admin-unified` app to the
BPANE-00142 redesign plan. It is based on the current routes, libraries,
components, and smoke scripts.

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

| Redesign step | Source plan | Status | Consolidated note |
| --- | --- | --- | --- |
| Step 0: Baseline | `04_IMPLEMENTATION_STEPS.md` | Done | `/admin/` remains present and smoke-covered. |
| Step 1: New app beside current admin | `04_IMPLEMENTATION_STEPS.md` | Done | `bpane-admin-unified` exists and is served at `/admin-new/`. |
| Step 1A: API coverage baseline | `04_IMPLEMENTATION_STEPS.md` | Partial | Planning exists, but no route-backed `/admin-new/api` or `/coverage` companion yet. |
| Step 2: Projects overview | `BPANE-00142_PROJECTS_EXISTING_EDIT_PLAN.md` | Done | Projects catalog/create/detail/edit are implemented. |
| Step 3: Resource foundation | browser contexts, egress profiles, file workspaces plans | Done for first pass | Core resource catalogs and detail/edit flows are implemented. |
| Step 4: Create session flow | `BPANE-00142_SESSIONS_ADMIN_PLAN.md`, `BPANE-00142_SESSION_CAPABILITIES_PLAN.md` | Done | Session creation is implemented with selectors and payload preview. |
| Step 5: Sessions catalog | `BPANE-00142_SESSIONS_ADMIN_PLAN.md` | Done | Catalog and selected-session metadata exist. |
| Step 6: Session detail overview | `BPANE-00142_SESSIONS_ADMIN_PLAN.md` | Partial | Overview/actions exist; route-backed detail subareas still need to be split out. |
| Step 7: Live tab | `04_IMPLEMENTATION_STEPS.md` | Partial replacement | Preview popup exists. Dedicated `/live` tab route is not implemented. |
| Step 8: Session files | `04_IMPLEMENTATION_STEPS.md` | Missing in admin-new session subroute | File workspace app exists, but session-specific files route is missing. |
| Step 9: Recordings | `04_IMPLEMENTATION_STEPS.md`, recording plans | Partial | Top-level recording catalog exists; session recordings subroute and deeper playback controls remain. |
| Step 10: Network | `04_IMPLEMENTATION_STEPS.md` | Missing as session subroute | Egress profile resource exists, but session network/diagnostics route is missing. |
| Step 11: Automation | `04_IMPLEMENTATION_STEPS.md`, MCP delegation plans | Partial | MCP delegation is implemented in session detail; workflow associations and automation subroute remain incomplete. |
| Step 12: Browser policy | `04_IMPLEMENTATION_STEPS.md` | Missing as session subroute | Old admin/browser-policy smoke exists; unified route does not. |
| Step 13: Observability | `04_IMPLEMENTATION_STEPS.md` | Missing as session subroute | Metrics drawer exists in preview, but logs/events/admin stream route is missing. |
| Step 14: Remaining resource catalogs | `04_IMPLEMENTATION_STEPS.md` | Partial | Workflows/runs exist; templates, extensions, credentials, event subscriptions, identity remain. |
| Step 15: Dashboard | `BPANE-00142_DASHBOARD_PLAN.md` | Done for first pass | Dashboard overview exists and has smoke coverage. |
| Step 16: Command palette | `04_IMPLEMENTATION_STEPS.md` | Missing | No implemented command palette route/component. |
| Step 17: API reference/coverage companion | `04_IMPLEMENTATION_STEPS.md` | Missing | Navigation planned but routes absent. |
| Step 18: Promotion decision | `04_IMPLEMENTATION_STEPS.md` | Not ready | `/admin/` must remain default until parity/security gates pass. |

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

