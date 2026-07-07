# BrowserPane Consolidated Planning Workspace

Created: 2026-07-07

This folder is the standalone planning workspace for the active BrowserPane
plan set. It consolidates the still-valid information from the old plan files
and the current implementation state. The previous scattered planning folder
was consolidated into this current `docs/` layout so active requirements,
roadmaps, and validation expectations can be maintained from one place.

The consolidation is scoped around the current BPANE-00142 unified admin app
work and the cleanup/security plan that now gates promotion. It intentionally
keeps the focus on what is implemented, what is still valid, and what should be
worked next.

Legacy inputs that were consolidated:

- all project plan files under the previous planning folder
- the BPANE-00142 admin redesign requirements workspace
- the open-issues integration plan
- the current implementation under `code/web/bpane-admin-unified`

Current branch context:

- `/admin/` remains the stable/default admin console.
- `/admin-new/` is the route-backed unified admin app under active
  development.
- PR `#143` has been merged and is the current baseline for `/admin-new`,
  workflow source hardening, and MCP control-auth hardening.
- Issue `#142` remains the parent redesign issue and should not be closed by
  follow-up implementation PRs until the promotion gate is actually reached.

## Files

- `ADMIN_NEW_STATUS.md`: current `/admin-new` implementation state versus the
  consolidated redesign requirements.
- `ADMIN_NEW_REQUIREMENTS.md`: standalone information architecture, parity,
  route, UX, selector, and pattern requirements for the unified admin app.
- `ADMIN_NEW_API_COVERAGE.md`: owner-scoped OpenAPI coverage, operation
  classification, compatibility endpoints, schema parity, and API companion
  requirements.
- `ADMIN_NEW_IMPLEMENTATION_GUARDRAILS.md`: side-by-side package topology,
  old-admin parity anchors, API extraction anchors, selector policy, pattern
  library guardrails, and prototype route corrections.
- `ADMIN_NEW_MANUAL_CHECKPOINTS.md`: route-by-route manual validation gates and
  final regression sequence for the side-by-side admin migration.
- `ADMIN_INTERACTION_REQUIREMENTS.md`: global notifications, panel-local
  feedback, session creation configurator, metrics, and logs.
- `DOMAIN_REQUIREMENTS.md`: standalone control-plane and product-domain
  requirements that remain relevant to the admin app and future slices.
- `IDENTITY_ACCESS_REQUIREMENTS.md`: current principal, access review,
  service-principal registry, identity mappings, and admin/CLI requirements.
- `LEGACY_DOC_RETENTION_AUDIT.md`: per-file disposition table for every legacy
  planning document and the consolidated destination that now owns its active
  context.
- `LEGACY_SECTION_COVERAGE_AUDIT.md`: section-level audit showing how major
  headings and intent from the legacy plans map into the consolidated
  workspace.
- `PROJECT_GOVERNANCE_REQUIREMENTS.md`: detailed project quota, admission,
  queueing, policy binding, usage, retained-storage, and privacy-boundary
  requirements.
- `REVIEW_FINDINGS_RECONCILIATION.md`: comparison of the raw `review/`
  findings against the consolidated docs and current tree, including which
  findings are superseded, still open, or newly promoted into the roadmap.
- `RESOURCE_LIFECYCLE_REQUIREMENTS.md`: browser-context, session-template,
  network identity, egress profile, diagnostics, and proxy-auth lifecycle
  requirements.
- `RUNTIME_OPERATOR_REQUIREMENTS.md`: local setup, workflow source, MCP,
  certificate, runtime release/reconnect, and operator CLI requirements.
- `SOURCE_PLAN_INVENTORY.md`: compact legacy plan-to-topic map with the
  retained requirements embedded by topic.
- `NEXT_WORKING_ROADMAP.md`: prioritized remaining work, keeping admin-new
  promotion in view.
- `SECURITY_RUNTIME_ROADMAP.md`: security, runtime, scale, and production
  hardening roadmap extracted from the review cleanup plan.
- `VALIDATION_MATRIX.md`: focused validation and smoke expectations for the
  current state and upcoming slices.

## High-Level Result

The unified admin app has meaningful route-backed coverage now:

- dashboard
- projects
- browser contexts
- egress profiles
- file workspaces
- sessions
- session creation
- session preview popup
- recordings overview
- workflows
- workflow runs overview
- MCP delegation controls in session detail

The unified admin app is not ready to replace `/admin/` yet. The important
remaining admin-new gaps are:

- route-backed workflow-run detail with logs, events, controls, input, and
  produced files
- route-backed session subareas for files, network, policy, automation, and
  observability
- identity/access review route
- API companion and API coverage route
- session template catalog management
- extensions catalog
- credential bindings catalog
- workflow event subscriptions catalog
- command palette
- explicit promotion/cutover gate

The cleanup/security plan also identifies hardening work that should happen
before promotion is treated as production-ready:

- token domain separation and URL credential redaction
- admin browser auth and web-security hardening
- admin event stream auth without owner bearer query parameters
- webhook SSRF controls
- browser context import safety
- gateway lifecycle/readiness
- admin/session catalog scalability

## Second-Pass Audit Result

The consolidation was checked again against the high-context legacy files, not
only against filenames. The second pass added standalone coverage for:

- the detailed OpenAPI operation classification matrix,
- non-OpenAPI compatibility endpoints and MCP/runtime helper surfaces,
- schema/request/content/error parity requirements,
- route-by-route manual checkpoints,
- the final promotion regression sequence,
- detailed project governance quota, queue, usage, policy, and retained-storage
  behavior.

The third pass added standalone coverage for the older platform plans that
pre-date the `/admin-new` redesign but still matter for implementation:

- local workflow source trust, MCP startup, and certificate recovery,
- session runtime release/reconnect/stopped semantics,
- operator CLI command behavior and profile safety,
- browser-context clone/export/import/retention/storage lifecycle,
- session-template catalog semantics,
- network identity and egress/proxy-auth diagnostics,
- identity access-review, service-principal registry, and identity mappings,
- global feedback and session creation configurator behavior.

The fourth pass added:

- old-admin route and symbol parity anchors,
- API boundary extraction and mapper/test anchors,
- selector manifest and pattern library details,
- side-by-side static serving/build guardrails,
- concept/prototype route corrections,
- a per-file retention audit for all legacy planning documents.

The fifth pass added a section-level coverage audit for the high-context legacy
plans. It confirms that the open-issues merge maps, review cleanup slices,
large admin redesign steps, domain/resource plans, and repeated smoke sections
are represented by current consolidated documents instead of by references back
to the old folder.

The sixth pass reconciled the raw `review/` folder. It confirmed that workflow
source RCE/preview-symlink and bridge-local control-auth findings are now
superseded by later hardening, while token-domain separation, admin web auth,
webhook SSRF, browser-context import limits, graceful shutdown/readiness,
control-plane aggregation scalability, and durable documentation fixes remain
active roadmap items.

## Maintenance Rule

When maintaining this planning workspace, verify that:

1. every active requirement is represented in this folder,
2. obsolete historical issue merge maps are either intentionally dropped or
   summarized in `SOURCE_PLAN_INVENTORY.md`,
3. section-level coverage remains represented in
   `LEGACY_SECTION_COVERAGE_AUDIT.md`,
4. contributor guidance points future planning documents at `docs/`,
5. the promotion gate in `ADMIN_NEW_STATUS.md` still reflects the live code.
