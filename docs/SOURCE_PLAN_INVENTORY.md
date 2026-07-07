# Consolidated Legacy Plan Inventory

This file records which legacy planning topics were consolidated and what
active information was retained. It is intentionally self-contained: the old
planning files should not be needed to understand current work.

## Status Legend

- Implemented: the functional slice exists and is not a direct next step.
- Partial: some implementation exists, but remaining work is still relevant.
- Merged baseline: included in the current mainline baseline.
- Next: recommended near-term work.
- Backlog: valid later work, not an immediate admin-new blocker.
- Historical: useful lineage only; implementation guidance is superseded by the
  consolidated docs here.

## Active Admin-New Redesign Requirements

### Second-Pass Consolidation Additions

Status: Added to this consolidated workspace.

The first consolidation pass intentionally kept requirements compact. A second
audit against the highest-context legacy files added standalone documents for
details that should not be lost when the old planning folder is retired:

- `ADMIN_NEW_API_COVERAGE.md` preserves the OpenAPI operation classification
  matrix, compatibility endpoint inventory, schema/request/content/error
  parity rules, and API companion acceptance criteria.
- `ADMIN_NEW_MANUAL_CHECKPOINTS.md` preserves the route-by-route manual gates,
  old/new admin coexistence checks, and final promotion regression sequence.
- `PROJECT_GOVERNANCE_REQUIREMENTS.md` preserves project quota, queueing,
  usage, policy binding, egress privacy, retained-storage, and smoke
  requirements from the large project-governance plan.

A third audit against the older platform plans added:

- `RUNTIME_OPERATOR_REQUIREMENTS.md` for local workflow source trust, MCP,
  certificate recovery, runtime release/reconnect, and operator CLI behavior.
- `RESOURCE_LIFECYCLE_REQUIREMENTS.md` for browser contexts, session templates,
  network identity, egress profile diagnostics, local presets, and proxy-auth
  validation.
- `IDENTITY_ACCESS_REQUIREMENTS.md` for current identity, access review,
  service-principal registry, identity mappings, and admin/CLI behavior.
- `ADMIN_INTERACTION_REQUIREMENTS.md` for global feedback, panel-local
  messages, session creation configurator behavior, metrics, and logs.

A fourth audit against identifier-level gaps added:

- `ADMIN_NEW_IMPLEMENTATION_GUARDRAILS.md` for side-by-side app topology,
  old-admin parity anchors, API boundary extraction anchors, selector policy,
  pattern library guardrails, and route/prototype corrections.
- `LEGACY_DOC_RETENTION_AUDIT.md` for a per-file disposition table mapping
  every old planning document to its consolidated destination.

A fifth audit against legacy headings added:

- `LEGACY_SECTION_COVERAGE_AUDIT.md` for section-level coverage of the
  open-issues integration plan, review cleanup prioritization plan, large
  admin redesign implementation plan, admin requirements workspace, BPANE-00142
  slice plans, older domain/resource plans, and repeated validation sections.

A sixth audit against the raw `review/` reports added:

- `REVIEW_FINDINGS_RECONCILIATION.md` for the corrected review findings,
  including which security items are superseded by the current baseline and
  which token/auth/webhook/import/lifecycle/scalability/docs findings remain
  open.

### Redesign Foundation Decision

Status: Active requirement.

Retained decision:

- the redesign foundation is a task and resource control-plane model, not a
  generated API reference alone,
- the raw API contract remains important for integrators, examples, and
  coverage auditing,
- the operator UX should be built around resource routes, list/detail views,
  live session inspection, workflow execution, files, recordings, egress,
  identity, and diagnostics,
- OpenAPI/Redoc-style documentation is a companion surface, not the primary app
  structure,
- migration should preserve current live and inspect capabilities before
  adding broad new product scope.

### Unified Admin Shell And Promotion

Status: Partial.

Retained requirements:

