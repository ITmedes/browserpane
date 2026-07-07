# Legacy Section Coverage Audit

Created: 2026-07-07

This audit supplements `LEGACY_DOC_RETENTION_AUDIT.md`. The retention audit
proves every old planning file has a consolidated destination. This file checks
the next level down: the major sections and planning intent that existed inside
those files.

The goal is not to copy old prose. The goal is to prove that the active
requirements, examples, validation expectations, and unresolved product gaps
from the old planning folder are now owned by standalone documents in this
folder.

## Method

The section map was extracted from the old planning folder with heading-level
searches across all Markdown files:

```sh
rg -n "^#{1,6} " docs --glob "*.md"
```

The extracted headings were compared against this consolidated workspace and
then grouped by product area. Repeated legacy headings such as `Goal`, `Scope`,
`Example Use Case`, `Implementation Steps`, `Smoke Sequence`, and `Validation`
are not listed separately for every file unless their contents carried a unique
requirement. Their active content is represented by the destination documents
listed below.

## Coverage Result

Result: no active section family remains orphaned.

The old docs contained three kinds of material:

- active requirements and acceptance criteria, now embedded by topic,
- current status and next-step decisions, now embedded in roadmap/status docs,
- historical execution notes, old branch/PR context, vendor comparison prose,
  or obsolete route names, intentionally not carried forward as requirements.

## Strategic Planning Sections

### Open Issues Integration Plan

Legacy source: `BPANE_OPEN_ISSUES_INTEGRATION_PLAN.md`

Covered section families:

- `Consolidation Result`
- `First-Pass Merge Map`
- `Second-Pass Merge Map`
- `Third-Pass Merge Map`
- `Fourth-Pass Merge Map`
- `Current Implementation Evidence`
- `Priority Decision`
- `Active Issue Matrix`
- `Deliberately Not Merged`
- `Completed Implementation Packages`
- `Implementation Plan`
- `Step 1: Session Catalog And Configuration`
- `Step 2: Product API Consolidation`
- `Step 3: Deployment And Enterprise Readiness`
- `Step 4: Top-Level Tracking`
- `Working Rules`

Consolidated ownership:

- `SOURCE_PLAN_INVENTORY.md` retains the completed package lineage, issue
  consolidation result, and historical status.
- `NEXT_WORKING_ROADMAP.md` retains the current working priority and the next
  shippable slices.
- `SECURITY_RUNTIME_ROADMAP.md` retains deployment, security, runtime, and
  production-readiness cleanup.
- `PROJECT_GOVERNANCE_REQUIREMENTS.md` retains project/session/workflow
  governance requirements that were previously mixed into the active issue
  matrix.
- `ADMIN_NEW_STATUS.md` retains the merged `/admin-new` baseline and promotion
  status.

Superseded material:

- old merge-map prose is retained as lineage, not as a live process rule,
- old issue numbering and duplicate consolidation decisions are not active
  requirements,
- old top-level tracking language is replaced by the current roadmap files.

### Review Cleanup Prioritization Plan

Legacy source: `BPANE-00142_REVIEW_CLEANUP_PRIORITIZATION_PLAN.md`

Covered section families:

- `Purpose`
- `Quadruple-Check Result`
- `Example Use Case`
- `Smoke Sequence For Each Slice`
- `Review Reconciliation`
- `Priority Change Path`
- `Slice 0: Convert The Review Into Workable Issues`
- `Slice 1: Workflow Git Source And Preview Safety`
- `Slice 1B: Docker Runtime Launch Boundary`
- `Slice 2: MCP Bridge Inbound Auth And Exposure`
- `Slice 3: Session Token Domain Separation And Credential Redaction`
- `Slice 3B: Recording Artifact Finalization Boundary`
- `Slice 4: Webhook SSRF Controls`
- `Slice 5: Browser Context Import Safety`
- `Slice 6: Gateway Lifecycle, Health, And Readiness`
- `Slice 7: Admin And Session Catalog Scalability`
- `Slice 8: Admin Cutover Gate`
- `Slice 9: Durable Documentation And Guardrails`
- `Recommended Immediate Next Step`

