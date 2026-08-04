# Admin-New Implementation Guardrails

This document preserves implementation-level context from the old admin
redesign plans. It is not a product requirements list; it is the checklist for
keeping the new app aligned with the existing app, package topology, smoke
selectors, and route migration constraints.

## Side-By-Side App Topology

The new admin app must remain a separate SvelteKit package mounted beside the
current admin app until an explicit promotion decision is accepted.

Required topology:

- current app package stays mounted at `/admin/`,
- new app package lives at `code/web/bpane-admin-unified`,
- new app is mounted at `/admin-new/`,
- local and compose builds use static SvelteKit output,
- use `@sveltejs/adapter-static` with `fallback: 'index.html'`,
- route base comes from `BPANE_ADMIN_BASE_PATH`,
- local build/dev scripts set `BPANE_ADMIN_BASE_PATH=/admin-new` for the new
  package only,
- keep the current admin toolchain shape unless deliberately changed:
  SvelteKit/Svelte 5, Vite, TypeScript, Tailwind 4, `lucide-svelte`, and
  Vitest,
- create source/config/static-shell equivalents for `package.json`,
  `package-lock.json`, `svelte.config.js`, `vite.config.ts`, `tsconfig.json`,
  `src/app.html`, `src/app.css`, and `static/browserpane-logo.png`,
- never copy generated or vendored output such as `.svelte-kit/`, `build/`, or
  `node_modules/` into source control.

Required web serving behavior:

- keep the existing `build-admin` web build stage for `code/web/bpane-admin`,
- add a separate build stage for `code/web/bpane-admin-unified`,
- copy the new build output to `/usr/share/nginx/html/admin-new/`,
- keep the shared browser SDK at `/usr/share/nginx/html/dist/`,
- `location = /admin-new` redirects to `/admin-new/`,
- `/admin-new/` deep links fall back to `/admin-new/index.html`,
- preserve existing `/admin/`, `/dist/`, `/auth-config.json`, `/cert-hash`,
  `/cert-fingerprint`, and `/api/` behavior.

Baseline manual check:

1. Open `/admin-new/` and complete login.
2. Refresh `/admin-new/sessions` through the compose web server.
3. Verify `/admin/` is unchanged.
4. Verify `/dist/bpane.js`, `/auth-config.json`, `/cert-hash`, and
   `/cert-fingerprint` still resolve.
5. Keep `concept.html` and generated `dev/certs/` drift local unless
   explicitly asked to commit them.

## Current App Parity Anchors

The new app does not need to copy symbols one-for-one, but each behavior must
land in a route-backed destination, a tested replacement, or an explicit
deferral.

Old app route files to keep covered while migrating behavior:

- `code/web/bpane-admin/src/routes/+layout.svelte`
- `code/web/bpane-admin/src/routes/+page.svelte`
- `code/web/bpane-admin/src/routes/sessions/+page.svelte`
- `code/web/bpane-admin/src/routes/sessions/[session_id]/+page.svelte`
- `code/web/bpane-admin/src/routes/browser-contexts/+page.svelte`
- `code/web/bpane-admin/src/routes/files/workspaces/+page.svelte`
- `code/web/bpane-admin/src/routes/files/workspaces/[workspace_id]/+page.svelte`
- `code/web/bpane-admin/src/routes/workflows/+page.svelte`
- `code/web/bpane-admin/src/routes/workflows/[workflow_id]/+page.svelte`
- `code/web/bpane-admin/src/routes/workflow-runs/+page.svelte`
- `code/web/bpane-admin/src/routes/workflow-runs/[run_id]/+page.svelte`
- route loaders under sessions, workspaces, workflows, and workflow-runs.

Old app behavior anchors:

- shell/cross-route state: `AdminRouteShell`, `AdminHeader`,
  `AdminMessage`, `AdminSessionSurface`, `BrowserWorkspaceOverlayLayout`,
  `AdminWorkspaceTabs`,
- sessions: `AdminSessionListRoute`, `SessionListPanel`, `SessionTable`,
  `AdminSessionDetailRoute`, `SessionDetailPanel`,
  `SessionCreateConfigurator`,