- build `/admin-new/` beside `/admin/`,
- use static SvelteKit output,
- preserve `/admin/`, `/dist/`, auth config, cert metadata, and API routes,
- keep `/admin/` stable/default until parity and security gates pass,
- route-backed navigation and detail pages,
- explicit loading/error/empty/disabled/success states,
- manual checkpoints after each route migration,
- no old admin deletion until explicit promotion and fallback removal gate.
- implementation guardrails for static serving, package topology, old-admin
  parity anchors, selectors, and patterns are preserved in
  `ADMIN_NEW_IMPLEMENTATION_GUARDRAILS.md`.

Current implementation:

- shell and dashboard exist,
- grouped navigation exists,
- several navigation targets are still missing.

### Current Admin Parity

Status: Active requirement.

Retained behavior families:

- auth bootstrap, OIDC login completion, token refresh, logout, auth failure,
- global messages and route feedback,
- session lifecycle, connection control, queue control, stop/release/kill,
- live browser connect, browser SDK loading, certificate guidance, upload,
  microphone, camera, display, viewport sizing,
- session files and file workspaces,
- recordings and playback/export,
- MCP delegation and workflow operations,
- browser context lifecycle,
- egress profile management and diagnostics,
- identity/access review,
- metrics/logs/admin events,
- browser policy guardrails.

### Concept Mapping

Status: Active requirement.

Retained direction:

- operational layout, not landing page,
- persistent shell,
- route-oriented navigation,
- dashboard, sessions, contexts, egress, runs, workspaces, projects, workflows,
  identity, API/coverage companion,
- route-backed session tabs,
- exclude fake browser titlebar, mock URL text, mock data defaults, prototype
  component names, and external provider references,
- use product route names such as `/browser-contexts`,
  `/files/workspaces`, and `/workflow-runs`.

### API Coverage

Status: Partial.

Retained requirements:

- frozen owner-scoped contract is the v1 OpenAPI,
- classify every operation as UI-primary, UI-evidence, API-companion, or
  internal-worker,
- preserve security split between owner bearer and session automation access,
- keep compatibility endpoints separate from frozen API,
- expose high-risk schema families: lifecycle, content/export paths, egress
  diagnostics, observability counters, quotas, identity, automation refs.

Current gap:

- `/admin-new/api` and `/admin-new/coverage` are missing.

### Pattern Library And Selectors

Status: Partial.

Retained requirements:

- prefer local Svelte/TypeScript patterns,
- extract only when used or foundational,
- semantic selectors tied to product behavior,
- keep smoke selectors stable or update smoke and selector contract in same
  slice,
- avoid nested cards and cosmetic selector dependencies.

## Implemented Or Mostly Implemented Product Domains

### Local Workflow, MCP, Certificates, Setup

Status: Implemented with additional hardening merged through PR `#143`.

Retained outcomes:

- local `/workspace` workflow source trust must be explicit,
- workflow source errors should be structured and actionable,
- MCP bridge should use installed local `@playwright/mcp`,
- first connect should not download `@playwright/mcp@latest`,
- dev certificate metadata and recovery guidance must remain clear,
- local troubleshooting should cover certs, workflow source, and MCP bridge.
- detailed local setup, certificate, MCP, runtime, and CLI behavior is
  preserved in `RUNTIME_OPERATOR_REQUIREMENTS.md`.

### Session Runtime Lifecycle

Status: Implemented.

Retained outcomes:

- release runtime without deleting session resource,
- reconnect distinguishes exact live runtime from profile restart,
- stopped/released state must be visible,
- stop should be disabled while browser viewers are connected,
- disconnect-all enables lifecycle operations,
- stale tabs should not silently reconnect into wrong or broken state.

### Operator CLI And MCP Delegation

Status: Implemented.

Retained outcomes:

- JSON CLI surface for session and MCP operations,
- profile config with secure token storage,
- MCP doctor/preflight/repair,
- strict automation exit behavior,
- unsupported option rejection,
- CLI smoke remains mandatory for MCP/control changes.
- detailed command, profile, preflight, repair, and smoke behavior is preserved
  in `RUNTIME_OPERATOR_REQUIREMENTS.md`.

### Browser Context Resources

Status: Implemented core, partial admin parity.

Retained outcomes:

