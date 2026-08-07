# Admin-New Implementation Status

Revalidated against current routes and package scripts: 2026-08-07

This file maps the current `code/web/bpane-admin-unified` app to the
consolidated redesign requirements. It is based on the current routes,
libraries, components, and smoke scripts.

Capability maturity: Prototype. The route coverage below is current product
evidence, but default promotion remains a Phase 1 gate owned by #163. Use
`DELIVERY_ROADMAP.md` for cross-product sequencing and
`PRODUCT_PHASES_AND_RELEASE_GATES.md` for claim language.

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
| Session detail | `/admin-new/sessions/[session_id]`, `/live`, `/automation`, `/policy`, `/files`, `/recordings`, `/network`, `/observability` | Implemented for #155/#156 scope | Overview/actions remain on the base route. Each operational concern has a refresh-safe subroute; observability uses current REST evidence plus the authenticated owner-scoped event stream. |
| Session preview | `/admin-new/sessions/[session_id]/preview` | Implemented | Popup preview, browser SDK loading, metrics drawer, connect/disconnect behavior. |
| Recordings | `/admin-new/recordings`, `/admin-new/sessions/[session_id]/recordings` | Implemented for current API | Global catalog/download plus session-scoped policy, retained segments, independent playback summary, one-segment WebM, and multi-segment ZIP export are available. Manual Record/Stop controls remain intentionally outside admin-new. |
| Workflows | `/admin-new/workflows`, `/admin-new/workflows/[workflow_id]` | Partial | Catalog, detail, source tree/code preview, workflow launch controls exist. Publishing/catalog management is still not complete. |
| Workflow integration endpoints | Recommended future `/admin-new/workflow-endpoints`, `/new`, `/[endpoint_id]`, `/runs`, `/deliveries` | Missing, Phase N | Stable project-scoped BPM action endpoints, service-principal grants, typed contracts, immutable revision promotion/rollback, completion profiles, overload/readiness, and callback diagnostics are planned in `#172`; this is not an admin-new promotion blocker. |
| Workflow Studio / Teach Mode | Recommended future `/admin-new/workflows/teach`, `/admin-new/workflow-training/[draft_id]` | Missing, Phase N | Semantic demonstration capture, candidate generation, replay, review, immutable publication, and controlled repair are planned in `#171`; this is not an admin-new promotion blocker. |
| Workflow runs | `/admin-new/workflow-runs`, `/admin-new/workflow-runs/[run_id]`; `/admin-new/runs` aliases | Implemented | Catalog and stable detail route expose metadata, independently loaded logs/events/files, produced-file downloads, related links, and state-gated intervention/cancel controls. |
| Identity | Navigation points to `/admin-new/identity` | Missing | No route-backed identity/access-review implementation in unified admin. |
| API reference | Navigation points to `/admin-new/api` | Missing | No API companion route yet. |
| Design memo/docs | Navigation points to `/admin-new/docs` | Missing | Navigation exists, route does not. |
| API coverage | Navigation points to `/admin-new/coverage` | Missing | Navigation exists, route does not. |

## Redesign Step Status

| Redesign step | Requirement | Status | Consolidated note |
| --- | --- | --- | --- |
| Step 0: Baseline | Keep `/admin/` stable while building `/admin-new/`. | Done | `/admin/` remains present and smoke-covered. |
| Step 1: New app beside current admin | Scaffold and serve a static SvelteKit app at `/admin-new/` without changing `/admin/`, `/dist/`, auth config, cert metadata, or APIs. | Done | `bpane-admin-unified` exists and is served at `/admin-new/`. |
| Step 1A: API coverage baseline | Classify every frozen API operation and expose owner, worker, evidence, and compatibility surfaces clearly. | Partial | The inventory exists, but #179 conformance/compatibility enforcement and route-backed `/admin-new/api` or `/coverage` companions are missing. |
| Step 2: Projects overview | Route-backed project catalog, create, detail/edit, quotas, policy gates, usage, and alerts. | Done | Projects catalog/create/detail/edit are implemented. |
| Step 3: Resource foundation | Selector-grade browser context, egress profile, and file workspace catalogs/details. | Done for first pass | Core resource catalogs and detail/edit flows are implemented. |
| Step 4: Create session flow | Session creation form with project/template/context/network/egress/capabilities/recording/payload preview. | Done | Session creation is implemented with selectors and payload preview. |
| Step 5: Sessions catalog | Focused list plus selected-session metadata and explicit connect/disconnect/reconnect behavior. | Done | Catalog and selected-session metadata exist. |
| Step 6: Session detail overview | Route-backed overview preserving lifecycle actions, queue state, connections, disconnect controls, and evidence facts. | Implemented for current split | Overview/actions remain stable while live, automation, policy, files, recordings, network, and observability have dedicated routes. |
| Step 7: Live tab | Route-backed or equivalent live browser surface with browser SDK, container sizing, attach/detach, upload, mic, camera, display controls, and trust guidance. | Implemented as route plus popup | `/live` exposes refresh-safe connection evidence and explicitly launches the standalone `/preview` stream surface, which retains the browser SDK and media controls. |
| Step 8: Session files | Session-specific file and file-binding surface with workspace-file binding and download behavior. | Implemented | `/admin-new/sessions/[session_id]/files` exposes retained uploads/downloads, exact downloads, project-policy-aware binding create/remove, workspace filtering, validation, and partial-error states. |
| Step 9: Recordings | Session recording status, retained segments, downloads, playback manifest, and playback export. | Implemented for current API | `/admin-new/sessions/[session_id]/recordings` exposes policy mutation, independent segment/playback states, WebM download, and multi-segment playback export. |
| Step 10: Network | Session network identity, effective egress, diagnostics, probes, and egress profile link. | Implemented | `/admin-new/sessions/[session_id]/network` separates requested/effective identity, exposes sanitized proof and warnings, links profiles, and gates active probes to already-running runtimes. |
| Step 11: Automation | MCP delegation, workflow associations, automation owner/delegate state, and worker-route separation. | Implemented for current API | `/automation` owns MCP authorize/revoke/default controls and shows only workflow runs bound to the selected session. |
| Step 12: Browser policy | Local-file and File System Access guardrails, project restrictions, and runtime evidence. | Implemented as policy evidence | `/policy` separates effective session capabilities from optional project policy and labels managed-browser startup evidence as non-probe evidence. |
| Step 13: Observability | Current state, admin event stream health, workflow/recording/file/MCP snapshots, and bounded local history. | Implemented for current event API | `/observability` uses short-lived initial-message WebSocket auth, selected-session projection, reconnect status, and a 40-entry in-memory timeline that is not presented as an audit log. |
| Step 14: Remaining resource catalogs | Workflows, workflow runs, templates, extensions, credential bindings, event subscriptions, operation counters, identity/access review. | Partial | Workflow catalog/run launch and workflow-run catalog/detail exist; templates, extensions, credentials, event subscriptions, and identity remain. |
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
- `smoke:admin-unified-recordings`
- `smoke:admin-unified-sessions`
- `smoke:admin-unified-workflows`
- `smoke:admin-unified-workflow-runs`

These should remain mandatory for PRs that touch the unified admin app. #151
must make the selected minimal checks enforceable in CI; the existence of local
scripts alone is not a promotion gate.

## Promotion Assessment

The unified app is useful for manual testing and feature development, but it is
not ready to become default because:

1. Navigation advertises routes that are not implemented.
2. Identity/access-review is absent from the new app despite being a core
   enterprise control-plane surface.
3. API companion and coverage routes are absent.
4. Some security cleanup slices still affect admin trust and production safety.
