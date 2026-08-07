# BrowserPane Consolidated Planning Workspace

Created: 2026-07-07
Revalidated: 2026-07-31

This folder is the standalone planning workspace for the active BrowserPane
plan set. It consolidates the still-valid information from the old plan files
and the current implementation state. The previous scattered planning folder
was consolidated into this current `docs/` layout so active requirements,
roadmaps, and validation expectations can be maintained from one place.

The execution model was tightened under issue `#173` after the code, live issue
tracker, validation state, management deck, and investment website were audited
together. The docs now separate live delivery state, capability maturity,
release gates, risks, durable decisions, requirements, implementation plans,
and historical audit evidence.

The original consolidation was scoped around the BPANE-00142 unified admin app.
The current workspace covers the whole product: Foundation hardening, bounded
Phase 0 value, admin-new Productization, Production readiness, Phase N
enterprise integration, and controlled innovation such as Teach Mode.

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
- Issue `#142` is closed as the historical redesign foundation. Follow-up
  implementation PRs should use focused open issues or create them before
  coding.

## Canonical Operating Documents

Use these first:

- `DELIVERY_ROADMAP.md`: current delivery lanes, states, dependencies, gates,
  and the next three implementation slices.
- `CAPABILITY_MATURITY_MATRIX.md`: evidence-backed Implemented, Prototype,
  Pilot-ready, Production-ready, Planned, and Hypothesis classifications.
- `PRODUCT_PHASES_AND_RELEASE_GATES.md`: Foundation, Phase 0, Phase 1,
  Production Baseline, and Phase N promotion criteria.
- `RISK_REGISTER.md`: active technical, security, operational, legal, delivery,
  and claim risks.
- `PLAN_TEMPLATE.md`: required structure, Definition of Ready, and Definition
  of Done for future `docs/*_PLAN.md` files.
- `OPEN_ISSUES_CONTEXT.md`: issue-to-document ownership map.
- `adr/`: durable architecture/product decisions and pending decisions.

GitHub owns live issue state, assignee, priority, dependency, and milestone.
Do not duplicate that state across requirement documents. A bounded
implementation `*_PLAN.md` becomes executable only when its focused slice
enters Ready or In Progress. Higher-level feature and qualification plans are
specifications and still require a bounded slice plan before coding.

## Files

- `ADMIN_NEW_STATUS.md`: current `/admin-new` implementation state versus the
  consolidated redesign requirements.
- `ADMIN_NEW_REQUIREMENTS.md`: standalone information architecture, parity,
  route, UX, selector, and pattern requirements for the unified admin app.
- `ADMIN_NEW_API_COVERAGE.md`: owner-scoped OpenAPI coverage, operation
  classification, compatibility endpoints, schema parity, and API companion
  requirements.
- `CONTROL_API_COMPATIBILITY_POLICY.md`: frozen v1 baseline selection,
  additive and breaking change rules, deprecation, support windows, and
  emergency corrections.
- `ADMIN_NEW_IMPLEMENTATION_GUARDRAILS.md`: side-by-side package topology,
  old-admin parity anchors, API extraction anchors, selector policy, pattern
  library guardrails, and prototype route corrections.
- `ADMIN_NEW_MANUAL_CHECKPOINTS.md`: route-by-route manual validation gates and
  final regression sequence for the side-by-side admin migration.
- `ADMIN_INTERACTION_REQUIREMENTS.md`: global notifications, panel-local
  feedback, session creation configurator, metrics, and logs.
- `BPANE-00151_MINIMAL_CI_VALIDATION_PLAN.md`: implemented Foundation baseline
  for dependency remediation, reproducible validation, coverage ratchets,
  required GitHub checks, and bounded compose evidence.
- `BPANE-00179_CONTROL_API_CONFORMANCE_PLAN.md`: pinned OpenAPI lint,
  generated operation evidence, executable examples, Axum route conformance,
  semantic compatibility, and CI integration.
- `BPANE-00184_COMPOSE_VALIDATION_PERFORMANCE_PLAN.md`: focused Foundation
  follow-up for reducing hosted compose feedback time while preserving the
  complete local and hosted scenario set; measured cold Docker build
  acceleration remains isolated in GitHub issue `#185`.
- `BPANE-00185_CI_RUST_BUILDER_PLAN.md`: deterministic GHCR Rust builder,
  trusted publication, digest-based compose consumption, cold fallback, and
  timing evidence for the measured #184 image-build bottleneck.
- `BPANE-00171_WORKFLOW_TEACH_MODE_PLAN.md`: Phase N Teach Mode contract,
  semantic demonstration capture, candidate generation, replay, immutable
  publication, controlled repair, and smoke sequence.
- `BPANE-00172_BPM_WORKFLOW_ENDPOINT_INTEGRATION_PLAN.md`: Phase N stable
  project-scoped workflow endpoint contract for BPM/orchestration callers,
  including machine grants, schemas, outcomes, deadlines, callbacks,
  artifacts, endpoint revisions, completion profiles, compatibility exports,
  credential/variable boundaries, handoff ownership, recoverable event
  sequencing/replay, adapters, and conformance smoke.