- live operations: `BrowserEmbedPanel`, `LiveSessionActionsSurface`,
  `LiveSessionActionsPanel`, `DisplayControlsSurface`,
  `DisplayControlsPanel`, `SessionLifecycleSurface`,
- files and policy: `SessionFilesSurface`, `SessionFileBindingsSurface`,
  `SessionFileCard`, `BrowserPolicySurface`, `BrowserPolicyPanel`,
- recordings: `RecordingSurface`, `RecordingPanel`,
  `RecordingSegmentCard`, `recording-downloads`,
- egress: `EgressProfileCatalogPanel`, `egress-profile-catalog`,
  `local-egress-presets`,
- automation/workflows: `McpDelegationSurface`, `McpDelegationPanel`,
  `mcp-delegation-view-model`, `WorkflowOperationsSurface`,
  `WorkflowOperationsPanel`, `workflow-operations-service`,
  `workflow-definition-visibility`, `workflow-error-messages`,
  `workflow-template-catalog-view-model`,
- observability: `MetricsSurface`, `MetricsPanel`, `metrics-view-model`,
  `metrics-diagnostics-payload`, `metrics-sample-extrema`, `LogsSurface`,
  `LogsPanel`, `logs-view-model`, `admin-log-entries`,
  `admin-session-event-sync`, `admin-workflow-follow`,
  `admin-feedback-notifications`,
- resource catalogs: `AdminBrowserContextListRoute`,
  `BrowserContextCatalogPanel`, `AdminFileWorkspaceListRoute`,
  `AdminFileWorkspaceDetailRoute`, `file-workspace-view-model`,
  `AdminWorkflowCatalogRoute`, `AdminWorkflowDefinitionDetailRoute`,
  `AdminWorkflowRunListRoute`, `AdminWorkflowRunDetailRoute`,
  `IdentityAccessReviewPanel`, `identity-access-review-view-model`,
  `identity-mapping-catalog`, `service-principal-catalog`,
- `FeaturePlaceholderPanel` must not remain on promoted routes except for
  explicit deferrals.

## API Boundary Extraction Anchors

Do not replace or extract old admin API modules without equivalent mapper,
client, and route tests.

Shared transport/error handling:

- `authenticated-api`,
- `control-wire`,
- `AccessTokenProvider`,
- `AuthenticationFailureHandler`,
- `ControlApiError`,
- `ControlApiErrorDetails`,
- `FetchLike`,
- `parseControlApiErrorBody`,
- `expectBoolean`, `expectNumber`, `expectRecord`, `expectString`,
  `expectStringRecord`, `optionalString`.

Owner control API:

- `control-client`,
- `control-types`,
- `control-session-mapper`,
- `control-session-status-mapper`,
- `control-session-file-mapper`,
- `control-file-workspace-mapper`,
- `session-status-types`,
- `ControlClient`,
- `ControlClientOptions`,
- `ControlSessionMapper`,
- `ControlSessionStatusMapper`,
- `ControlSessionFileMapper`,
- `ControlFileWorkspaceMapper`,
- CSV filter encoding used by `appendCsvFilter`.

Admin events:

- `admin-event-client`,
- `admin-event-mapper`,
- `admin-event-snapshots`,
- `admin-event-stream-access`,
- `AdminEventClient`,
- `AdminEventStreamAccessMapper`,
- `AdminEventConnectionStatus`,
- `AdminSessionsSnapshotEvent`,
- `AdminSessionFilesSnapshotEvent`,
- `AdminRecordingsSnapshotEvent`,
- `AdminWorkflowRunsSnapshotEvent`,
- `AdminMcpDelegationSnapshotEvent`,
- scoped token response validation and query-free URL construction through
  `AdminEventStreamAccessMapper`.

Other API boundaries:

- workflow: `workflow-client`, `workflow-types`, `workflow-mapper`,
  `workflow-run-mapper`,
