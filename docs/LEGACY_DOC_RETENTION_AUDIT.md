# Legacy Document Retention Audit

This file records the final consolidation disposition for every planning
document that existed before the current `docs/` layout was created. It is
intentionally explicit so the removed legacy documents do not need to be
restored to understand where their still-valid content went.

Disposition meanings:

- embedded: active requirements are represented in standalone consolidated
  topic docs.
- historical: old branch names, old issue merge maps, or already-completed
  execution notes are retained only as lineage in `SOURCE_PLAN_INVENTORY.md`.
- superseded: implementation path changed, but the valid product requirement is
  represented in a consolidated doc.

## Top-Level Documents

| Legacy document | Disposition | Consolidated destination |
| --- | --- | --- |
| `ADMIN_APP_REDESIGN_FOUNDATION.md` | embedded | `ADMIN_NEW_REQUIREMENTS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md`, `ADMIN_NEW_IMPLEMENTATION_GUARDRAILS.md` |
| `BPANE-00119_LOCAL_WORKFLOW_MCP_HARDENING_PLAN.md` | embedded | `RUNTIME_OPERATOR_REQUIREMENTS.md`, `SECURITY_RUNTIME_ROADMAP.md`, `VALIDATION_MATRIX.md` |
| `BPANE-00120_SESSION_RUNTIME_LIFECYCLE_PLAN.md` | embedded | `RUNTIME_OPERATOR_REQUIREMENTS.md`, `DOMAIN_REQUIREMENTS.md`, `VALIDATION_MATRIX.md` |
| `BPANE-00120_SESSION_RUNTIME_LIFECYCLE_TEST_PLAN.md` | embedded | `RUNTIME_OPERATOR_REQUIREMENTS.md`, `VALIDATION_MATRIX.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md` |
| `BPANE-00128_EGRESS_PROFILE_HARDENING_PLAN.md` | embedded | `RESOURCE_LIFECYCLE_REQUIREMENTS.md`, `DOMAIN_REQUIREMENTS.md`, `VALIDATION_MATRIX.md` |
| `BPANE-00129_PRODUCTION_PROXY_AUTH_VALIDATION_PLAN.md` | embedded | `RESOURCE_LIFECYCLE_REQUIREMENTS.md`, `VALIDATION_MATRIX.md` |
| `BPANE-00132_PROJECT_GOVERNANCE_WORKFLOW_PLAN.md` | embedded | `PROJECT_GOVERNANCE_REQUIREMENTS.md`, `DOMAIN_REQUIREMENTS.md`, `VALIDATION_MATRIX.md` |
| `BPANE-00135_SERVICE_PRINCIPAL_REGISTRY_PLAN.md` | embedded | `IDENTITY_ACCESS_REQUIREMENTS.md`, `ADMIN_NEW_API_COVERAGE.md`, `VALIDATION_MATRIX.md` |
| `BPANE-00137_IDENTITY_PROVISIONING_PROJECT_MAPPING_PLAN.md` | embedded | `IDENTITY_ACCESS_REQUIREMENTS.md`, `PROJECT_GOVERNANCE_REQUIREMENTS.md`, `ADMIN_NEW_API_COVERAGE.md` |
| `BPANE-00138_IDENTITY_ADMIN_POLISH_PLAN.md` | embedded | `IDENTITY_ACCESS_REQUIREMENTS.md`, `ADMIN_INTERACTION_REQUIREMENTS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md` |
| `BPANE-00142_ADMIN_APP_REDESIGN_PLAN.md` | embedded | `ADMIN_NEW_REQUIREMENTS.md`, `ADMIN_NEW_STATUS.md`, `ADMIN_NEW_API_COVERAGE.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md`, `ADMIN_NEW_IMPLEMENTATION_GUARDRAILS.md`, `VALIDATION_MATRIX.md` |
| `BPANE-00142_BROWSER_CONTEXTS_ADMIN_PLAN.md` | embedded | `RESOURCE_LIFECYCLE_REQUIREMENTS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md`, `VALIDATION_MATRIX.md` |
| `BPANE-00142_DASHBOARD_PLAN.md` | embedded | `ADMIN_NEW_REQUIREMENTS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md`, `VALIDATION_MATRIX.md` |
| `BPANE-00142_EGRESS_PROFILES_ADMIN_PLAN.md` | embedded | `RESOURCE_LIFECYCLE_REQUIREMENTS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md`, `VALIDATION_MATRIX.md` |
| `BPANE-00142_FILE_WORKSPACES_ADMIN_PLAN.md` | embedded | `DOMAIN_REQUIREMENTS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md`, `VALIDATION_MATRIX.md` |
| `BPANE-00142_MCP_BRIDGE_CONTROL_AUTH_PLAN.md` | embedded | `RUNTIME_OPERATOR_REQUIREMENTS.md`, `DOMAIN_REQUIREMENTS.md`, `SECURITY_RUNTIME_ROADMAP.md`, `VALIDATION_MATRIX.md` |
| `BPANE-00142_PROJECTS_EXISTING_EDIT_PLAN.md` | embedded | `PROJECT_GOVERNANCE_REQUIREMENTS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md`, `VALIDATION_MATRIX.md` |
| `BPANE-00142_RECORDING_LIFECYCLE_PLAN.md` | embedded | `DOMAIN_REQUIREMENTS.md`, `SECURITY_RUNTIME_ROADMAP.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md`, `VALIDATION_MATRIX.md` |
| `BPANE-00142_REVIEW_CLEANUP_PRIORITIZATION_PLAN.md` | embedded | `SECURITY_RUNTIME_ROADMAP.md`, `NEXT_WORKING_ROADMAP.md`, `VALIDATION_MATRIX.md` |
| `BPANE-00142_SESSIONS_ADMIN_PLAN.md` | embedded | `ADMIN_NEW_REQUIREMENTS.md`, `RUNTIME_OPERATOR_REQUIREMENTS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md` |
| `BPANE-00142_SESSION_CAPABILITIES_PLAN.md` | embedded | `ADMIN_INTERACTION_REQUIREMENTS.md`, `PROJECT_GOVERNANCE_REQUIREMENTS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md` |
| `BPANE-00142_SESSION_PREVIEW_METRICS_PLAN.md` | embedded | `ADMIN_INTERACTION_REQUIREMENTS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md`, `VALIDATION_MATRIX.md` |
| `BPANE-00142_SESSION_RECORDING_POLICY_PLAN.md` | embedded | `DOMAIN_REQUIREMENTS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md`, `SECURITY_RUNTIME_ROADMAP.md` |
| `BPANE-00142_UNIFIED_ADMIN_MCP_DELEGATION_PLAN.md` | embedded | `RUNTIME_OPERATOR_REQUIREMENTS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md`, `DOMAIN_REQUIREMENTS.md` |
| `BPANE-00142_WORKFLOW_CODE_PREVIEW_PLAN.md` | embedded | `DOMAIN_REQUIREMENTS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md`, `ADMIN_NEW_API_COVERAGE.md` |
| `BPANE-00142_WORKFLOW_DEFINITIONS_ADMIN_PLAN.md` | embedded | `DOMAIN_REQUIREMENTS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md`, `ADMIN_NEW_API_COVERAGE.md` |
| `BPANE-00142_WORKFLOW_EXECUTION_UI_PLAN.md` | embedded | `DOMAIN_REQUIREMENTS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md`, `VALIDATION_MATRIX.md` |
| `BPANE-00142_WORKFLOW_RUNS_PLAN.md` | embedded | `ADMIN_NEW_REQUIREMENTS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md`, `VALIDATION_MATRIX.md` |
| `BPANE-00142_WORKFLOW_SOURCE_BROWSER_PLAN.md` | embedded | `DOMAIN_REQUIREMENTS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md`, `ADMIN_NEW_API_COVERAGE.md` |
| `BPANE-00142_WORKFLOW_SOURCE_HARDENING_PLAN.md` | embedded | `SECURITY_RUNTIME_ROADMAP.md`, `DOMAIN_REQUIREMENTS.md`, `VALIDATION_MATRIX.md` |
| `BPANE-0015_BROWSER_CONTEXTS_PLAN.md` | embedded | `RESOURCE_LIFECYCLE_REQUIREMENTS.md`, `DOMAIN_REQUIREMENTS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md` |
| `BPANE-0016_OPERATOR_CLI_PLAN.md` | embedded | `RUNTIME_OPERATOR_REQUIREMENTS.md`, `VALIDATION_MATRIX.md` |
| `BPANE-0019_NETWORK_IDENTITY_EGRESS_PLAN.md` | embedded | `RESOURCE_LIFECYCLE_REQUIREMENTS.md`, `DOMAIN_REQUIREMENTS.md`, `ADMIN_NEW_API_COVERAGE.md` |
| `BPANE-0024_SESSION_TEMPLATES_CATALOG_PLAN.md` | embedded | `RESOURCE_LIFECYCLE_REQUIREMENTS.md`, `DOMAIN_REQUIREMENTS.md`, `ADMIN_NEW_API_COVERAGE.md` |
| `BPANE-0027_PROJECTS_QUOTAS_ADMISSION_PLAN.md` | embedded | `PROJECT_GOVERNANCE_REQUIREMENTS.md`, `DOMAIN_REQUIREMENTS.md`, `VALIDATION_MATRIX.md` |
| `BPANE-0052_EXTERNAL_IDENTITY_ACCESS_REVIEW_PLAN.md` | embedded | `IDENTITY_ACCESS_REQUIREMENTS.md`, `ADMIN_NEW_API_COVERAGE.md`, `VALIDATION_MATRIX.md` |
| `BPANE-0093_ADMIN_FEEDBACK_GAPS_PLAN.md` | embedded | `ADMIN_INTERACTION_REQUIREMENTS.md`, `VALIDATION_MATRIX.md` |
| `BPANE-0093_SESSION_CREATION_CONFIGURATOR_PLAN.md` | embedded | `ADMIN_INTERACTION_REQUIREMENTS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md`, `ADMIN_NEW_API_COVERAGE.md` |
| `BPANE_OPEN_ISSUES_INTEGRATION_PLAN.md` | superseded | `NEXT_WORKING_ROADMAP.md`, `SECURITY_RUNTIME_ROADMAP.md`, `PROJECT_GOVERNANCE_REQUIREMENTS.md` |

