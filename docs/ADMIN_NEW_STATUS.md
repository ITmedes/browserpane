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
| Browser contexts | `/admin-new/browser-contexts`, `/new`, `/import`, `/[context_id]`, `/[context_id]/clone` | Implemented for #160 scope | Catalog, create, detail/edit/delete, clone, direct ZIP export, and bounded archive import are route-backed. Active-writer blockers, storage warnings, and retryable import errors remain visible to the operator. |
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
| Identity | `/admin-new/identity` | Implemented for #157 scope | Current principal, project/resource review, delegated principals, unmapped signals, and service-principal/identity-mapping create, edit, disable, and re-enable flows are route-backed and smoke-covered. Registry metadata remains distinct from future enforced RBAC grants in #176. |
| Approved extensions | `/admin-new/extensions`, `/new`, `/[extension_id]` | Implemented for #159 scope | Catalog/create/detail, version publication, and enable/disable actions follow the existing extension control API. Installed paths remain deployment-managed. |
| Credential bindings | `/admin-new/credential-bindings`, `/new`, `/[binding_id]` | Implemented for #159 scope | Owner/project-scoped Vault binding creation and safe metadata inspection are route-backed; submitted secret payloads remain write-only. |
| Workflow event subscriptions | `/admin-new/workflow-event-subscriptions`, `/new`, `/[subscription_id]` | Implemented for #159 scope | Signed subscription creation, detail, deletion, and persisted delivery-attempt diagnostics are available without rendering signing secrets. |
| API reference | `/admin-new/api` | Implemented for #158 scope | Contract-derived task flows expose copyable, placeholder-only commands for projects, sessions/connect tickets, workflow runs, and file workspaces without persisting or rendering the browser bearer token. |
| Integration docs | `/admin-new/docs` | Implemented for #158 scope | Contract scope, authentication domains, error conventions, governance evidence, downloads, and 14 non-v1 compatibility surfaces are separated explicitly. |
| API coverage | `/admin-new/coverage` | Implemented for #158 scope | All 131 frozen operations can be searched and filtered by family, classification, and auth domain from the generated contract evidence. |

## Redesign Step Status

| Redesign step | Requirement | Status | Consolidated note |
| --- | --- | --- | --- |
| Step 0: Baseline | Keep `/admin/` stable while building `/admin-new/`. | Done | `/admin/` remains present and smoke-covered. |
| Step 1: New app beside current admin | Scaffold and serve a static SvelteKit app at `/admin-new/` without changing `/admin/`, `/dist/`, auth config, cert metadata, or APIs. | Done | `bpane-admin-unified` exists and is served at `/admin-new/`. |
| Step 1A: API coverage baseline | Classify every frozen API operation and expose owner, worker, evidence, and compatibility surfaces clearly. | Done | #179 enforces the 131-operation contract inventory and #158 exposes the generated operation, classification, example, and compatibility evidence through route-backed companion views. |
| Step 2: Projects overview | Route-backed project catalog, create, detail/edit, quotas, policy gates, usage, and alerts. | Done | Projects catalog/create/detail/edit are implemented. |
| Step 3: Resource foundation | Selector-grade browser context, egress profile, and file workspace catalogs/details. | Done for current scope | Core resource catalogs and detail/edit flows are implemented; browser-context clone/export/import lifecycle parity is implemented under #160. |
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
| Step 14: Remaining resource catalogs | Workflows, workflow runs, templates, extensions, credential bindings, event subscriptions, operation counters, identity/access review. | Partial | Workflow/run, identity/access, extension, credential-binding, and event-subscription catalogs exist. Session templates (#124), operation counters, and deeper workflow publishing remain. |
| Step 15: Dashboard | Read-only resource counts, recent operational activity, and links to active work. | Done for first pass | Dashboard overview exists and has smoke coverage. |
| Step 16: Command palette | Global navigation/session join/common creation actions without a second hidden state model. | Missing | No implemented command palette route/component. |
| Step 17: API reference/coverage companion | Copyable API examples, operation classification, OpenAPI link, compatibility separation. | Done for current contract | `/api`, `/coverage`, and `/docs` load strictly validated committed evidence, expose 19 schema-validated examples and 14 separate compatibility surfaces, and are compose-smoke covered. |
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
- `smoke:admin-unified-identity`
- `smoke:admin-unified-api-companion`
- `smoke:admin-unified-resource-catalogs`

These should remain mandatory for PRs that touch the unified admin app. #151
must make the selected minimal checks enforceable in CI; the existence of local
scripts alone is not a promotion gate.

## Promotion Assessment

The unified app is useful for manual testing and feature development, but it is
not ready to become default because:

1. Session-template and operation-counter catalogs plus command-palette behavior
   are incomplete.
2. Project-governance parity remains the active focused slice under #161;
   browser-context lifecycle parity merged through PR #203.
3. The explicit promotion, regression, and fallback gate in #163 has not run.
4. Some production security/operability slices remain outside admin parity.
