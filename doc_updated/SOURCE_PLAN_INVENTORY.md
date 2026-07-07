# Plan Source Inventory

This inventory consolidates all `docs/*PLAN*.md` files and the BPANE-00142
requirements workspace against the current `/admin-new` implementation.

Status values:

- Done: implemented and no longer a direct next admin-new slice.
- Partial: implemented in part; keep relevant remaining work.
- Active: current branch or PR contains the slice.
- Next: recommended near-term slice.
- Backlog: valid later work, not a current admin-new blocker.
- Superseded: old plan is covered by a newer route, issue, or consolidated
  plan.

## BPANE-00142 Unified Admin Plans

| Source plan | Status | Consolidated outcome |
| --- | --- | --- |
| `BPANE-00142_ADMIN_APP_REDESIGN_PLAN.md` | Active parent | Keep as historical audit trail. Durable consolidated state now lives in `docs/BPANE-00142_ADMIN_APP_REDESIGN/` and this `doc_updated/` folder. |
| `BPANE-00142_ADMIN_APP_REDESIGN/00_INDEX.md` | Active parent | Still valid as the versioned requirements workspace. |
| `BPANE-00142_ADMIN_APP_REDESIGN/01_CURRENT_ADMIN_PARITY.md` | Active | Parity list remains valid; unified admin has not reached promotion parity. |
| `BPANE-00142_ADMIN_APP_REDESIGN/02_CONCEPT_MAPPING.md` | Active | Route naming corrections remain important; `/admin-new/runs` should still gain `/workflow-runs` alias or route rename before promotion. |
| `BPANE-00142_ADMIN_APP_REDESIGN/03_API_COVERAGE.md` | Partial | API coverage planning exists, but `/admin-new/api` and `/coverage` are not implemented. |
| `BPANE-00142_ADMIN_APP_REDESIGN/04_IMPLEMENTATION_STEPS.md` | Active | Steps 0-5 and 15 are mostly done; steps 6-14 and 16-18 remain partial/missing. |
| `BPANE-00142_ADMIN_APP_REDESIGN/05_TEST_AND_SMOKE_MATRIX.md` | Active | Keep as validation source. Admin-new smokes now exist for core resource routes. |
| `BPANE-00142_ADMIN_APP_REDESIGN/06_SELECTOR_CONTRACT.md` | Active | Still valid. Route and smoke selectors should stay stable. |
| `BPANE-00142_ADMIN_APP_REDESIGN/07_PATTERN_LIBRARY.md` | Partial | Many practical components exist, but there is no explicit internal pattern-library route. |
| `BPANE-00142_ADMIN_APP_REDESIGN/08_MANUAL_CHECKPOINTS.md` | Active | Still useful; should be updated after each route-backed area lands. |
| `BPANE-00142_PROJECTS_EXISTING_EDIT_PLAN.md` | Done | Projects overview/create/detail/edit are implemented in `/admin-new/projects`. |
| `BPANE-00142_BROWSER_CONTEXTS_ADMIN_PLAN.md` | Done for first pass | Catalog/create/detail/edit are implemented. Clone/import/export can remain broader context-resource follow-up. |
| `BPANE-00142_EGRESS_PROFILES_ADMIN_PLAN.md` | Done for first pass | Catalog/create/edit exists. Production auth/provider variants are separate backend/security work. |
| `BPANE-00142_FILE_WORKSPACES_ADMIN_PLAN.md` | Done for first pass | File workspace catalog/create/detail/edit exists. Session-file subroute remains separate. |
| `BPANE-00142_SESSIONS_ADMIN_PLAN.md` | Partial | Sessions catalog, create, detail, preview, lifecycle, recording policy, and MCP delegation are present. Missing: route-backed session subareas. |
| `BPANE-00142_SESSION_CAPABILITIES_PLAN.md` | Done | Create-session capability controls are implemented and tested. |
| `BPANE-00142_SESSION_RECORDING_POLICY_PLAN.md` | Partial | Session recording policy and top-level recordings catalog exist. Session-specific recordings subroute and deeper playback management remain. |
| `BPANE-00142_SESSION_PREVIEW_METRICS_PLAN.md` | Done | Preview metrics drawer is implemented. |
| `BPANE-00142_RECORDING_LIFECYCLE_PLAN.md` | Partial | Lifecycle behavior was improved; recording artifact finalization boundary remains a separate cleanup/security slice. |
| `BPANE-00142_UNIFIED_ADMIN_MCP_DELEGATION_PLAN.md` | Done | Unified admin MCP delegation and session endpoint controls are implemented. |
| `BPANE-00142_MCP_BRIDGE_CONTROL_AUTH_PLAN.md` | Active PR | Implemented and pushed in PR `#143`. Direct `/control-session` is internal-bearer protected; gateway proxy handles owner auth. |
| `BPANE-00142_WORKFLOW_DEFINITIONS_ADMIN_PLAN.md` | Partial | Workflow catalog/detail/source/launch exist. Publishing/catalog-management gaps remain. |
| `BPANE-00142_WORKFLOW_CODE_PREVIEW_PLAN.md` | Done | Code preview with library-based TypeScript highlighting exists. |
| `BPANE-00142_WORKFLOW_SOURCE_BROWSER_PLAN.md` | Done for UI | Source tree and file navigation exist. Backend hardening is separate and active in the branch. |
| `BPANE-00142_WORKFLOW_EXECUTION_UI_PLAN.md` | Partial | Launch controls exist. Workflow-run detail controls/logs/events/produced files remain missing in unified admin. |
| `BPANE-00142_WORKFLOW_RUNS_PLAN.md` | Partial | Workflow runs overview exists at `/admin-new/runs`; detail route is missing. |
| `BPANE-00142_DASHBOARD_PLAN.md` | Done for first pass | Dashboard overview exists and has smoke coverage. |
| `BPANE-00142_WORKFLOW_SOURCE_HARDENING_PLAN.md` | Active PR | Implemented on the current branch and included in PR `#143`. |
| `BPANE-00142_REVIEW_CLEANUP_PRIORITIZATION_PLAN.md` | Active cleanup parent | This is the current security/runtime cleanup gate before admin-new promotion. |

