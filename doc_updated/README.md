# Consolidated BrowserPane Plans

Created: 2026-07-07

This folder consolidates the plan files from `docs/` without modifying the
source plans. The consolidation is scoped around the current BPANE-00142 unified
admin app work and the cleanup/security plan that now gates promotion.

Source folders and files reviewed:

- `docs/*PLAN*.md`
- `docs/BPANE_OPEN_ISSUES_INTEGRATION_PLAN.md`
- `docs/BPANE-00142_ADMIN_APP_REDESIGN/`
- current implementation under `code/web/bpane-admin-unified`

Current branch context:

- `/admin/` remains the stable/default admin console.
- `/admin-new/` is the route-backed unified admin app under active
  development.
- PR `#143` snapshots the current `/admin-new` and MCP control-auth state for
  review.
- Issue `#142` remains the parent redesign issue and should not be closed by
  the current PR.

## Consolidated Files

- `ADMIN_NEW_STATUS.md`: current `/admin-new` implementation state versus the
  BPANE-00142 redesign plan.
- `SOURCE_PLAN_INVENTORY.md`: every source plan file and how it maps to current
  implemented, partial, or deferred work.
- `NEXT_WORKING_ROADMAP.md`: prioritized remaining work, keeping admin-new
  promotion in view.
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
- admin event stream auth without owner bearer query parameters
- recording artifact finalization boundary
- webhook SSRF controls
- browser context import safety
- gateway lifecycle/readiness
- admin/session catalog scalability