Consolidated ownership:

- `SECURITY_RUNTIME_ROADMAP.md` owns the security/runtime cleanup slices and
  preserves their priority order.
- `NEXT_WORKING_ROADMAP.md` turns the same findings into shippable near-term
  slices for the current branch sequence.
- `VALIDATION_MATRIX.md` owns the cross-slice validation and smoke expectations.
- `ADMIN_NEW_STATUS.md` and `ADMIN_NEW_REQUIREMENTS.md` own the cutover and
  promotion gate.
- `README.md` and this audit family own the durable documentation cleanup rule.

Superseded material:

- the old "convert review to issues" planning action is historical once the
  findings are represented as roadmap slices,
- dated local review references are retained only as lineage.

## Admin Redesign Foundation Sections

### Redesign Foundation

Legacy source: `ADMIN_APP_REDESIGN_FOUNDATION.md`

Covered section families:

- `Recommendation`
- `Product and Design Orientation`
- `Target App Shape`
- `UI Principles`
- `Current Admin Feature Inventory`
- `Global Shell and Auth`
- `Live Browser Workspace`
- `Sessions`
- `Session Lifecycle`
- `Live Session Actions`
- `Display Controls`
- `Session Files and Workspace Bindings`
- `Recording`
- `Workflows`
- `MCP Delegation`
- `Browser Contexts`
- `Egress Profiles`
- `Identity and Access`
- `Browser Policy`
- `Metrics`
- `Logs`
- `Inspect and Catalog Routes`
- `Redesign Acceptance Criteria`

Consolidated ownership:

- `ADMIN_NEW_REQUIREMENTS.md` owns the product position, information
  architecture, route requirements, parity families, and promotion gate.
- `ADMIN_NEW_IMPLEMENTATION_GUARDRAILS.md` owns side-by-side topology,
  old-admin parity anchors, layout/concept corrections, selector discipline,
  and reusable pattern rules.
- `DOMAIN_REQUIREMENTS.md` owns the product-domain behavior that must remain
  visible through the new admin.
- `ADMIN_NEW_MANUAL_CHECKPOINTS.md` owns the route-by-route manual checkpoints.
- `ADMIN_INTERACTION_REQUIREMENTS.md` owns feedback, messages, metrics, and
  logs.

Superseded material:

- external vendor references from the foundation discussion are not repeated,
- the fake browser-window framing from the concept is explicitly rejected in the
  guardrails,
- mock-only concept features are not active requirements without backend/API
  support.

### Admin Redesign Implementation Plan

Legacy source: `BPANE-00142_ADMIN_APP_REDESIGN_PLAN.md`

Covered section families:

- `Objective`
- `Quadruple-Check Findings`
- `Concept Summary`
- `Concept-To-Implementation Mapping`
- `Current-Implementation Parity Mapping`
- `Product Corrections Before Implementation`
- `Manual Test Gate Rule`
- `Step 0: Baseline and Branch Hygiene`
- `Step 1: Create the New Admin App Beside /admin/`
- `Step 1A: API Client and OpenAPI Coverage Baseline`
- `OpenAPI Operation Coverage Matrix`
- `Step 2: Dashboard With Real Read-Only Data`
- `Step 3: Sessions Catalog`
- `Step 4: Session Detail Shell and Overview Tab`
- `Step 5: Live Tab and Attach Model`
- `Step 6: Session Files Tab`
- `Step 7: Recordings Tab`
- `Step 8: Network Tab`
- `Step 9: Automation Tab`
- `Step 9A: Browser Policy and Runtime Safety Tab`
- `Step 10: Observability Tab`
- `Step 11: Create Session Flow`
- `Step 12: Resource Catalog Migration`
- `Step 12.1: Projects`
- `Step 12.2: Browser Contexts`
- `Step 12.3: Egress Profiles`
- `Step 12.4: File Workspaces`
- `Step 12.5: Workflows and Workflow Runs`
- `Step 12.6: Session Templates`
- `Step 12.7: Extensions`
- `Step 12.8: Credential Bindings`
- `Step 12.9: Workflow Event Subscriptions`
- `Step 12.10: Operation Counters and Internal Automation Evidence`
- `Step 12.11: Identity and Access`
- `Step 13: Command Palette`
- `Step 14: API Reference and API Coverage Companion`
- `Step 15: Parity Review and Promotion Decision`
- `State Management`
- `Layout`
- `Accessibility`
- `Documentation`
- `Final Regression Sequence`