- first-class reusable browser context resources,
- runtime materialization with one active writer per context,
- admin visibility for references, active writer, storage, retention,
- clone/export/import exist as product requirements even where unified admin
  parity still lags.
- detailed context retention, storage, clone, export, import, and safety
  behavior is preserved in `RESOURCE_LIFECYCLE_REQUIREMENTS.md`.

### Network Identity And Egress

Status: Implemented baseline and hardening.

Retained outcomes:

- session/template network identity fields,
- egress profile resources,
- docker-backed runtime launch materialization,
- sanitized observer metadata and diagnostics,
- explicit TLS interception mode,
- local no-egress/proxy/TLS presets,
- proxy-auth fixture validation,
- no inline proxy credentials.
- detailed network identity, TLS interception, diagnostics, local presets, and
  proxy-auth behavior is preserved in `RESOURCE_LIFECYCLE_REQUIREMENTS.md`.

### Session Templates

Status: Implemented backend/create flow, admin catalog missing.

Retained outcomes:

- owner-scoped template resources with defaults,
- versioned updates,
- merge template defaults with session creation overrides,
- session catalog filters by template and metadata,
- admin-new creation uses templates,
- dedicated template catalog management remains open.
- detailed template defaults, merge, filtering, and smoke behavior is preserved
  in `RESOURCE_LIFECYCLE_REQUIREMENTS.md`.

### Projects, Quotas, Governance

Status: Implemented in several slices, still partial for admin-new governance.

Retained outcomes:

- project resources,
- session admission quotas,
- workflow-run project binding and quota admission,
- queue visibility and queue cancellation,
- usage budgets with warning/blocking modes,
- session creation rate limits,
- runtime-usage budgets,
- egress byte usage ingestion,
- project-scoped egress profiles and credential bindings,
- project policy bindings for templates, contexts, egress, extensions, file
  workspaces, uploads/downloads, session-file bindings, and recordings.
- detailed queue, budget, retained-storage, and policy behavior is preserved in
  `PROJECT_GOVERNANCE_REQUIREMENTS.md`.

Remaining:

- richer admin-new governance evidence,
- retained-storage/artifact quota UX,
- project audit/event hooks.

### Identity And Access Review

Status: Implemented backend and old-admin polish; missing unified route.

Retained outcomes:

- current principal,
- access review,
- service-principal registry/lifecycle,
- disabled service principals block new delegation,
- identity/project mappings for users/groups/claims/service principals,
- safe token claim rendering,
- unmapped signal evidence.
- detailed principal, service-principal, mapping, admin, CLI, and validation
  behavior is preserved in `IDENTITY_ACCESS_REQUIREMENTS.md`.

Remaining:

- `/admin-new/identity` route.

### Admin Feedback

Status: Implemented core feedback model.

Retained outcomes:

- global notifications,
- browser connection feedback,
- lifecycle messages,
- admin event stream health,
- selected-session external state diffs,
- workflow state messages,
- artifact/delegation snapshots.
- detailed global notification, panel-local message, session creation, metrics,
  and logs behavior is preserved in `ADMIN_INTERACTION_REQUIREMENTS.md`.

## Recently Merged Security Slices

### Workflow Source Hardening

Status: Merged baseline.

Retained outcomes:

- safe scheme validation,
- git protocol restrictions,
- explicit trusted local roots,
- symlink/path escape source preview rejection,
- materialization/listing limits,
- local workflow smokes preserved.

### MCP Bridge Control Auth

Status: Merged baseline.

Retained outcomes:

- direct bridge `/control-session` requires internal bearer,
- admin/CLI use gateway owner-auth proxy,
- gateway reconciles ambiguous bridge writes,
- session-scoped MCP endpoint remains directly usable,
- real MCP tool invocation smoke is required.

## Remaining Admin-New Work

### Workflow Runs

Status: Partial.

Needed:

- route-backed run detail,
- logs,
- events,
- produced files,
- run controls,
- intervention state,
- related session/workflow/project links.

### Session Subareas

Status: Partial.

Needed:

- live,
- files,
- recordings,
- network,
- automation,
- policy,
- observability.