## Non-00142 Plans Already Implemented Or Mostly Covered

| Source plan | Status | Consolidated outcome |
| --- | --- | --- |
| `BPANE-0015_BROWSER_CONTEXTS_PLAN.md` | Done | Control-plane/runtime/admin resource work is implemented; unified admin has browser-context routes. |
| `BPANE-0016_OPERATOR_CLI_PLAN.md` | Done | Operator CLI and delegation path exist; CLI smoke covers MCP actions. |
| `BPANE-0019_NETWORK_IDENTITY_EGRESS_PLAN.md` | Done baseline | Session network identity and egress profile primitives exist; unified egress UI exists. |
| `BPANE-0024_SESSION_TEMPLATES_CATALOG_PLAN.md` | Done backend/create use | Session templates exist and are selectable in create flows. Admin template catalog management remains `#124`. |
| `BPANE-0027_PROJECTS_QUOTAS_ADMISSION_PLAN.md` | Done first pass | Project resources/session quota admission exist; unified projects UI exists. Broader governance moved to `#132`. |
| `BPANE-0052_EXTERNAL_IDENTITY_ACCESS_REVIEW_PLAN.md` | Done backend/foundation | Backend identity/access review exists; unified admin identity route is missing. |
| `BPANE-0093_ADMIN_FEEDBACK_GAPS_PLAN.md` | Done | Core feedback/message gaps are closed in old admin and reused patterns appear in unified admin. |
| `BPANE-00119_LOCAL_WORKFLOW_MCP_HARDENING_PLAN.md` | Done | Landed previously through PR `#121`; current branch includes additional hardening beyond it. |
| `BPANE-00120_SESSION_RUNTIME_LIFECYCLE_PLAN.md` | Done | Runtime lifecycle safety landed previously. Unified sessions build on this behavior. |
| `BPANE-00120_SESSION_RUNTIME_LIFECYCLE_TEST_PLAN.md` | Superseded | Covered by runtime lifecycle validation and current session smokes. |
| `BPANE-00128_EGRESS_PROFILE_HARDENING_PLAN.md` | Done | Egress hardening/admin management landed; unified egress UI exists. |
| `BPANE-00129_PRODUCTION_PROXY_AUTH_VALIDATION_PLAN.md` | Done | Production proxy-auth validation landed. |
| `BPANE-00135_SERVICE_PRINCIPAL_REGISTRY_PLAN.md` | Done backend | Service principal registry exists; unified admin management route is missing. |
| `BPANE-00137_IDENTITY_PROVISIONING_PROJECT_MAPPING_PLAN.md` | Done backend | Identity/project mapping exists; unified admin route is missing. |
| `BPANE-00138_IDENTITY_ADMIN_POLISH_PLAN.md` | Done old-admin/backend | Identity polish exists in prior admin/backend work; unified admin identity route is missing. |

## Plans With Remaining Product/Admin-New Work

| Source plan | Status | Remaining work |
| --- | --- | --- |
| `BPANE-0093_SESSION_CREATION_CONFIGURATOR_PLAN.md` | Partial/superseded | Unified session create covers much of this. Keep only any missing compact configurator UX refinements if still relevant. |
| `BPANE-00132_PROJECT_GOVERNANCE_WORKFLOW_PLAN.md` | Partial | Backend/API governance work exists, but admin-new still needs richer project governance evidence, workflow quota visibility, and cross-resource policy UX. |
| `BPANE_OPEN_ISSUES_INTEGRATION_PLAN.md` | Superseded by newer state | Its historical issue status is useful, but it predates the current PR `#143` and the cleanup prioritization plan. Use only for issue lineage. |

## Backlog Or Roadmap Work Not To Mix Into Admin-New Promotion PRs

- `#28` generalized resource/security events and export
- `#30` exportable debug/support bundles
- `#47` workflow publishing and supported execution interfaces
- `#69` productized automation descriptors
- `#70` enterprise API keys, audit log, and retention policy controls
- `#71` signed human handoff and challenge fallback
- `#72` enterprise security baseline and threat model
- `#73` backup/restore and disaster recovery
- `#74` high availability and zero-downtime operations
- `#75` supply-chain security and release governance
- `#76` data residency, encryption, and BYOK
- `#79` central enterprise policy engine
- `#80` DLP/content inspection hooks