Consolidated ownership:

- `ADMIN_NEW_STATUS.md` owns what has already landed and what remains before
  promotion.
- `ADMIN_NEW_REQUIREMENTS.md` owns route requirements, pattern requirements,
  selector requirements, and promotion criteria.
- `ADMIN_NEW_API_COVERAGE.md` owns API operation classification, schema
  parity, request parity, compatibility endpoints, errors, and the API
  companion acceptance model.
- `ADMIN_NEW_MANUAL_CHECKPOINTS.md` owns the manual gate sequence for each
  route and the final promotion regression sequence.
- `ADMIN_NEW_IMPLEMENTATION_GUARDRAILS.md` owns package topology, static
  serving, API boundary extraction, SvelteKit path conventions, selectors,
  pattern-library guardrails, and concept corrections.
- `NEXT_WORKING_ROADMAP.md` owns the next admin-new slices.
- `VALIDATION_MATRIX.md` owns automated and manual validation expectations.

Superseded material:

- old step numbers are not the source of truth for current implementation
  order; the current roadmap is,
- old branch hygiene instructions are historical for the branch that created
  `/admin-new`,
- duplicate operation matrices are represented once in
  `ADMIN_NEW_API_COVERAGE.md`.

## Admin Requirements Workspace Sections

Legacy sources:

- `BPANE-00142_ADMIN_APP_REDESIGN/00_INDEX.md`
- `BPANE-00142_ADMIN_APP_REDESIGN/01_CURRENT_ADMIN_PARITY.md`
- `BPANE-00142_ADMIN_APP_REDESIGN/02_CONCEPT_MAPPING.md`
- `BPANE-00142_ADMIN_APP_REDESIGN/03_API_COVERAGE.md`
- `BPANE-00142_ADMIN_APP_REDESIGN/04_IMPLEMENTATION_STEPS.md`
- `BPANE-00142_ADMIN_APP_REDESIGN/05_TEST_AND_SMOKE_MATRIX.md`
- `BPANE-00142_ADMIN_APP_REDESIGN/06_SELECTOR_CONTRACT.md`
- `BPANE-00142_ADMIN_APP_REDESIGN/07_PATTERN_LIBRARY.md`
- `BPANE-00142_ADMIN_APP_REDESIGN/08_MANUAL_CHECKPOINTS.md`

Covered section families:

- workspace `Documents`, `Operating Rules`, and `First Implementation Slice`,
- `Current Surface`, `Route Parity`, `Behavior Families`, and generated parity
  inventory,
- concept keep/add/exclude rules, route naming corrections, and UX direction,
- API audit baseline, operation classifications, API families, security model,
  schema property parity, non-OpenAPI surfaces, and gateway route audit,
- implementation steps from side-by-side app creation through promotion,
- current admin baseline checks, package checks, smoke direction, and error
  cases,
- generated selector manifest, high-risk selectors, and selector policy,
- pattern-library recommendation, initial patterns, rules, acceptance, and
  first pattern slice,
- baseline, shell, projects, session, file, recording, workflow, egress,
  identity, and promotion manual checkpoints.

Consolidated ownership:

- `README.md` owns the replacement workspace overview.
- `ADMIN_NEW_IMPLEMENTATION_GUARDRAILS.md` owns workspace operating rules,
  parity anchors, concept corrections, selectors, and patterns.
- `ADMIN_NEW_REQUIREMENTS.md` owns the information architecture, behavior
  families, route requirements, patterns, selectors, and promotion gate.