### Recordings

Status: Partial.

Needed:

- session-specific recordings route,
- playback/export management,
- artifact-boundary hardening.

### Identity, API, Coverage, Docs Routes

Status: Missing.

Needed:

- `/admin-new/identity`,
- `/admin-new/api`,
- `/admin-new/coverage`,
- either implement or hide `/admin-new/docs`.

### Resource Catalogs Still Missing

Status: Missing/Backlog.

Needed:

- session template management,
- extensions,
- credential bindings,
- workflow event subscriptions,
- operation counters/internal automation evidence.

## Security And Runtime Roadmap Retained

Near-term:

- token domain separation and credential redaction,
- admin browser auth and web-security hardening,
- recording artifact finalization boundary,
- admin event auth without owner bearer query,
- webhook SSRF controls,
- browser context import safety,
- gateway lifecycle/readiness,
- admin/session catalog scalability,
- docker runtime launch boundary.

Enterprise backlog:

- automation descriptors,
- unified artifact/browser-output/recording-export model,
- generalized resource/security events,
- API keys, audit log, retention policies,
- signed human handoff/challenge fallback,
- support/debug bundles,
- production deployment packaging,
- backup/restore,
- HA,
- supply-chain governance,
- data residency/BYOK,
- DLP/content inspection,
- central policy engine.

## Historical Issue Lineage Retained As Status

Completed implementation packages:

- local workflow/MCP/certificate/setup hardening,
- session runtime lifecycle safety,
- operator CLI and delegation path,
- session templates/catalog APIs,
- browser context resources,
- egress profile hardening,
- production proxy-auth validation,
- first project resources and session quota admission,
- first identity/access-review foundation,
- service-principal registry,
- identity provisioning/project mappings,
- admin identity polish and safe token claim evaluation.

Current parent issue:

- BPANE-00142 remains the unified admin redesign parent.

Current baseline checkpoint:

- merged PR `#143` snapshots current unified admin, workflow source hardening,
  MCP control auth, and documentation state. It did not close the parent
  redesign issue.

## Legacy Inputs Represented

The following legacy planning inputs are represented by the topics above. This
list is retained only as a deletion audit checklist; the active requirements are
the consolidated sections in this folder.

Admin redesign and migration:

- admin app redesign foundation
- BPANE-00142 admin app redesign implementation plan
- BPANE-00142 admin redesign requirements workspace
- projects existing-project edit plan
- browser contexts admin plan
- egress profiles admin plan
- file workspaces admin plan
- sessions admin plan
- session capabilities plan
- session recording policy plan
- session preview metrics plan
- recording lifecycle plan
- unified admin MCP delegation plan
- MCP bridge control auth plan
- workflow definitions admin plan
- workflow code preview plan
- workflow source browser/editor plan
- workflow execution UI plan
- workflow runs plan
- dashboard plan
- workflow source hardening plan
- review cleanup prioritization plan

Implemented foundation plans:

- local workflow/MCP/certificate/setup hardening
- session runtime lifecycle hardening
- session runtime lifecycle test plan
- browser context resources
- operator CLI, MCP delegation, and cleanup
- network identity and egress
- session templates and catalog
- first project quotas/admission
- external identity/access review
- service principal registry
- identity provisioning/project mapping
- identity admin polish
- admin feedback gaps
- egress profile hardening
- production proxy-auth validation

Partially retained or superseded plans:

- project governance workflow plan
- session creation configurator plan
- open issues integration plan

Exact legacy filenames represented in this consolidation:

- `ADMIN_APP_REDESIGN_FOUNDATION.md`
- `BPANE-00119_LOCAL_WORKFLOW_MCP_HARDENING_PLAN.md`
- `BPANE-00120_SESSION_RUNTIME_LIFECYCLE_PLAN.md`
- `BPANE-00120_SESSION_RUNTIME_LIFECYCLE_TEST_PLAN.md`
- `BPANE-00128_EGRESS_PROFILE_HARDENING_PLAN.md`
- `BPANE-00129_PRODUCTION_PROXY_AUTH_VALIDATION_PLAN.md`
- `BPANE-00132_PROJECT_GOVERNANCE_WORKFLOW_PLAN.md`
- `BPANE-00135_SERVICE_PRINCIPAL_REGISTRY_PLAN.md`
- `BPANE-00137_IDENTITY_PROVISIONING_PROJECT_MAPPING_PLAN.md`
- `BPANE-00138_IDENTITY_ADMIN_POLISH_PLAN.md`
- `BPANE-00142_ADMIN_APP_REDESIGN_PLAN.md`
- `BPANE-00142_BROWSER_CONTEXTS_ADMIN_PLAN.md`
- `BPANE-00142_DASHBOARD_PLAN.md`
- `BPANE-00142_EGRESS_PROFILES_ADMIN_PLAN.md`
- `BPANE-00142_FILE_WORKSPACES_ADMIN_PLAN.md`
- `BPANE-00142_MCP_BRIDGE_CONTROL_AUTH_PLAN.md`
- `BPANE-00142_PROJECTS_EXISTING_EDIT_PLAN.md`
- `BPANE-00142_RECORDING_LIFECYCLE_PLAN.md`
- `BPANE-00142_REVIEW_CLEANUP_PRIORITIZATION_PLAN.md`
- `BPANE-00142_SESSIONS_ADMIN_PLAN.md`
- `BPANE-00142_SESSION_CAPABILITIES_PLAN.md`
- `BPANE-00142_SESSION_PREVIEW_METRICS_PLAN.md`
- `BPANE-00142_SESSION_RECORDING_POLICY_PLAN.md`
- `BPANE-00142_UNIFIED_ADMIN_MCP_DELEGATION_PLAN.md`
- `BPANE-00142_WORKFLOW_CODE_PREVIEW_PLAN.md`
- `BPANE-00142_WORKFLOW_DEFINITIONS_ADMIN_PLAN.md`
- `BPANE-00142_WORKFLOW_EXECUTION_UI_PLAN.md`
- `BPANE-00142_WORKFLOW_RUNS_PLAN.md`
- `BPANE-00142_WORKFLOW_SOURCE_BROWSER_PLAN.md`
- `BPANE-00142_WORKFLOW_SOURCE_HARDENING_PLAN.md`
- `BPANE-0015_BROWSER_CONTEXTS_PLAN.md`
- `BPANE-0016_OPERATOR_CLI_PLAN.md`
- `BPANE-0019_NETWORK_IDENTITY_EGRESS_PLAN.md`
- `BPANE-0024_SESSION_TEMPLATES_CATALOG_PLAN.md`
- `BPANE-0027_PROJECTS_QUOTAS_ADMISSION_PLAN.md`
- `BPANE-0052_EXTERNAL_IDENTITY_ACCESS_REVIEW_PLAN.md`
- `BPANE-0093_ADMIN_FEEDBACK_GAPS_PLAN.md`
- `BPANE-0093_SESSION_CREATION_CONFIGURATOR_PLAN.md`
- `BPANE_OPEN_ISSUES_INTEGRATION_PLAN.md`
- `BPANE-00142_ADMIN_APP_REDESIGN/00_INDEX.md`
- `BPANE-00142_ADMIN_APP_REDESIGN/01_CURRENT_ADMIN_PARITY.md`
- `BPANE-00142_ADMIN_APP_REDESIGN/02_CONCEPT_MAPPING.md`
- `BPANE-00142_ADMIN_APP_REDESIGN/03_API_COVERAGE.md`
- `BPANE-00142_ADMIN_APP_REDESIGN/04_IMPLEMENTATION_STEPS.md`
- `BPANE-00142_ADMIN_APP_REDESIGN/05_TEST_AND_SMOKE_MATRIX.md`
- `BPANE-00142_ADMIN_APP_REDESIGN/06_SELECTOR_CONTRACT.md`
- `BPANE-00142_ADMIN_APP_REDESIGN/07_PATTERN_LIBRARY.md`
- `BPANE-00142_ADMIN_APP_REDESIGN/08_MANUAL_CHECKPOINTS.md`
