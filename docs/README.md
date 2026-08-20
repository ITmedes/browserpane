# BrowserPane Consolidated Planning Workspace

Created: 2026-07-07
Revalidated: 2026-08-20

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

- `main` is synchronized through PR `#236` at `2adbfdee`.
- `/admin-new/` is the standard route-backed operator console and the default
  web-root target after promotion through PR `#211`.
- `/admin/` remains directly addressable only as a compatibility fallback
  pending a separate removal decision.
- The evidence-linked threat model and production-hardening baseline merged
  through PR `#224`. The bounded single-node Compose deployment profile merged
  through PR `#226` under broader issue `#66`. The gateway-to-runtime-broker
  tracing checkpoint merged through PR `#228`, workflow/recording metrics
  merged through PR `#230`, and the Prometheus SLI/alert/runbook baseline merged
  through PR `#232`; the Grafana operations dashboard merged through PR `#234`.
  Compose runner reliability then merged through PR `#236`, with all five
  post-merge Compose lanes green on `main`.
- Issue `#142` is closed as the historical redesign foundation. Follow-up
  implementation PRs use focused issues such as `#124` rather than reopening
  that lineage.

## Canonical Operating Documents

Use these first:

- `CURRENT_CONTEXT.md`: concise fresh-session handoff for current product
  decisions, Phase 0 and later-gate boundaries, issue order, implementation
  gaps, durable working agreements, and working-tree guardrails.
- `DELIVERY_ROADMAP.md`: current delivery lanes, states, dependencies, gates,
  and the next three implementation slices.
- `CAPABILITY_MATURITY_MATRIX.md`: evidence-backed Implemented, Prototype,
  Pilot-ready, Production-ready, Planned, and Hypothesis classifications.
- `PRODUCT_PHASES_AND_RELEASE_GATES.md`: Foundation, Phase 0, Phase 1,
  Production Baseline, and Phase N promotion criteria.
- `RISK_REGISTER.md`: active technical, security, operational, legal, delivery,
  and claim risks.
- `THREAT_MODEL.md`: current assets, actors, trust boundaries, implemented
  controls, required deployment controls, and residual risks.
- `PRODUCTION_SECURITY_BASELINE.md`: responsibility model and deployment gate
  for future supported production profiles.
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
- `BPANE-00223_THREAT_MODEL_BASELINE_PLAN.md`: focused production checkpoint
  that composes the current threat model, hardening checklist, executable
  security contract, and negative-path evidence under issue #223.
- `BPANE-00225_SINGLE_NODE_COMPOSE_BASELINE_PLAN.md`: implemented Production slice
  for an independent broker-only single-node Compose profile, deployment
  secret files, structured preflight, live fixture, and bounded operator
  runbook under issue #225/#66.
- `BPANE-00227_OTEL_RUNTIME_TRACING_PLAN.md`: merged Production checkpoint for
  W3C/OpenTelemetry propagation across gateway and runtime broker under #178.
- `BPANE-00229_WORKFLOW_RECORDING_METRICS_PLAN.md`: merged bounded Production
  checkpoint for shared workflow/recording operations counters and OpenMetrics
  export under #178.
- `BPANE-00231_PROMETHEUS_SLI_ALERT_BASELINE_PLAN.md`: merged bounded Production
  checkpoint for Prometheus recording rules, starter alerts, deterministic
  behavior tests, and operator runbooks under #178.
- `BPANE-00233_GRAFANA_OPERATIONS_DASHBOARD_PLAN.md`: merged bounded Production
  checkpoint for the provisioned aggregate Grafana operations dashboard under
  #178.
- `operations/PROMETHEUS_ALERT_RUNBOOK.md`: alert-specific aggregate triage,
  mitigation, recovery, and escalation for the #231 starter rules.
- `SINGLE_NODE_DEPLOYMENT.md`: operator runbook and exact support boundary for
  the independent single-host package.
- `BPANE-00171_WORKFLOW_TEACH_MODE_PLAN.md`: Phase N Teach Mode contract,
  semantic demonstration capture, candidate generation, replay, immutable
  publication, controlled repair, and smoke sequence.
- `BPANE-00047_WORKFLOW_PACKAGE_CONTRACT_PLAN.md`: next Ready slice for the
  supported immutable Git-backed Playwright package and publication contract.
- `BPANE-00172_PHASE_0_WORKFLOW_ENDPOINT_PLAN.md`: bounded project-scoped,
  service-principal-authorized polling endpoint for one Phase 0 BPM activity.
- `BPANE-00172_BPM_WORKFLOW_ENDPOINT_INTEGRATION_PLAN.md`: historical combined
  endpoint specification retained as source context; not executable.
- `BPANE-00240_WORKFLOW_ENDPOINT_PRODUCTIZATION_PLAN.md`: deferred endpoint
  revisions, callbacks, replay, tracing, throttling, and connector expansion.
- `BPANE-00173_DELIVERY_GOVERNANCE_PLAN.md`: governance slice that establishes
  the canonical roadmap, maturity, phase gates, risks, issue ownership, and
  management claim traceability.
- `BPANE-00174_PHASE_0_REFERENCE_WORKFLOW_PLAN.md`: process qualification,
  bounded BPM browser-activity delivery, operating evidence, and Stop/Operate/
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
- `THREAT_MODEL.md`: evidence-linked security model for local,
  broker-validation, and bounded single-node profiles.
- `PRODUCTION_SECURITY_BASELINE.md`: required application, deployment,
  infrastructure, and operator controls for accepting a target profile.
- `SINGLE_NODE_DEPLOYMENT.md`: bounded single-host deployment configuration,
  operation, recovery, qualification, and unsupported scope.
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

The unified admin app is the standard operator route. Important remaining
operator-product gaps are:

- session template catalog management
- operation-counter catalog and deeper workflow publishing controls
- command palette
- a separately governed compatibility-admin removal decision

Admin promotion does not imply production readiness. The current production
lane retains these explicit hardening and qualification gaps:

- supported deployment packaging and target-specific runtime sandbox evidence
- complete authorization, identity lifecycle, secret rotation, and audit policy
- backup/restore, HA, release governance, and tested capacity envelopes
- MCP ingress hardening plus broader telemetry, SLO, and incident runbooks

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

The twelfth pass originally added a broad Phase N issue `#172` after a
code/OpenAPI and industry-contract audit. The 2026-08-20 boundary review later
narrowed `#172` to the Phase 0 polling endpoint and moved revision promotion,
callbacks, replay, tracing, overload/readiness, and connector compatibility to
`#240`. The old combined plan remains historical context only.

The thirteenth pass established issue `#173` and the executable delivery
governance model. It added parallel Foundation, Pilot Value, Operator Product,
Production, Enterprise, and Innovation lanes; capability maturity and release
gates; a risk register; durable ADRs; focused issues `#174` through `#180`; and
a bounded Phase 0 reference-workflow plan.

The fourteenth pass synchronized the long-running implementation session after
PR #236. It added the concise current-context handoff, restored the missing
plan template, narrowed Phase 0 to one polling-based BPM browser activity,
split the old combined #172 specification into bounded #172 and deferred #240
plans, made #47 the next Ready slice, and removed Human Handoff and Teach Mode
from Phase 0 dependencies.

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
9. `CURRENT_CONTEXT.md` reflects the current Phase 0 boundary and first Ready
   issue.