- `ADMIN_NEW_API_COVERAGE.md` owns API coverage and compatibility details.
- `ADMIN_NEW_STATUS.md` owns implementation status.
- `ADMIN_NEW_MANUAL_CHECKPOINTS.md` owns route manual validation.
- `VALIDATION_MATRIX.md` owns smoke and test coverage.

## Admin-New Slice Sections

Legacy sources and consolidated ownership:

| Legacy slice | Important section families | Consolidated owner |
| --- | --- | --- |
| `BPANE-00142_DASHBOARD_PLAN.md` | goal, example use case, implementation steps, smoke sequence | `ADMIN_NEW_REQUIREMENTS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md`, `VALIDATION_MATRIX.md` |
| `BPANE-00142_PROJECTS_EXISTING_EDIT_PLAN.md` | scope, project edit use case, fields, smoke sequence | `PROJECT_GOVERNANCE_REQUIREMENTS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md`, `VALIDATION_MATRIX.md` |
| `BPANE-00142_BROWSER_CONTEXTS_ADMIN_PLAN.md` | scope, context lifecycle UI, follow-up, smoke | `RESOURCE_LIFECYCLE_REQUIREMENTS.md`, `ADMIN_NEW_REQUIREMENTS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md` |
| `BPANE-00142_EGRESS_PROFILES_ADMIN_PLAN.md` | profile list/detail/create/edit, smoke | `RESOURCE_LIFECYCLE_REQUIREMENTS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md`, `VALIDATION_MATRIX.md` |
| `BPANE-00142_FILE_WORKSPACES_ADMIN_PLAN.md` | workspace list/detail/create/edit, file actions, smoke | `DOMAIN_REQUIREMENTS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md`, `VALIDATION_MATRIX.md` |
| `BPANE-00142_SESSIONS_ADMIN_PLAN.md` | session catalog, create flow, preview window, start/stop, smoke | `ADMIN_NEW_REQUIREMENTS.md`, `DOMAIN_REQUIREMENTS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md` |
| `BPANE-00142_SESSION_CAPABILITIES_PLAN.md` | camera, microphone, upload capability controls, smoke | `ADMIN_INTERACTION_REQUIREMENTS.md`, `PROJECT_GOVERNANCE_REQUIREMENTS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md` |
| `BPANE-00142_SESSION_PREVIEW_METRICS_PLAN.md` | browser transition metrics capture and smoke | `ADMIN_INTERACTION_REQUIREMENTS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md`, `VALIDATION_MATRIX.md` |
| `BPANE-00142_SESSION_RECORDING_POLICY_PLAN.md` | default-off recording, enable-for-session, catalog, follow-up | `DOMAIN_REQUIREMENTS.md`, `SECURITY_RUNTIME_ROADMAP.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md` |
| `BPANE-00142_RECORDING_LIFECYCLE_PLAN.md` | recording lifecycle stabilization and smoke | `DOMAIN_REQUIREMENTS.md`, `SECURITY_RUNTIME_ROADMAP.md`, `VALIDATION_MATRIX.md` |
| `BPANE-00142_UNIFIED_ADMIN_MCP_DELEGATION_PLAN.md` | session endpoint, delegate/default MCP controls, smoke | `DOMAIN_REQUIREMENTS.md`, `RUNTIME_OPERATOR_REQUIREMENTS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md` |
| `BPANE-00142_MCP_BRIDGE_CONTROL_AUTH_PLAN.md` | bridge control auth, gateway proxy, smoke | `SECURITY_RUNTIME_ROADMAP.md`, `RUNTIME_OPERATOR_REQUIREMENTS.md`, `VALIDATION_MATRIX.md` |
| `BPANE-00142_WORKFLOW_DEFINITIONS_ADMIN_PLAN.md` | workflow list/detail/create/edit and smoke | `DOMAIN_REQUIREMENTS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md`, `ADMIN_NEW_API_COVERAGE.md` |
| `BPANE-00142_WORKFLOW_CODE_PREVIEW_PLAN.md` | syntax-highlighted code preview and smoke | `DOMAIN_REQUIREMENTS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md`, `ADMIN_NEW_API_COVERAGE.md` |
| `BPANE-00142_WORKFLOW_SOURCE_BROWSER_PLAN.md` | source tree/file browser/editor and smoke | `DOMAIN_REQUIREMENTS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md`, `ADMIN_NEW_API_COVERAGE.md` |
| `BPANE-00142_WORKFLOW_EXECUTION_UI_PLAN.md` | start/connect workflow execution, input parameters, external start guidance | `DOMAIN_REQUIREMENTS.md`, `ADMIN_NEW_REQUIREMENTS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md` |
| `BPANE-00142_WORKFLOW_RUNS_PLAN.md` | workflow run list/detail direction and smoke | `ADMIN_NEW_REQUIREMENTS.md`, `NEXT_WORKING_ROADMAP.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md` |
| `BPANE-00142_WORKFLOW_SOURCE_HARDENING_PLAN.md` | git source safety, trusted roots, preview limits, smoke | `SECURITY_RUNTIME_ROADMAP.md`, `DOMAIN_REQUIREMENTS.md`, `VALIDATION_MATRIX.md` |