## Admin Redesign Workspace Documents

| Legacy document | Disposition | Consolidated destination |
| --- | --- | --- |
| `BPANE-00142_ADMIN_APP_REDESIGN/00_INDEX.md` | superseded | `README.md`, `ADMIN_NEW_IMPLEMENTATION_GUARDRAILS.md` |
| `BPANE-00142_ADMIN_APP_REDESIGN/01_CURRENT_ADMIN_PARITY.md` | embedded | `ADMIN_NEW_REQUIREMENTS.md`, `ADMIN_NEW_IMPLEMENTATION_GUARDRAILS.md`, `VALIDATION_MATRIX.md` |
| `BPANE-00142_ADMIN_APP_REDESIGN/02_CONCEPT_MAPPING.md` | embedded | `ADMIN_NEW_REQUIREMENTS.md`, `ADMIN_NEW_IMPLEMENTATION_GUARDRAILS.md` |
| `BPANE-00142_ADMIN_APP_REDESIGN/03_API_COVERAGE.md` | embedded | `ADMIN_NEW_API_COVERAGE.md` |
| `BPANE-00142_ADMIN_APP_REDESIGN/04_IMPLEMENTATION_STEPS.md` | embedded | `ADMIN_NEW_STATUS.md`, `NEXT_WORKING_ROADMAP.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md` |
| `BPANE-00142_ADMIN_APP_REDESIGN/05_TEST_AND_SMOKE_MATRIX.md` | embedded | `VALIDATION_MATRIX.md` |
| `BPANE-00142_ADMIN_APP_REDESIGN/06_SELECTOR_CONTRACT.md` | embedded | `ADMIN_NEW_IMPLEMENTATION_GUARDRAILS.md`, `ADMIN_NEW_REQUIREMENTS.md` |
| `BPANE-00142_ADMIN_APP_REDESIGN/07_PATTERN_LIBRARY.md` | embedded | `ADMIN_NEW_IMPLEMENTATION_GUARDRAILS.md`, `ADMIN_NEW_REQUIREMENTS.md` |
| `BPANE-00142_ADMIN_APP_REDESIGN/08_MANUAL_CHECKPOINTS.md` | embedded | `ADMIN_NEW_MANUAL_CHECKPOINTS.md` |