- recordings: `recording-types`, `recording-mapper`,
- local integration: `mcp-bridge-client`, `local-egress-preset-types`,
  `McpBridgeClient`, `McpBridgeHealth`, `McpBridgeControlSession`,
  `LOCAL_EGRESS_PRESET_LABEL_KEY`, `LOCAL_EGRESS_PROXY_NAME`,
  `LOCAL_EGRESS_PROXY_PRESET`, `LOCAL_EGRESS_TLS_NAME`,
  `LOCAL_EGRESS_TLS_PRESET`.

Test anchors to preserve or replace:

- `control-client.test.ts`,
- `control-file-workspace-client.test.ts`,
- `control-file-workspace-mapper.test.ts`,
- `control-session-files.test.ts`,
- `control-session-status-client.test.ts`,
- `recording-client.test.ts`,
- `workflow-client.test.ts`,
- `admin-event-client.test.ts`,
- `authenticated-api.test.ts`,
- `mcp-bridge-client.test.ts`.

## Supporting Behavior Anchors

Behavior modules that need tested migration or explicit replacement:

- selection and route state: `admin-session-selection`,
  `admin-route-context`, `admin-workspace-view-model`,
- messaging/accessibility: `admin-message-types`,
  `admin-message-accessibility`,
- live connection and local trust: `browser-session-connector`,
  `browser-session-sdk-loader`, `browser-session-types`,
  `certificate-metadata`,
- auth/session bootstrap: `auth-config`, `browser-token-store`,
  `oidc-auth-client`, `oidc-endpoint-client`, `oidc-types`,
  `oidc-wire-mapper`, `pkce-codec`,
- session/files/policy/display/recording/metrics/workflow view models:
  `session-view-model`, `session-file-view-model`, `session-file-format`,
  `browser-policy-view-model`, `display-settings-view-model`,
  `live-session-actions-view-model`, `recording-view-model`,
  `recording-format`, `metrics-diagnostics-types`,
  `workflow-operations-view-model`,
- resource view models: `browser-context-view-model`,
  `egress-profile-catalog`, `file-workspace-view-model`,
  `identity-access-review-view-model`, `identity-mapping-catalog`,
  `service-principal-catalog`, `mcp-delegation-view-model`,
  `metrics-view-model`, `logs-view-model`,
  `workflow-template-catalog-view-model`.

Non-API tests that should remain represented:

- application tests such as `admin-feedback-notifications.test.ts`,
  `admin-log-entries.test.ts`, `admin-session-event-sync.test.ts`,
  `admin-session-selection.test.ts`, `admin-workflow-follow.test.ts`,
  `local-egress-presets.test.ts`, `workflow-definition-visibility.test.ts`,
  `workflow-error-messages.test.ts`, `workflow-operations-service.test.ts`,
- auth/security/session tests such as `auth-config.test.ts`,
  `oidc-auth-client.test.ts`, `certificate-metadata.test.ts`,
  `browser-session-connector.test.ts`, `browser-session-sdk-loader.test.ts`,
- presentation/view-model tests such as `admin-message-accessibility.test.ts`,
  `admin-workspace-view-model.test.ts`, `browser-context-view-model.test.ts`,
  `browser-policy-view-model.test.ts`, `display-settings-view-model.test.ts`,
  `egress-profile-catalog.test.ts`, `file-workspace-view-model.test.ts`,
  `identity-access-review-view-model.test.ts`,
  `identity-mapping-catalog.test.ts`,
  `live-session-actions-view-model.test.ts`, `logs-view-model.test.ts`,
  `mcp-delegation-view-model.test.ts`, `metrics-view-model.test.ts`,
  `recording-view-model.test.ts`, `service-principal-catalog.test.ts`,
  `session-create-configurator.test.ts`, `session-file-format.test.ts`,
  `session-file-view-model.test.ts`, `session-view-model.test.ts`,
  `workflow-operations-view-model.test.ts`,
  `workflow-template-catalog-view-model.test.ts`.

## Selector Manifest Policy

Before route promotion, generate or update a selector manifest from:

- `code/web/bpane-admin/src`,
- impacted `code/web/bpane-client/scripts/*.mjs`.

The manifest must include:

- `data-testid`,
- `data-action`,
- `aria-label`,
- route paths,
- key DOM selectors used by Playwright scripts.

Old audit baseline:

- 319 current source `data-testid` values,
- 1 current source `data-action` value,
- 10 smoke `data-testid` values,
- 2 smoke `data-action` values.

High-risk selectors to preserve or update with smokes in the same slice:

- `browser-viewport`,
- `browser-viewport-mount`,
- `session-row`,
- `session-inspector-row`,
- `session-join`,
- `session-disconnect`,
- `session-detail-link`,
- `admin-log-entry`,
- `admin-global-message-region`,
- `recording-library-row`,
- `recording-segment-download`,
- `download-recording`,
- `file-workspace-file-row`,
- `session-file-binding-row`,
- `session-file-binding-download`,
- `egress-profile-row`,
- `egress-profile-edit`,
- `egress-profile-clone`,
- `egress-profile-reachability-probe`,
- `browser-context-row`,
- `browser-context-clone`,
- `browser-context-import`,
- `browser-context-export`,
- `workflow-run-inspector-row`,
- `workflow-catalog-row`,
- `identity-service-principal-row`,
- `identity-mapping-row`,
- `download-workflow-file`.

Selector policy:

- prefer semantic selectors tied to product behavior,
- avoid cosmetic wrapper selectors,
- keep route IDs, row IDs, and selected-state attributes explicit,
- if a selector is removed, update the smoke and manifest in the same slice.

## Pattern Library Guardrails

The new app should use a small internal pattern library, not a public design
system.

Pattern folder:

- `code/web/bpane-admin-unified/src/lib/patterns`.

Optional internal route:

- `/admin-new/patterns`.

Initial reusable patterns:

- `AppShell`,
- `PageHeader`,
- `ResourceList`,
- `ResourceSummary`,
- `DetailTabs`,
- `ActionBar`,
- `StatusBadge`,
- `FeedbackMessage`,
- `EmptyState`,
- `LoadingState`,
- `ErrorState`,
- `FormSection`,
- `FieldRow`,
- `PayloadPreview`,
- `DangerZone`,
- `FileDropUpload`,
- `DownloadAction`,
- `LiveViewportFrame`,
- `CopyButton`,
- `CommandPalette`.

Pattern acceptance:

- at least one real route consumer,
- component or view-model test where logic exists,
- documented loading, empty, disabled, and error states,
- stable selectors if smoke scripts use it,
- no nested cards,
- compact operational density,
- accessible labels and keyboard behavior.

## Concept And Route Corrections

Use concept.html for direction only. Do not copy production code or mock-only
features from it.

Keep:

- persistent shell,
- route-oriented navigation,
- dashboard,
- sessions catalog/detail,
- live attach,
- resource catalogs,
- global attach banner,
- command palette,
- API/coverage companion,
- compact operational layout.

Add from current app/API:

- recordings,
- network and egress diagnostics,
- automation and MCP delegation,
- browser policy,
- observability,
- session templates,
- extensions,
- credential bindings,
- workflow event subscriptions and deliveries,
- operation counters,
- automation-task evidence.

Exclude or defer:

- fake browser titlebar,
- mock URL text,
- mock data defaults,
- prototype names in route names or UI copy,
- `ShareTokenForm` until a backend share/handoff contract exists,
- external provider references from the concept memo.

Route naming corrections:

- use `/admin-new/workflow-runs`, not `/admin-new/runs`,
- use `/admin-new/browser-contexts`, not `/admin-new/contexts`,
- use `/admin-new/files/workspaces`,
- use `/admin-new/sessions/[session_id]/live`,
- use `/admin-new/sessions/[session_id]/recordings`,
- use `/admin-new/sessions/[session_id]/network`,
- use `/admin-new/sessions/[session_id]/automation`,
- use `/admin-new/sessions/[session_id]/policy`,
- use `/admin-new/sessions/[session_id]/observability`.

Historical references to `/admin/sessions`, `/admin/workflows`,
`/admin/workflow-runs`, `/admin/browser-contexts`, `/admin/files/workspaces`,
`/admin/egress`, `/admin/identity`, `/admin/api`, and `/admin/projects` refer
to old-admin parity or promotion target concepts. They should not be used to
erase the current side-by-side rule that `/admin/` remains stable while
`/admin-new/` is developed.