## Platform And Resource Plan Sections

### Local Workflow, MCP, Certificate, And Setup

Legacy source: `BPANE-00119_LOCAL_WORKFLOW_MCP_HARDENING_PLAN.md`

Covered section families:

- `Goal`, `Example Use Case`, `Scope`, `Current Code Map`,
  `Implementation Steps`, `Validation Plan`, `Post-Implementation Smoke Test
  Sequence`, `Risks And Decisions`, and `Done Criteria`.

Consolidated ownership:

- `RUNTIME_OPERATOR_REQUIREMENTS.md` owns local workflow source trust, MCP
  startup, certificate recovery, and local troubleshooting.
- `SECURITY_RUNTIME_ROADMAP.md` owns the completed workflow-source and MCP
  hardening status.
- `VALIDATION_MATRIX.md` owns the workflow/MCP smoke checks.

### Session Runtime Lifecycle

Legacy sources:

- `BPANE-00120_SESSION_RUNTIME_LIFECYCLE_PLAN.md`
- `BPANE-00120_SESSION_RUNTIME_LIFECYCLE_TEST_PLAN.md`

Covered section families:

- lifecycle goal, current notes, MVP scope, non-goals, implementation steps,
  validation, end-user smoke, automated matrix, manual UI sequence, manual API
  sequence, regression checks, and pass criteria.

Consolidated ownership:

- `RUNTIME_OPERATOR_REQUIREMENTS.md` owns release/reconnect/stopped semantics,
  disconnect requirements, and lifecycle visibility.
- `DOMAIN_REQUIREMENTS.md` owns session behavior exposed to admin-new.
- `VALIDATION_MATRIX.md` owns automated and manual lifecycle validation.
- `ADMIN_NEW_MANUAL_CHECKPOINTS.md` owns admin route checks for lifecycle
  controls.

### Browser Context Resources

Legacy source: `BPANE-0015_BROWSER_CONTEXTS_PLAN.md`

Covered section families:

- control-plane context contract,
- runtime materialization,
- admin UX,
- lifecycle,
- usage observability,
- storage observability,
- retention,
- retention cleanup,
- storage quota policy,
- clone workflow,
- export archive,
- import archive,
- acceptance criteria,
- smoke sequence,
- validation commands.

Consolidated ownership:

- `RESOURCE_LIFECYCLE_REQUIREMENTS.md` owns context retention, storage,
  clone/export/import, one-writer semantics, and import safety expectations.
- `DOMAIN_REQUIREMENTS.md` owns the domain-level browser-context requirements.
- `ADMIN_NEW_MANUAL_CHECKPOINTS.md` owns the admin route manual checks.
- `SECURITY_RUNTIME_ROADMAP.md` owns import safety as a remaining hardening
  slice.

### Operator CLI And MCP Delegation

Legacy source: `BPANE-0016_OPERATOR_CLI_PLAN.md`

Covered section families:

- CLI command surface,
- session operations,
- MCP operations,
- operator access and recovery commands,
- MCP doctor and delegation preflight,
- automation exit behavior,
- profiles and local config,
- validation,
- documentation,
- repair safety,
- profile file permission hardening,
- option parser hardening,
- post-implementation smoke,
- acceptance criteria.

Consolidated ownership:

- `RUNTIME_OPERATOR_REQUIREMENTS.md` owns the CLI command, profile, repair, and
  preflight behavior.
- `VALIDATION_MATRIX.md` owns CLI, MCP, and smoke expectations.
- `DOMAIN_REQUIREMENTS.md` owns MCP delegation behavior that must appear in
  admin-new.

### Network Identity, Egress, And Proxy Auth

Legacy sources:

- `BPANE-0019_NETWORK_IDENTITY_EGRESS_PLAN.md`
- `BPANE-00128_EGRESS_PROFILE_HARDENING_PLAN.md`
- `BPANE-00129_PRODUCTION_PROXY_AUTH_VALIDATION_PLAN.md`

Covered section families:

- network identity business case and MVP scope,
- control-plane contract,
- runtime application,
- admin and CLI surfaces,
- egress observer guidance,
- explicit TLS inspection mode,
- local egress presets,
- profile hardening MVP and expanded scope,
- profile acceptance criteria,
- production proxy-auth validation,
- smoke sequences,
- validation commands and final evidence.

Consolidated ownership:

- `RESOURCE_LIFECYCLE_REQUIREMENTS.md` owns egress profile lifecycle,
  diagnostics, proxy-auth, local presets, TLS interception, and safe observer
  metadata.
- `DOMAIN_REQUIREMENTS.md` owns network identity and egress behavior exposed to
  admin-new and sessions.
- `ADMIN_NEW_API_COVERAGE.md` owns API operation and schema coverage for egress
  resources and diagnostics.
- `VALIDATION_MATRIX.md` owns proxy-auth, egress, and diagnostics smokes.

### Session Templates

Legacy source: `BPANE-0024_SESSION_TEMPLATES_CATALOG_PLAN.md`

Covered section families:

- goal,
- example use case,
- scope,
- out of scope,
- implementation steps,
- smoke sequence,
- validation.

Consolidated ownership:

- `RESOURCE_LIFECYCLE_REQUIREMENTS.md` owns template defaults, overrides,
  versioning, catalog filters, and admin catalog requirements.
- `DOMAIN_REQUIREMENTS.md` owns session-template behavior in the product model.
- `ADMIN_NEW_API_COVERAGE.md` owns route and schema coverage.
- `ADMIN_NEW_MANUAL_CHECKPOINTS.md` owns future admin catalog checks.

### Project Governance

Legacy sources:

- `BPANE-0027_PROJECTS_QUOTAS_ADMISSION_PLAN.md`
- `BPANE-00132_PROJECT_GOVERNANCE_WORKFLOW_PLAN.md`

Covered section families:

- project business purpose,
- initial MVP slice,
- API sketch,
- workflow run admission,
- session admission,
- queue controls,
- session-creation budget blocking,
- template and egress policy bindings,
- extension and context policy bindings,
- sanitized egress usage ingestion,
- project-scoped credential governance,
- file workspace policy binding,
- retained storage accounting and enforcement,
- examples and smoke sequences for each completed slice,
- deferred enterprise work.

Consolidated ownership:

- `PROJECT_GOVERNANCE_REQUIREMENTS.md` owns project quotas, queueing, budgets,
  retained storage, policy bindings, file/credential/egress boundaries, and
  manual smoke.
- `DOMAIN_REQUIREMENTS.md` owns the product-domain project model.
- `VALIDATION_MATRIX.md` owns project, workflow, and session validation.
- `ADMIN_NEW_REQUIREMENTS.md` and `ADMIN_NEW_MANUAL_CHECKPOINTS.md` own
  admin-new project route expectations.

### Identity And Access

Legacy sources:

- `BPANE-0052_EXTERNAL_IDENTITY_ACCESS_REVIEW_PLAN.md`
- `BPANE-00135_SERVICE_PRINCIPAL_REGISTRY_PLAN.md`
- `BPANE-00137_IDENTITY_PROVISIONING_PROJECT_MAPPING_PLAN.md`
- `BPANE-00138_IDENTITY_ADMIN_POLISH_PLAN.md`