## Historical Material Not Carried Forward As Requirements

The following categories were intentionally not preserved as active
requirements:

- old branch names except where useful for issue lineage,
- local commit hashes from old implementation notes,
- duplicate smoke command blocks when the same command exists in
  `VALIDATION_MATRIX.md`,
- old issue merge-map prose whose only purpose was deciding past issue
  consolidation,
- prototype route names that conflict with current product routes, such as
  `/admin-new/contexts`,
- old promotion-target route names under `/admin/...` when they conflict with
  the current side-by-side `/admin/` plus `/admin-new/` rule,
- external provider comparison text where it is not needed to define
  BrowserPane behavior,
- mock-only concept features such as share-token/handoff forms without backend
  support.

## Identifier Audit Notes

The consolidation is not a byte-for-byte or token-for-token copy. Exact
backtick-token equality is intentionally not required because:

- smoke commands are preserved in executable command blocks in
  `VALIDATION_MATRIX.md`, not always as individual inline backtick tokens,
- old route placeholders such as `/admin/sessions/:session_id` are historical
  old-admin or prototype notation; current product routes use `/admin-new` and
  SvelteKit `[session_id]` route segments where applicable,
- old shorthand routes such as `/admin-new/contexts` are intentionally
  superseded by `/admin-new/browser-contexts`,
- old workspace split-file names such as `01_CURRENT_ADMIN_PARITY.md` are
  represented by topic files in this folder,
- old source-relative paths such as `src/routes/+page.svelte` are represented
  by full old-admin path anchors in `ADMIN_NEW_IMPLEMENTATION_GUARDRAILS.md`,
- historical branch names and local-only paths are not active requirements.

## Final Coverage Checks

The old folder was removed after these checks passed:

1. Run exact file inventory coverage against `SOURCE_PLAN_INVENTORY.md`.
2. Run section-level coverage against `LEGACY_SECTION_COVERAGE_AUDIT.md`.
3. Run high-signal token coverage against this consolidated workspace.
4. Confirm no consolidated document tells readers to go back to old plans for
   active requirements.
5. Confirm future planning guidance points to this consolidated workspace.
6. Keep future planning documents in the consolidated `docs/` workspace.