- `BPANE-00173_DELIVERY_GOVERNANCE_PLAN.md`: governance slice that establishes
  the canonical roadmap, maturity, phase gates, risks, issue ownership, and
  management claim traceability.
- `BPANE-00174_PHASE_0_REFERENCE_WORKFLOW_PLAN.md`: process qualification,
  bounded reference-workflow delivery, operating evidence, and Stop/Operate/
  Phase 1 exit plan.
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
- `REVIEW_FINDINGS_COVERAGE_AUDIT.md`: traceability matrix proving every
  `review/` report family is represented by a current owner, superseded by the
  baseline, or intentionally deferred.
- `IMPLEMENTATION_WORK_ORDER.md`: detailed rationale and topic inventory across
  security cleanup, admin-new parity, resource/domain work, runtime/operator
  work, docs, validation, and refactoring. `DELIVERY_ROADMAP.md` owns current
  execution order.
- `RESOURCE_LIFECYCLE_REQUIREMENTS.md`: browser-context, session-template,
  network identity, egress profile, diagnostics, and proxy-auth lifecycle
  requirements.
- `RUNTIME_OPERATOR_REQUIREMENTS.md`: local setup, workflow source, MCP,
  certificate, runtime release/reconnect, and operator CLI requirements.
- `SOURCE_PLAN_INVENTORY.md`: compact legacy plan-to-topic map with the
  retained requirements embedded by topic.
- `NEXT_WORKING_ROADMAP.md`: retained admin-new transition context.
  `DELIVERY_ROADMAP.md` owns the current cross-product sequence.
- `OPEN_ISSUES_CONTEXT.md`: live open GitHub issue context mapped to the
  consolidated docs, work-order items, and issue hygiene gaps.
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
- workflow-run overview/detail with logs, events, controls, inputs, and files
- route-backed session live, files, recordings, network, automation, policy,
  and observability areas
- identity and access review
- API companion, full operation coverage, and integration docs

The unified admin app is not ready to replace `/admin/` yet. The important
remaining admin-new gaps are:

- session template catalog management
- extensions catalog
- credential bindings catalog
- workflow event subscriptions catalog
- browser-context clone/import/export parity
- project governance evidence and cross-resource policy UX
- operator CLI resource parity
- command palette
- explicit promotion/cutover gate

The cleanup/security plan also identifies hardening work that should happen
before promotion is treated as production-ready:

- browser context import safety
- recording artifact finalization hardening
- production runtime launch boundaries
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

The seventh pass added explicit traceability for every raw `review/` report.
It separates active next-slice work from explicit backlog and intentional
non-blocking deferrals, so the review folder no longer needs to be read to know
whether a finding is represented.

The eighth pass added a consolidated implementation work order. It ranks
security cleanup, admin-new parity, domain-resource work, runtime/operator
work, documentation, validation, and refactoring tasks by production risk,
dependency order, shippable PR scope, and validation confidence.

The ninth pass mapped the consolidated docs against the live open GitHub issue
list. It records which docs own each open issue, which work-order item should
drive it, and which focused issue owns each shippable slice. Work-order items
1 through 26 now have dedicated issues `#145` through `#170`; session-template
catalog work remains in focused issue `#124`.

The tenth pass revalidated the workspace against the current route tree,
package scripts, contributor standards, and all live GitHub issue bodies. It
corrected stale module paths, validation-command drift, event-stream ownership,
and the historical relationship between PR `#143` and closed issue `#142`.

The eleventh pass added focused Phase N issue `#171` and a dedicated Teach Mode
plan. It separates semantic demonstration-to-workflow authoring and controlled
repair from the existing workflow publishing, observability, artifact, and
Human Handoff issue scopes.

The twelfth pass added focused Phase N issue `#172` after a code/OpenAPI and
industry-contract audit. It separates stable BPM-facing Workflow Endpoints,
machine authorization, typed run semantics, traceable callbacks, and connector
conformance from owner-facing workflow publishing and from Teach Mode. A
second endpoint audit added immutable revision/environment promotion,
poll/webhook/callback-token completion profiles, caller overload/readiness,
attempt/side-effect evidence, credential/variable boundaries, Human Handoff
ownership, public event sequencing/replay, and generated connector
compatibility exports.

The thirteenth pass established issue `#173` and the executable delivery
governance model. It added parallel Foundation, Pilot Value, Operator Product,
Production, Enterprise, and Innovation lanes; capability maturity and release
gates; a risk register; durable ADRs; focused issues `#174` through `#180`; and
a bounded Phase 0 reference-workflow plan.

## Maintenance Rule

When maintaining this planning workspace, verify that:

1. `DELIVERY_ROADMAP.md` identifies exactly one first Ready implementation
   slice,
2. every active requirement is represented in this folder,
3. obsolete historical issue merge maps are either intentionally dropped or
   summarized in `SOURCE_PLAN_INVENTORY.md`,
4. section-level coverage remains represented in
   `LEGACY_SECTION_COVERAGE_AUDIT.md`,
5. contributor guidance points future planning documents at `docs/`,
6. capability maturity, risks, and gate evidence match code and tests,
7. external management claims are current evidence, Pilot target, roadmap, or
   hypothesis and link to their evidence/owner,
8. the promotion gate in `ADMIN_NEW_STATUS.md` still reflects the live code.