Covered section families:

- current principal and access review,
- service-principal registry,
- identity provisioning and project mapping,
- token claim rendering,
- safe mapping evidence,
- admin polish,
- CLI behavior,
- validation,
- smoke sequences,
- out-of-scope boundaries.

Consolidated ownership:

- `IDENTITY_ACCESS_REQUIREMENTS.md` owns identity foundation, service
  principals, mappings, admin-new requirements, CLI requirements, manual smoke,
  and validation.
- `PROJECT_GOVERNANCE_REQUIREMENTS.md` owns project-bound identity mapping
  implications.
- `ADMIN_NEW_API_COVERAGE.md` owns identity/access route classification.
- `VALIDATION_MATRIX.md` owns identity and access-review smokes.

### Admin Feedback And Session Creation

Legacy sources:

- `BPANE-0093_ADMIN_FEEDBACK_GAPS_PLAN.md`
- `BPANE-0093_SESSION_CREATION_CONFIGURATOR_PLAN.md`

Covered section families:

- global admin notifications,
- browser connection state,
- external session state diffs,
- admin event stream health,
- workflow run state,
- artifact and delegation snapshots,
- command builder and validation,
- admin UI entry points,
- tests and smoke coverage,
- validation,
- end-user tests,
- open decisions,
- definition of done.

Consolidated ownership:

- `ADMIN_INTERACTION_REQUIREMENTS.md` owns feedback, panel-local messages,
  session creation configurator behavior, metrics, and logs.
- `ADMIN_NEW_MANUAL_CHECKPOINTS.md` owns route-level manual feedback checks.
- `ADMIN_NEW_API_COVERAGE.md` owns API field/request coverage where session
  creation uses templates, capabilities, policies, and resource bindings.
- `VALIDATION_MATRIX.md` owns feedback and configurator validation.

## Validation Section Coverage

The old planning folder repeated validation details in many files. The active
validation content is consolidated this way:

- command-level checks and package builds are in `VALIDATION_MATRIX.md`,
- route-by-route manual checks are in `ADMIN_NEW_MANUAL_CHECKPOINTS.md`,
- project-governance manual smoke is in `PROJECT_GOVERNANCE_REQUIREMENTS.md`,
- identity manual smoke is in `IDENTITY_ACCESS_REQUIREMENTS.md`,
- runtime/operator smoke is in `RUNTIME_OPERATOR_REQUIREMENTS.md`,
- resource lifecycle smoke is in `RESOURCE_LIFECYCLE_REQUIREMENTS.md`.

Duplicate historical command blocks are not repeated everywhere. The current
rule is to validate the affected subsystem plus impacted cross-cutting smokes.

## Material Intentionally Not Retained As Active Requirements

The following old section types were reviewed and intentionally not carried
forward as active requirements:

- old branch names and commit hashes,
- old PR status notes except the current baseline checkpoint,
- old issue merge debates once a canonical product requirement exists,
- competitor/provider name drops and broad market prose,
- duplicate smoke command blocks,
- old route names that conflict with current `/admin-new/` names,
- prototype-only UI content from `concept.html`,
- mock-only features without backend/API support,
- references that told future readers to inspect old source files.

Where those sections contained a valid product decision, that decision is now
represented by the destination documents above.

## Final Assessment

After this pass, the old planning folder was treated as historical source
material, not as required working context. The consolidated workspace now has
coverage at four levels:

1. file-level coverage in `LEGACY_DOC_RETENTION_AUDIT.md`,
2. section-level coverage in this file,
3. active requirements embedded by topic,
4. next-step execution owned by `NEXT_WORKING_ROADMAP.md` and
   `SECURITY_RUNTIME_ROADMAP.md`.

The old folder has been removed in a dedicated documentation cleanup step after
contributor guidance was pointed at the temporary replacement workspace. The
remaining cleanup is to rename `doc_updated/` back to `docs/`.
