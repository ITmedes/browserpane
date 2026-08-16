# Open GitHub Issues Context

Created: 2026-07-10
Revalidated: 2026-08-16 after #231 merged through PR #232 and focused Grafana
operations-dashboard issue #233 entered implementation

This document maps the current `docs/` workspace to the live open GitHub
issues for `ITmedes/browserpane`. It is the bridge between the consolidated
local planning docs and the public issue tracker.

Source check:

- fetched through the GitHub API on 2026-08-16,
- open issue range: `#6` through `#233`,
- focused docs-derived implementation issues created on 2026-07-10: `#145`
  through `#170`,
- focused Phase N Teach Mode issue created on 2026-07-31: `#171`,
- focused Phase N BPM Workflow Endpoint issue created on 2026-07-31: `#172`,
- delivery-governance issue created on 2026-07-31: `#173`,
- focused Phase 0, protocol, identity, observability, API-contract, and
  open-source-governance issues created on 2026-07-31: `#174` through `#180`,
- focused compose-validation performance issue created on 2026-08-03: `#184`,
- focused deterministic Docker build-cache follow-up created on 2026-08-03:
  `#185`,
- focused policy-validating runtime launch-broker follow-up created on
  2026-08-13: `#214`,
- focused threat-model baseline `#223` merged and closed through PR `#224` on
  2026-08-14,
- focused single-node Compose deployment slice created on 2026-08-14: `#225`,
- focused OpenTelemetry runtime-tracing slice `#227` merged and closed through
  PR `#228` on 2026-08-14,
- focused workflow/recording metrics slice `#229` merged and closed through PR
  `#230` on 2026-08-16,
- focused Prometheus SLI/alert/runbook slice `#231` merged and closed through PR
  `#232` on 2026-08-16,
- focused Grafana operations-dashboard slice created on 2026-08-16: `#233`,
- all executable open issues carry a priority, lane, state, and target-gate
  milestone; umbrella tracker `#6` intentionally carries only priority/state,
- `#151` and `#173` are implemented and closed; `#184` is the implemented
  bounded sharding slice and `#185` owns its measured cold-build follow-up,
- cross-reference pass on 2026-07-31 verified that every open issue has a
  docs source or docs cross-reference section and links back to this file,
- closed admin-redesign lineage issue: `#142`, closed as completed on
  2026-07-07 and updated on 2026-07-10 to point at the consolidated docs.

## Issue Roles

| Role | Issues | How to use |
| --- | --- | --- |
| Umbrella tracker | `#6` | Keep open for high-level roadmap context only. Do not use as the implementation issue for feature PRs. |
| Focused current admin resource issue | `#124` | Use for the session-template catalog route when that slice is selected. |
| Focused docs-derived work-order issues | `#145` through `#170` | Use these as canonical implementation issues for work-order items 1 through 26. |
| Focused Phase N productization issues | `#172`, `#171` | Use `#172` for stable project-scoped BPM Workflow Endpoints and their machine-facing run contract. Use `#171` for Workflow Studio Teach Mode, semantic demonstrations, candidate generation, replay gates, immutable publication, and controlled repair. |
| Delivery governance | `#173` | Owns the canonical roadmap, maturity, gates, risks, plan template, and issue/claim reconciliation. It does not own runtime features. |
| Focused cross-product gaps | `#174` through `#180` | Use these for Phase 0 delivery, protocol conformance, authorization, identity lifecycle, platform telemetry, API compatibility, and open-source governance. |
| Focused validation performance | `#184` | Preserve the #151 validation baseline while reducing hosted compose feedback time through isolated execution lanes. |
| Focused Docker build acceleration | `#185` | Preserve #184 lane coverage while adding deterministic, supply-chain-safe Docker build reuse for trusted hosted runs. |
| Focused runtime authorization boundary | `#214` | Complete the typed, policy-validating broker and gateway-isolated Docker-host topology beyond the #167 direct compatibility boundary. |
| Completed threat-model baseline | closed `#223` | Durable evidence baseline merged through PR #224; #72 keeps residual security ownership. |
| Completed single-node deployment | closed `#225` | Independent single-node package merged through PR #226; #66 keeps broader deployment ownership. |
| Completed runtime tracing | closed `#227` | Gateway-to-broker browser lifecycle tracing merged through PR #228; #178 keeps broader telemetry ownership. |
| Completed subsystem metrics | closed `#229` | Workflow/recording operations counters merged through PR #230; #178 keeps broader telemetry ownership. |
| Completed SLI/alert baseline | closed `#231` | Prometheus recording rules, starter alerts, deterministic rule tests, and operator runbooks merged through PR #232. |
| Focused operations dashboard | `#233` | Current bounded child of #178 for a provisioned aggregate Grafana dashboard and its structural/live qualification. |
| Product/platform backlog | `#20`, `#21`, `#28`, `#30`, `#31`, `#47`, `#66`, `#69`, `#70`, `#71`, `#72`, `#73`, `#74`, `#75`, `#76`, `#79`, `#80` | Keep as roadmap and enterprise/product context. Prefer the matching focused issue from `#145`-`#170` when an implementation slice is covered there. |
| Closed admin redesign lineage | `#142` | Historical design record for admin-new. It is not open; route remaining admin implementation slices through the focused admin issues in `#153`-`#163`. |

## Open Issue Matrix

| Issue | Current docs context | Work-order owner | Priority interpretation |
| --- | --- | --- | --- |
| `#6` Epic: Integration-ready control plane and focused issue tracker | `README.md`, `SOURCE_PLAN_INVENTORY.md`, `IMPLEMENTATION_WORK_ORDER.md` | Entire work order | Umbrella only. Use focused issues for implementation. |
| `#20` Per-session observability, logs, and tab/page inspection APIs | `DOMAIN_REQUIREMENTS.md`, `ADMIN_INTERACTION_REQUIREMENTS.md`, `ADMIN_NEW_REQUIREMENTS.md`, `VALIDATION_MATRIX.md` | Items 11, 12, 20, 22 | Current-state admin-new observability is implemented; durable inspection APIs remain later work. |
| `#21` Artifact, browser-output, and recording export APIs | `DOMAIN_REQUIREMENTS.md`, `RESOURCE_LIFECYCLE_REQUIREMENTS.md`, `SECURITY_RUNTIME_ROADMAP.md` | Items 5, 11, 21, 27 | Session files/recordings are implemented; the generalized artifact model remains later work. |
| `#28` Resource event subscriptions and security-event export | `SECURITY_RUNTIME_ROADMAP.md`, `DOMAIN_REQUIREMENTS.md`, `ADMIN_INTERACTION_REQUIREMENTS.md` | Items 3, 12, 21, 27 | Webhook SSRF is near-term; generalized events/security export are later. |
| `#30` Debug and support bundles | `REVIEW_FINDINGS_COVERAGE_AUDIT.md`, `IMPLEMENTATION_WORK_ORDER.md`, `SECURITY_RUNTIME_ROADMAP.md` | Items 22, 27 | Deferred production/support packaging. |
| `#31` Mobile and device-mode sessions | `IMPLEMENTATION_WORK_ORDER.md`, `DOMAIN_REQUIREMENTS.md` | Item 27 | Deferred product capability. Needs dedicated requirements if promoted. |
| `#47` Workflow publishing and supported execution interfaces | `DOMAIN_REQUIREMENTS.md`, `ADMIN_NEW_API_COVERAGE.md`, `RUNTIME_OPERATOR_REQUIREMENTS.md` | Items 10, 15, 18, 21 | Workflow/run routes exist; publishing and broader productization remain. |
| `#66` Compose, Kubernetes, and AWS Fargate deployments | `RUNTIME_OPERATOR_REQUIREMENTS.md`, `SECURITY_RUNTIME_ROADMAP.md`, `IMPLEMENTATION_WORK_ORDER.md` | Items 6, 18, 22, 23, 27 | Parent deployment owner. #225 owns the current single-node Compose slice; Kubernetes/EKS, Fargate, shared-storage expansion, and cross-target architecture remain here. |
| `#69` Session-scoped automation connection APIs | `DOMAIN_REQUIREMENTS.md`, `ADMIN_NEW_API_COVERAGE.md`, `RUNTIME_OPERATOR_REQUIREMENTS.md` | Items 12, 18, 27 | Session automation route visibility is admin parity; productized external contract is later. |
| `#70` API key, audit log, and retention policy controls | `IDENTITY_ACCESS_REQUIREMENTS.md`, `PROJECT_GOVERNANCE_REQUIREMENTS.md`, `IMPLEMENTATION_WORK_ORDER.md` | Items 13, 17, 27 | Identity route can surface current access review now; API keys/audit are enterprise backlog. |
| `#71` Signed human handoff, challenge detection, and private fallback | `IMPLEMENTATION_WORK_ORDER.md`, `DOMAIN_REQUIREMENTS.md` | Item 27 | Deferred product capability. Needs dedicated requirements if promoted. |
| `#72` Enterprise security hardening baseline and threat model | `THREAT_MODEL.md`, `PRODUCTION_SECURITY_BASELINE.md`, `SECURITY_RUNTIME_ROADMAP.md`, `REVIEW_FINDINGS_RECONCILIATION.md`, `IMPLEMENTATION_WORK_ORDER.md` | Items 1, 2, 3, 4, 6, 22, 23 | Broad residual security-hardening owner. #223 is merged evidence; supported deployment packaging remains #66/#225. |
| `#73` Backup, restore, and disaster recovery runbooks | `SECURITY_RUNTIME_ROADMAP.md`, `IMPLEMENTATION_WORK_ORDER.md`, `RUNTIME_OPERATOR_REQUIREMENTS.md` | Items 22, 27 | Deferred production runbook work. |
| `#74` High availability and zero-downtime operations | `SECURITY_RUNTIME_ROADMAP.md`, `IMPLEMENTATION_WORK_ORDER.md` | Items 6, 20, 23, 27 | Graceful shutdown/readiness are near-term; full HA is later. |
| `#75` Supply chain security and release governance | `REVIEW_FINDINGS_COVERAGE_AUDIT.md`, `IMPLEMENTATION_WORK_ORDER.md`, `VALIDATION_MATRIX.md` | Items 7, 22, 27 | Current critical/high dependency remediation and the CI/validation ratchet are near-term through `#151`; broader release governance is later. |
| `#76` Data residency, encryption, and BYOK controls | `IMPLEMENTATION_WORK_ORDER.md`, `RESOURCE_LIFECYCLE_REQUIREMENTS.md` | Item 27 | Deferred enterprise storage/security capability. |
| `#79` Central enterprise policy engine | `PROJECT_GOVERNANCE_REQUIREMENTS.md`, `IMPLEMENTATION_WORK_ORDER.md` | Items 17, 27 | Current project-policy UX is nearer term; central policy engine is later. |
| `#80` DLP and content inspection hooks | `PROJECT_GOVERNANCE_REQUIREMENTS.md`, `IMPLEMENTATION_WORK_ORDER.md` | Items 17, 27 | Current file policy visibility is nearer term; DLP provider hooks are later. |
| `#124` Admin session template catalog management | `DOMAIN_REQUIREMENTS.md`, `RESOURCE_LIFECYCLE_REQUIREMENTS.md`, `ADMIN_NEW_REQUIREMENTS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md` | Item 15 | Focused admin-new resource slice. Use as canonical issue for template catalog work. |
| `#171` Workflow Studio Teach Mode and controlled demonstration-to-workflow publishing | `BPANE-00171_WORKFLOW_TEACH_MODE_PLAN.md`, `DOMAIN_REQUIREMENTS.md`, `ADMIN_NEW_REQUIREMENTS.md`, `VALIDATION_MATRIX.md` | Item 27, focused Phase N slice | Canonical owner for semantic demonstrations, candidate generation, fresh-context replay, immutable publication gate, and controlled repair. Not implemented and not an admin-new promotion blocker. |
| `#172` Project-scoped workflow endpoints for BPM and orchestration integrations | `BPANE-00172_BPM_WORKFLOW_ENDPOINT_INTEGRATION_PLAN.md`, `DOMAIN_REQUIREMENTS.md`, `IDENTITY_ACCESS_REQUIREMENTS.md`, `ADMIN_NEW_REQUIREMENTS.md`, `PROJECT_GOVERNANCE_REQUIREMENTS.md`, `RUNTIME_OPERATOR_REQUIREMENTS.md`, `VALIDATION_MATRIX.md` | Item 27, focused Phase N slice | Canonical owner for stable endpoint deployment/revisions, machine grants, typed async invocation, completion profiles, deadlines, overload/readiness, tracing, callbacks, side-effect evidence, artifact handoff, and connector conformance. Implement before `#171` within Phase N by default. |
| `#173` Executable delivery roadmap, capability maturity, and release gates | `BPANE-00173_DELIVERY_GOVERNANCE_PLAN.md`, `DELIVERY_ROADMAP.md`, `CAPABILITY_MATURITY_MATRIX.md`, `PRODUCT_PHASES_AND_RELEASE_GATES.md`, `RISK_REGISTER.md` | Governance baseline | Canonical owner for delivery structure and cross-reference integrity. No runtime scope. |
| `#174` Phase 0 reference workflow | `BPANE-00174_PHASE_0_REFERENCE_WORKFLOW_PLAN.md`, `DELIVERY_ROADMAP.md`, `PRODUCT_PHASES_AND_RELEASE_GATES.md` | Pilot Value lane | Canonical owner for candidate qualification, bounded workflow delivery, operating evidence, and Stop/Operate/Phase 1 exit. |
| `#175` Remote protocol specification and conformance | `CAPABILITY_MATURITY_MATRIX.md`, `RISK_REGISTER.md`, ADR 0003 | Production lane | Canonical owner for wire spec, version negotiation, golden vectors, fuzzing, and gateway/client compatibility. |
| `#176` Organization/project-role/service-principal grant enforcement | `IDENTITY_ACCESS_REQUIREMENTS.md`, `CAPABILITY_MATURITY_MATRIX.md`, `RISK_REGISTER.md` | Enterprise lane | Canonical owner for enforceable authorization and migration from owner-scoped deployments. |
| `#177` Provisioning/deprovisioning and break-glass lifecycle | `IDENTITY_ACCESS_REQUIREMENTS.md`, `CAPABILITY_MATURITY_MATRIX.md`, `RISK_REGISTER.md` | Enterprise lane | Canonical owner for remaining identity lifecycle scope from closed #52. |
| `#178` Platform telemetry, SLOs, and capacity evidence | `BPANE-00178_PLATFORM_TELEMETRY_PLAN.md`, `PLATFORM_TELEMETRY.md`, `DELIVERY_ROADMAP.md`, `CAPABILITY_MATURITY_MATRIX.md`, `RISK_REGISTER.md` | Production lane | Gateway metrics merged through PR #222, browser lifecycle tracing through PR #228, workflow/recording counters through PR #230, and Prometheus SLI/alert/runbook rules through PR #232; #233 is the current Grafana dashboard child. This parent remains canonical for broader traces/metrics, calibrated SLOs, alert routing, synthetics, and tested envelopes. |
| `#179` Control API conformance and compatibility | `ADMIN_NEW_API_COVERAGE.md`, `CAPABILITY_MATURITY_MATRIX.md`, `RISK_REGISTER.md` | Foundation/Production lane | Canonical owner for OpenAPI lint, route/schema conformance, examples, and breaking-change policy. |
| `#180` Open-source license/contribution/IP governance | `PRODUCT_PHASES_AND_RELEASE_GATES.md`, `RISK_REGISTER.md`, ADR 0004 | Production governance | Canonical owner for resolving license metadata and contribution/security/IP policy. |
| `#184` Reduce representative compose validation wall time | `BPANE-00184_COMPOSE_VALIDATION_PERFORMANCE_PLAN.md`, `VALIDATION_MATRIX.md` | Foundation maintenance | Canonical owner for hosted compose sharding and timing evidence without reducing the #151 scenario baseline. |
| `#185` Persist deterministic Docker build cache | `BPANE-00185_CI_RUST_BUILDER_PLAN.md`, `BPANE-00184_COMPOSE_VALIDATION_PERFORMANCE_PLAN.md`, `VALIDATION_MATRIX.md` | Foundation maintenance | Follow-up owner for a deterministic GHCR Rust builder after #184; it must preserve coverage and cache trust boundaries. |
| `#214` Policy-validating runtime launch broker | `BPANE-00214_RUNTIME_LAUNCH_BROKER_PLAN.md`, `BPANE-00167_DOCKER_RUNTIME_BOUNDARY_PLAN.md`, `DELIVERY_ROADMAP.md`, `CAPABILITY_MATURITY_MATRIX.md`, `RISK_REGISTER.md`, `RUNTIME_OPERATOR_REQUIREMENTS.md`, `VALIDATION_MATRIX.md` | Production lane, completion of item 23 | Canonical owner for authenticated typed launch/lifecycle/storage operations, request-body policy enforcement, and the gateway-isolated production-like Docker-host topology. |
| `#225` Hardened single-node Compose deployment | `BPANE-00225_SINGLE_NODE_COMPOSE_BASELINE_PLAN.md`, `SINGLE_NODE_DEPLOYMENT.md`, `THREAT_MODEL.md`, `PRODUCTION_SECURITY_BASELINE.md`, `RUNTIME_OPERATOR_REQUIREMENTS.md`, `DELIVERY_ROADMAP.md`, `RISK_REGISTER.md`, `VALIDATION_MATRIX.md` | Closed Production child of #66 | Independent broker-only manifest, preflight, live workflow/recording/restart qualification, and bounded runbook merged through PR #226. |
| `#227` OpenTelemetry broker runtime tracing | `BPANE-00227_OTEL_RUNTIME_TRACING_PLAN.md`, `PLATFORM_TELEMETRY.md`, `DELIVERY_ROADMAP.md`, `CAPABILITY_MATURITY_MATRIX.md`, `RISK_REGISTER.md`, `VALIDATION_MATRIX.md` | Closed Production child of #178 | W3C propagation and bounded browser runtime lifecycle spans across gateway and broker merged through PR #228 with private collector parentage, redaction, malformed-context, and outage/recovery smoke evidence. |
| `#229` Workflow and recording OpenMetrics | `BPANE-00229_WORKFLOW_RECORDING_METRICS_PLAN.md`, `PLATFORM_TELEMETRY.md`, `DELIVERY_ROADMAP.md`, `CAPABILITY_MATURITY_MATRIX.md`, `RISK_REGISTER.md`, `VALIDATION_MATRIX.md` | Closed Production child of #178 | Shared label-free workflow/recording operation counters merged through PR #230. |
| `#231` Prometheus SLI and alert baseline | `BPANE-00231_PROMETHEUS_SLI_ALERT_BASELINE_PLAN.md`, `PLATFORM_TELEMETRY.md`, `DELIVERY_ROADMAP.md`, `CAPABILITY_MATURITY_MATRIX.md`, `RISK_REGISTER.md`, `VALIDATION_MATRIX.md` | Closed Production child of #178 | Recording rules, conservative starter alerts, deterministic behavior tests, and operator runbooks merged through PR #232. |
| `#233` Grafana operations dashboard | `BPANE-00233_GRAFANA_OPERATIONS_DASHBOARD_PLAN.md`, `PLATFORM_TELEMETRY.md`, `DELIVERY_ROADMAP.md`, `CAPABILITY_MATURITY_MATRIX.md`, `RISK_REGISTER.md`, `VALIDATION_MATRIX.md` | Production lane, focused child of #178 | Current bounded slice: provisioned aggregate dashboard, deterministic query/security contracts, and private-network live Grafana qualification. |

## Focused Work-Order Issue Matrix

Created on 2026-07-10 from the reverse docs-to-issues audit. These issues
cover concrete topics that were present in `docs/` but did not yet have
dedicated open issue ownership.

| Work item | Issue | Docs source |
| --- | --- | --- |
| 1. Token domain separation and URL credential cleanup | `#145` Add token domain separation and URL credential cleanup | `IMPLEMENTATION_WORK_ORDER.md`, `NEXT_WORKING_ROADMAP.md`, `SECURITY_RUNTIME_ROADMAP.md` |
| 2. Shared admin browser auth and web-security hardening | `#146` Harden shared admin auth and browser security | `IMPLEMENTATION_WORK_ORDER.md`, `NEXT_WORKING_ROADMAP.md`, `SECURITY_RUNTIME_ROADMAP.md` |
| 3. Webhook SSRF controls | `#147` Add webhook SSRF controls for event delivery | `IMPLEMENTATION_WORK_ORDER.md`, `SECURITY_RUNTIME_ROADMAP.md`, `REVIEW_FINDINGS_RECONCILIATION.md` |
| 4. Browser context import safety | `#148` Harden browser context import safety | Implemented on `feature/BPANE-00148`; `BPANE-00148_BROWSER_CONTEXT_IMPORT_SAFETY_PLAN.md`, `IMPLEMENTATION_WORK_ORDER.md`, `SECURITY_RUNTIME_ROADMAP.md`, `RESOURCE_LIFECYCLE_REQUIREMENTS.md` |
| 5. Recording artifact finalization boundary | `#149` Harden recording artifact finalization boundary | Closed through merged PR #212; `BPANE-00149_RECORDING_ARTIFACT_FINALIZATION_PLAN.md`, `IMPLEMENTATION_WORK_ORDER.md`, `NEXT_WORKING_ROADMAP.md`, `SECURITY_RUNTIME_ROADMAP.md` |
| 6. Gateway lifecycle, health, and readiness | `#150` Add gateway lifecycle, health, and readiness endpoints | `IMPLEMENTATION_WORK_ORDER.md`, `SECURITY_RUNTIME_ROADMAP.md`, `RUNTIME_OPERATOR_REQUIREMENTS.md` |
| 7. Minimal CI, dependency safety, and validation ratchet | `#151` Add minimal CI, dependency safety, and validation ratchet | `IMPLEMENTATION_WORK_ORDER.md`, `VALIDATION_MATRIX.md`, `REVIEW_FINDINGS_COVERAGE_AUDIT.md` |
| 8. Postgres session-control store contract tests | `#152` Add Postgres session-control store contract tests | `IMPLEMENTATION_WORK_ORDER.md`, `VALIDATION_MATRIX.md`, `SECURITY_RUNTIME_ROADMAP.md` |
| 9. Admin-new pattern, API client, and feedback consolidation | `#153` Consolidate admin-new patterns, API client, and feedback handling | `IMPLEMENTATION_WORK_ORDER.md`, `ADMIN_NEW_IMPLEMENTATION_GUARDRAILS.md`, `ADMIN_INTERACTION_REQUIREMENTS.md` |
| 10. Route-backed workflow run detail | `#154` Add route-backed admin-new workflow run detail | `IMPLEMENTATION_WORK_ORDER.md`, `NEXT_WORKING_ROADMAP.md`, `ADMIN_NEW_REQUIREMENTS.md` |
| 11. Session subareas phase 1 | `#155` Add admin-new session subareas for live, files, recordings, and network | `IMPLEMENTATION_WORK_ORDER.md`, `NEXT_WORKING_ROADMAP.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md` |
| 12. Session subareas phase 2 | `#156` Add admin-new session automation, policy, and observability subareas | `IMPLEMENTATION_WORK_ORDER.md`, `NEXT_WORKING_ROADMAP.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md` |
| 13. Identity and access review route | `#157` Add admin-new identity and access review route | Merged through PR #199; `IMPLEMENTATION_WORK_ORDER.md`, `NEXT_WORKING_ROADMAP.md`, `IDENTITY_ACCESS_REQUIREMENTS.md` |
| 14. API companion, coverage, and docs routes | `#158` Add admin-new API companion, coverage, and docs routes | Merged through PR #200; `IMPLEMENTATION_WORK_ORDER.md`, `NEXT_WORKING_ROADMAP.md`, `ADMIN_NEW_API_COVERAGE.md` |
| 15. Missing resource catalogs except session templates | `#159` Add admin-new catalogs for extensions, credential bindings, and workflow event subscriptions | Merged through PR #201; `IMPLEMENTATION_WORK_ORDER.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md`, `RESOURCE_LIFECYCLE_REQUIREMENTS.md`; session templates remain `#124` |
| 16. Browser context clone/import/export UI parity | `#160` Add browser context clone, import, and export admin-new parity | Merged through PR #203; `BPANE-00160_ADMIN_NEW_BROWSER_CONTEXT_LIFECYCLE_PLAN.md`, `IMPLEMENTATION_WORK_ORDER.md`, `RESOURCE_LIFECYCLE_REQUIREMENTS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md` |
| 17. Project governance evidence and cross-resource policy UX | `#161` Add project governance evidence and cross-resource policy UX | Merged through PR #204; `BPANE-00161_PROJECT_GOVERNANCE_EVIDENCE_PLAN.md`, `IMPLEMENTATION_WORK_ORDER.md`, `PROJECT_GOVERNANCE_REQUIREMENTS.md`, `ADMIN_NEW_REQUIREMENTS.md` |
| 18. Operator CLI resource parity and local setup docs | `#162` Add operator CLI resource parity and local setup diagnostics docs | Merged through PR #209; `BPANE-00162_OPERATOR_CLI_PARITY_PLAN.md`, `IMPLEMENTATION_WORK_ORDER.md`, `RUNTIME_OPERATOR_REQUIREMENTS.md`, `VALIDATION_MATRIX.md` |
| 19. Admin-new promotion gate | `#163` Define admin-new promotion gate and fallback plan | Closed after the validation contract merged through PR #210 and default root routing through PR #211; `BPANE-00163_ADMIN_NEW_PROMOTION_PLAN.md`, `IMPLEMENTATION_WORK_ORDER.md`, `ADMIN_NEW_STATUS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md` |
| 20. Admin and session catalog scalability | `#164` Improve admin and session catalog scalability | `IMPLEMENTATION_WORK_ORDER.md`, `SECURITY_RUNTIME_ROADMAP.md`, `REVIEW_FINDINGS_COVERAGE_AUDIT.md` |
| 21. Worker and archive runtime hygiene | `#165` Harden worker logs, polling, and archive runtime behavior | Closed through merged PR #213; `BPANE-00165_WORKER_RUNTIME_HARDENING_PLAN.md`, `IMPLEMENTATION_WORK_ORDER.md`, `SECURITY_RUNTIME_ROADMAP.md`, `REVIEW_FINDINGS_RECONCILIATION.md` |
| 22. Documentation accuracy and production references | `#166` Update documentation accuracy and production references | `IMPLEMENTATION_WORK_ORDER.md`, `SECURITY_RUNTIME_ROADMAP.md`, `REVIEW_FINDINGS_COVERAGE_AUDIT.md` |
| 23. Docker runtime launch boundary | `#167` direct compatibility boundary; `#214` typed broker and isolated topology | `BPANE-00167_DOCKER_RUNTIME_BOUNDARY_PLAN.md`, `BPANE-00214_RUNTIME_LAUNCH_BROKER_PLAN.md`, `IMPLEMENTATION_WORK_ORDER.md`, `SECURITY_RUNTIME_ROADMAP.md`, `RUNTIME_OPERATOR_REQUIREMENTS.md` |
| 24. Host and client render hot-path work | `#168` Profile and optimize host and client render hot paths | `IMPLEMENTATION_WORK_ORDER.md`, `SECURITY_RUNTIME_ROADMAP.md`, `REVIEW_FINDINGS_COVERAGE_AUDIT.md` |
| 25. Gateway fan-out and transport optimization | `#169` Profile and optimize gateway fan-out and transport behavior | `IMPLEMENTATION_WORK_ORDER.md`, `SECURITY_RUNTIME_ROADMAP.md`, `REVIEW_FINDINGS_RECONCILIATION.md` |
| 26. Structural refactors | `#170` Split session-control structural refactors by domain | `IMPLEMENTATION_WORK_ORDER.md`, `SECURITY_RUNTIME_ROADMAP.md`, `REVIEW_FINDINGS_RECONCILIATION.md` |
| Phase N. BPM Workflow Integration Endpoints | `#172` Add project-scoped workflow endpoints for BPM and orchestration integrations | `BPANE-00172_BPM_WORKFLOW_ENDPOINT_INTEGRATION_PLAN.md`, `DOMAIN_REQUIREMENTS.md`, `IDENTITY_ACCESS_REQUIREMENTS.md`, `ADMIN_NEW_REQUIREMENTS.md`, `PROJECT_GOVERNANCE_REQUIREMENTS.md`, `RUNTIME_OPERATOR_REQUIREMENTS.md`, `VALIDATION_MATRIX.md` |
| Phase N. Workflow Studio Teach Mode | `#171` Add Workflow Studio Teach Mode and controlled demonstration-to-workflow publishing | `BPANE-00171_WORKFLOW_TEACH_MODE_PLAN.md`, `DOMAIN_REQUIREMENTS.md`, `ADMIN_NEW_REQUIREMENTS.md`, `VALIDATION_MATRIX.md` |
| Delivery governance | `#173` Establish executable delivery roadmap, capability maturity, and release gates | `BPANE-00173_DELIVERY_GOVERNANCE_PLAN.md`, `DELIVERY_ROADMAP.md`, `CAPABILITY_MATURITY_MATRIX.md`, `PRODUCT_PHASES_AND_RELEASE_GATES.md`, `RISK_REGISTER.md` |
| Phase 0 reference workflow | `#174` Qualify and deliver a Phase 0 reference workflow | `BPANE-00174_PHASE_0_REFERENCE_WORKFLOW_PLAN.md`, `DELIVERY_ROADMAP.md`, `PRODUCT_PHASES_AND_RELEASE_GATES.md` |
| Remote protocol product contract | `#175` Specify and conformance-test the BrowserPane remote protocol | `CAPABILITY_MATURITY_MATRIX.md`, `RISK_REGISTER.md`, `adr/0003-remote-protocol-product-contract.md` |
| Organization/project authorization | `#176` Enforce organization, project-role, and service-principal grants | `IDENTITY_ACCESS_REQUIREMENTS.md`, `CAPABILITY_MATURITY_MATRIX.md`, `RISK_REGISTER.md` |
| Identity lifecycle | `#177` Add identity provisioning, deprovisioning, and break-glass lifecycle | `IDENTITY_ACCESS_REQUIREMENTS.md`, `CAPABILITY_MATURITY_MATRIX.md`, `RISK_REGISTER.md` |
| Platform telemetry and SLOs | `#178` Add platform telemetry, SLOs, and capacity evidence | `BPANE-00178_PLATFORM_TELEMETRY_PLAN.md`, `PLATFORM_TELEMETRY.md`, `CAPABILITY_MATURITY_MATRIX.md`, `PRODUCT_PHASES_AND_RELEASE_GATES.md`, `RISK_REGISTER.md` |
| Threat model and production hardening baseline | `#223` Establish an evidence-linked threat model and hardening baseline | `BPANE-00223_THREAT_MODEL_BASELINE_PLAN.md`, `THREAT_MODEL.md`, `PRODUCTION_SECURITY_BASELINE.md`, `SECURITY_RUNTIME_ROADMAP.md`, `VALIDATION_MATRIX.md` |
| API conformance | `#179` Enforce control API conformance and compatibility governance | `ADMIN_NEW_API_COVERAGE.md`, `CAPABILITY_MATURITY_MATRIX.md`, `RISK_REGISTER.md` |
| Open-source governance | `#180` Resolve open-source licensing, contribution, and IP governance | `PRODUCT_PHASES_AND_RELEASE_GATES.md`, `RISK_REGISTER.md`, `adr/0004-open-source-license-governance.md` |

## Issue Body Audit

Checked again on 2026-07-31 against the live issue bodies, current
`/admin-new/` routes, implementation/test evidence, management claims, and the
consolidated docs. The first pass covered the original 19 open issues; the
reverse docs-to-issues pass created focused issues `#145` through `#170`; the
delivery-governance pass created `#173` through `#180`.

The current live result is 55 open issues: 19 broad/focused pre-existing
issues, the 26 work-order issues `#145` through `#170`, focused Phase N issues
`#171` and `#172`, governance issue `#173`, and seven focused cross-product
issues `#174` through `#180`. Every open issue is represented in this document
and links back to the consolidated docs.

The original 19 open issues remain relevant. None should be closed only
because of the docs consolidation or because `#142` is closed. The focused
issues now own the concrete work-order slices; the original broad issues remain
roadmap and enterprise/product context.

The cross-reference pass also updated the issue bodies in both directions:

- every broad roadmap issue now has a `Docs cross-reference` section with
  source docs, the central issue map, and related focused issues where
  applicable,
- every focused work-order issue from `#145` through `#170` has both `Docs
  source` and `Docs cross-reference` sections,
- focused implementation issues `#124`, `#145` through `#180` include an
  explicit example use case and post-implementation smoke sequence,
- every open issue links back to `docs/OPEN_ISSUES_CONTEXT.md`,
- stale wording that told slices to stay on broad issues was removed from the
  issue bodies.

| Issue | Current relevance | GitHub issue-body action | Admin-new reference status |
| --- | --- | --- | --- |
| `#6` | Relevant as umbrella tracker only. | Updated on GitHub 2026-07-10 to remove active-looking references to completed slices and point at this consolidated docs workspace. | Now mentions `#142` as closed lineage and `/admin-new/` as active side-by-side app, but does not own admin implementation PRs. |
| `#20` | Relevant for durable session inspection resources and broader diagnostics. #156 now supplies the route-backed current-state/event projection. | Updated on GitHub 2026-07-10 with `/admin-new/` session-inspector alignment and the existing-issue scoping rule. | `/admin-new/sessions/{id}/observability` is implemented; durable logs, audit history, and deeper diagnostics remain outside #156. |
| `#21` | Relevant. Recording export and session-file routes exist; generalized artifacts/browser outputs remain broader work. | Updated on GitHub 2026-07-10 with `/admin-new/recordings` and artifact-route alignment. | `/admin-new/recordings` and session-scoped files/recordings routes exist; generalized artifact catalog behavior remains broader scope. |
| `#28` | Relevant. Workflow webhooks exist; generalized resource events and security export remain. | Updated on GitHub 2026-07-10 with `/admin-new/` event-subscription and delivery-health alignment. | Workflow event subscriptions and delivery health are implemented under #159; generalized resource events and security export remain. |
| `#30` | Relevant as production/support backlog. | Updated on GitHub 2026-07-10 with `/admin-new/` support-bundle operator-surface alignment. | Support bundle generation/download should eventually be an admin-new operator route. |
| `#31` | Relevant but deferred product capability. | Updated on GitHub 2026-07-10 with `/admin-new/` session create/template/detail device-mode alignment. | Device-mode choices would belong in session create, templates, and session detail. |
| `#47` | Relevant, but the body contained stale route-scoping references to completed issues. | Updated on GitHub 2026-07-10 to replace old closed `#87`/`#89` routing notes with the current docs/work-order ownership. | Admin-new workflow catalog and workflow-run detail exist; publishing and deeper productization remain. |
| `#66` | Relevant parent; deployment packaging remains too broad for one PR. | Updated on GitHub 2026-08-14 to make admin-new the default console and delegate the bounded single-node package to #225. | #225 owns Compose; this parent retains Kubernetes/EKS, Fargate, shared storage, and cross-target deployment architecture. |
| `#69` | Relevant. Automation API productization remains broader than current MCP/workflow implementation. | Updated on GitHub 2026-07-10 with `/admin-new/` session-detail and API-companion alignment. | Session automation should surface in session detail and API companion routes. |
| `#70` | Relevant. Identity/access-review exists; API keys, immutable audit, and retention controls remain backlog. | Updated on GitHub 2026-07-10 with `/admin-new/identity` alignment. | Admin-new identity/access route is the near-term UI owner; API-key/audit/retention screens are later. |
| `#71` | Relevant but deferred product capability. | Updated on GitHub 2026-07-10 with `/admin-new/` session/workflow handoff alignment. | Admin-new handoff controls would belong in session/workflow detail. |
| `#72` | Relevant as the broad enterprise-security owner. Several completed and remaining hardening slices map here. | #223 merged through PR #224 and remains the durable evidence baseline. | #72 remains open for residual ingress, MCP, identity, runtime, data, and deployment hardening; #225 consumes the baseline for single-node packaging. |
| `#73` | Relevant but deferred production runbook work. | Updated on GitHub 2026-07-10 with `/admin-new/` restore-validation alignment. | Restore validation should include route availability and admin-new smoke checks. |
| `#74` | Relevant. Graceful shutdown/readiness is nearer term; full HA is later. | Updated on GitHub 2026-07-31 with the `#172` endpoint-availability boundary in addition to `/admin-new/` event-stream continuity and readiness alignment. | Admin-new should be part of event-stream continuity and route availability validation; `#172` consumes but does not own general HA. |
| `#75` | Relevant. The 2026-07-31 audit found patched critical/high dependency alerts, including runtime dependencies. Remediation and the minimal validation ratchet are nearer term; broader release governance is later. | Updated on GitHub 2026-07-31 to route the current dependency baseline and enforced Rust/Node lockfile scanning through `#151`. | Admin-new build/unit/smoke coverage and dependency safety must be included in the validation ratchet. |
| `#76` | Relevant but deferred enterprise storage/security capability. | Updated on GitHub 2026-07-31 with the `#172` data-classification/residency boundary in addition to `/admin-new/` effective-policy display alignment. | Effective residency/encryption policy belongs in admin-new once implemented; `#172` only references it. |
| `#79` | Relevant but broad. Current project-policy visibility is nearer term than a central policy engine. | Updated on GitHub 2026-07-31 with the narrow `#172` endpoint-grant boundary in addition to `/admin-new/` effective-policy diagnostics alignment. | Admin-new policy diagnostics should be part of the eventual effective-policy surface; generalized policy evaluation remains here. |
| `#80` | Relevant but broad. Current file-policy visibility is nearer term than pluggable DLP. | Updated on GitHub 2026-07-31 with the `#172` process-variable/artifact boundary in addition to `/admin-new/` file/artifact scan-state alignment. | Admin-new file/artifact indicators and filters are the expected operator UI; generalized inspection remains here. |
| `#124` | Relevant and focused. This is the cleanest current admin-new implementation issue. | Updated on GitHub 2026-07-10 to name `/admin-new/` explicitly as the target surface. | Admin-new session-template catalog route is missing and should be built when this issue is selected. |
| `#171` | Relevant and focused Phase N Teach Mode capability. | Created and cross-referenced on GitHub 2026-07-31; boundaries added to `#20`, `#21`, `#47`, `#71`, and `#172`. | Future full-width Workflow Studio and training routes; not an Admin-New promotion blocker. |
| `#172` | Relevant and focused Phase N external workflow integration capability. | Created and re-audited on GitHub 2026-07-31; boundaries added to `#6`, `#28`, `#47`, `#66`, `#69`-`#72`, `#74`, `#76`, `#79`, `#80`, `#147`, `#150`, `#161`, `#162`, `#164`, and `#171`. | Future Workflow Endpoint catalog/detail, immutable revisions, grants, completion/handoff profiles, runs, overload/readiness, state-event consistency, and delivery diagnostics; not an Admin-New promotion blocker. |
| `#173` | Current governance slice. | Created on GitHub 2026-07-31 with business case, use case, scope, acceptance, and smoke. | No runtime UI. It governs roadmap, maturity, gates, risks, and claim traceability. |
| `#174` | Qualified bounded Pilot Value slice. | Created on GitHub 2026-07-31 and backed by a dedicated Phase 0 plan. | Uses only the admin-new/operator surfaces selected by the Pilot agreement. |
| `#175` | Relevant protocol productization gap. | Created on GitHub 2026-07-31 after Rust/TypeScript version/conformance audit. | Diagnostics may expose safe version/capability metadata; no standalone admin catalog is required. |
| `#176` | Relevant authorization enforcement gap. | Created on GitHub 2026-07-31 to own organization/project roles and enforced service-principal grants. | Admin-new identity/project views must eventually show effective grants and denial reasons. |
| `#177` | Relevant later identity-lifecycle gap. | Created on GitHub 2026-07-31 to own provisioning/deprovisioning and safeguarded break-glass controls. | Admin-new identity/access review becomes the operator surface when implemented. |
| `#178` | Relevant production observability gap. | Created on GitHub 2026-07-31 to separate platform telemetry/SLOs from per-session inspection and readiness. | Admin-new observability consumes the common telemetry contract; it does not define a separate model. |
| `#179` | Merged public API governance baseline. | 131-operation inventory, pinned lint, executable examples, Axum route recognition, semantic compatibility, policy, and CI enforcement merged through PR #194. | #158 consumes the canonical generated evidence in admin-new without creating a second API truth. |
| `#180` | Relevant open-source trust/governance gap. | Created on GitHub 2026-07-31 after detecting AGPL root versus MIT Cargo metadata and absent contributor policies. | No dedicated admin route; documentation and release artifacts must be consistent. |
| `#214` | Implemented production-like runtime authorization boundary. | Typed browser/worker/storage operations and the gateway-isolated broker topology merged through PRs #220-#221. | Supplies runtime-boundary evidence to #223/#72/#66; it is not a supported production package by itself. |
| `#223` | Implemented and closed Production checkpoint. | Merged through PR #224 on 2026-08-14. | Durable threat model, responsibility baseline, composed static contract, negative-evidence audit, and synchronized security docs. |
| `#225` | Implemented and closed Production deployment checkpoint. | Independent broker-only single-node package and local qualification merged through PR #226 on 2026-08-14. | Target acceptance, Kubernetes/Fargate, HA, and broader production controls remain with #66 and focused owners. |
| `#227` | Closed Production telemetry checkpoint. | Created from #178 after #225 merged and closed through PR #228 on 2026-08-14. | Owns the merged W3C/OpenTelemetry gateway-to-broker browser runtime lifecycle trace checkpoint. |
| `#229` | Closed Production telemetry checkpoint. | Created from #178 after #227 merged and closed through PR #230 on 2026-08-16. | Owns shared label-free workflow/recording operations counters and their OpenMetrics qualification. |
| `#231` | Closed Production telemetry checkpoint. | Created from #178 after #229 merged and closed through PR #232 on 2026-08-16. | Owns merged Prometheus recording rules, starter alerts, rule tests, and operator runbooks. |
| `#233` | Current focused Production telemetry checkpoint. | Created from #178 after #231 merged; implementation started on 2026-08-16. | Owns provisioned aggregate Grafana dashboard and qualification without final SLO/alert-routing scope. |

## Docs-To-Issue Context

Open issues are listed as `#N`. The historical closed admin redesign lineage is
shown as `closed #142`.

| Docs file | Primary issue context |
| --- | --- |
| `ADMIN_INTERACTION_REQUIREMENTS.md` | `#20`, `#28`, `#153`, `#156`, closed `#142` |
| `ADMIN_NEW_API_COVERAGE.md` | `#69`, `#70`, `#124`, `#153`, `#158`, `#172`, closed `#142` |
| `ADMIN_NEW_IMPLEMENTATION_GUARDRAILS.md` | `#153`, `#163`, closed `#142` |
| `ADMIN_NEW_MANUAL_CHECKPOINTS.md` | `#124`, `#154`-`#163`, closed `#142` |
| `ADMIN_NEW_REQUIREMENTS.md` | `#20`, `#21`, `#47`, `#69`, `#124`, `#153`-`#159`, `#161`, `#163`, `#171`, `#172`, closed `#142` |
| `ADMIN_NEW_STATUS.md` | `#153`, `#163`, `#171`, `#172`, closed `#142` |
| `BPANE-00151_MINIMAL_CI_VALIDATION_PLAN.md` | closed `#151`, implemented Foundation validation baseline |
| `BPANE-00179_CONTROL_API_CONFORMANCE_PLAN.md` | `#179`, review-ready Foundation control-contract conformance and compatibility slice |
| `BPANE-00184_COMPOSE_VALIDATION_PERFORMANCE_PLAN.md` | `#184`, implemented Foundation validation-performance slice; measured build-cache follow-up `#185` |
| `BPANE-00185_CI_RUST_BUILDER_PLAN.md` | `#185`, active deterministic GHCR builder and compose-consumption slice |
| `BPANE-00167_DOCKER_RUNTIME_BOUNDARY_PLAN.md` | `#167`, merged direct compatibility proxy boundary; predecessor to `#214` |
| `BPANE-00214_RUNTIME_LAUNCH_BROKER_PLAN.md` | `#214`, typed broker, operation adapters, gateway routing, and broker-only topology completion |
| `BPANE-00223_THREAT_MODEL_BASELINE_PLAN.md` | `#223`, focused evidence-linked threat model and hardening-baseline checkpoint under `#72` |
| `BPANE-00225_SINGLE_NODE_COMPOSE_BASELINE_PLAN.md` | `#225`, focused single-node Compose package under parent `#66` |
| `BPANE-00227_OTEL_RUNTIME_TRACING_PLAN.md` | `#227`, focused gateway-to-broker tracing checkpoint under parent `#178` |
| `BPANE-00229_WORKFLOW_RECORDING_METRICS_PLAN.md` | `#229`, focused workflow/recording metrics checkpoint under parent `#178` |
| `BPANE-00231_PROMETHEUS_SLI_ALERT_BASELINE_PLAN.md` | `#231`, focused Prometheus SLI/alert/runbook checkpoint under parent `#178` |
| `BPANE-00233_GRAFANA_OPERATIONS_DASHBOARD_PLAN.md` | `#233`, focused Grafana operations-dashboard checkpoint under parent `#178` |
| `SINGLE_NODE_DEPLOYMENT.md` | `#225`, bounded operator runbook and support boundary under parent `#66` |
| `BPANE-00171_WORKFLOW_TEACH_MODE_PLAN.md` | `#171`, with dependencies on `#20`, `#21`, `#47`, `#71`, and `#172` |
| `BPANE-00172_BPM_WORKFLOW_ENDPOINT_INTEGRATION_PLAN.md` | `#172`, with dependencies on `#28`, `#47`, `#66`, `#69`-`#72`, `#74`, `#76`, `#79`, `#80`, `#147`, `#150`, `#161`, `#162`, `#164`, `#174`, `#176`, `#179`, and `#171` |
| `BPANE-00173_DELIVERY_GOVERNANCE_PLAN.md` | `#173` |
| `BPANE-00174_PHASE_0_REFERENCE_WORKFLOW_PLAN.md` | `#174`, after `#151`, with threat-profile-selected Foundation controls and conditional delivery dependencies on `#172`, `#149`, `#71`, `#66`, and `#154` |
| `DELIVERY_ROADMAP.md` | all open issues organized into Foundation, Pilot Value, Operator Product, Production, Enterprise, and Innovation lanes |
| `CAPABILITY_MATURITY_MATRIX.md` | all product capability owners, especially `#163`, `#171`-`#180` |
| `PRODUCT_PHASES_AND_RELEASE_GATES.md` | `#151`, `#145`-`#150`, `#152`, `#174`, `#153`-`#163`, and Production/Phase N owners |
| `RISK_REGISTER.md` | active and closed risk evidence, especially `#66`, `#72`, `#75`, `#175`-`#180`, and focused checkpoint `#223` |
| `THREAT_MODEL.md` | `#223` current evidence baseline, `#72` broader hardening, and residual owners `#28`, `#66`, `#69`, `#70`, `#73`-`#80`, `#175`-`#178` |
| `PRODUCTION_SECURITY_BASELINE.md` | `#223` application/deployment responsibility baseline, with production packaging owned by `#66` and broader hardening by `#72` |
| `PLAN_TEMPLATE.md` | every focused issue entering Ready or In Progress |
| `adr/` | `#173`, `#174`, `#175`, and `#180` plus related product-boundary issues |
| `DOMAIN_REQUIREMENTS.md` | `#20`, `#21`, `#28`, `#31`, `#47`, `#69`, `#124`, `#154`-`#157`, `#159`, `#161`, `#171`, `#172`, closed `#142` |
| `IDENTITY_ACCESS_REQUIREMENTS.md` | `#70`, `#157`, `#172`, `#176`, `#177`, closed `#142` |
| `IMPLEMENTATION_WORK_ORDER.md` | detailed issue/topic rationale; `DELIVERY_ROADMAP.md` owns current execution order |
| `OPEN_ISSUES_CONTEXT.md` | all open issues, plus closed `#142` lineage |
| `PROJECT_GOVERNANCE_REQUIREMENTS.md` | `#70`, `#79`, `#80`, `#161`, `#172`, closed `#142` |
| `RESOURCE_LIFECYCLE_REQUIREMENTS.md` | `#21`, `#66`, `#76`, `#80`, `#124`, `#148`, `#159`, `#160`, closed `#142` |
| `OPERATOR_CLI_AND_LOCAL_DIAGNOSTICS.md` | `#162`, canonical CLI support boundary and local troubleshooting sequence |
| `RUNTIME_OPERATOR_REQUIREMENTS.md` | `#66`, `#69`, `#72`, `#74`, `#150`, `#162`, `#167`, `#172`, `#214` |
| `SECURITY_RUNTIME_ROADMAP.md` | `#28`, `#66`, `#72`, `#74`, `#75`, `#145`-`#150`, `#164`-`#170` |
| `VALIDATION_MATRIX.md` | all implementation issues selected from this matrix, especially `#151`, `#152`, focused admin-new issues `#153`-`#163`, Phase N issues `#171`-`#172`, runtime boundary `#214`, and security baseline `#223` |
| `REVIEW_FINDINGS_RECONCILIATION.md` | `#72`, `#145`-`#150`, `#165`, `#169`, `#170`, plus security/runtime portions of `#28`, `#66`, `#74`, `#75` |
| `REVIEW_FINDINGS_COVERAGE_AUDIT.md` | `#72`, `#75`, `#151`, `#164`, `#166`, `#168`, and deferred product/platform backlog issues |
| `SOURCE_PLAN_INVENTORY.md` | `#6`, closed `#142` |
| `LEGACY_DOC_RETENTION_AUDIT.md` | `#6`, closed `#142` |
| `LEGACY_SECTION_COVERAGE_AUDIT.md` | `#6`, closed `#142` |
| `NEXT_WORKING_ROADMAP.md` | admin-new transition context for `#151`, `#145`, `#146`, `#149`, `#154`-`#158`, `#171`-`#180`, closed `#142` |
| `README.md` | `#6`, closed `#142` |
| `concept.html` | closed `#142` as design reference only |

## Issue Scoping Before Implementation

The delivery roadmap starts with validation and high-priority security slices,
then allows bounded Pilot, Operator Product, Production, and Enterprise lanes
to proceed through explicit gates. Before coding, select the relevant focused
issue and capture the exact slice boundary in a checked-in `docs/*_PLAN.md`:

| Work-order item | Current issue state | Recommended action |
| --- | --- | --- |
| Foundation validation baseline | closed `#151` | Keep the implemented validation scope intact. |
| Foundation validation performance | `#184` implemented; `#185` ready | Retain the measured sharding result; use #185 for deterministic Docker build acceleration without reducing coverage. |
| Remaining Foundation trust/runtime work | `#145`-`#150`, `#152`, and `#179` merged | Foundation Gate is complete; preserve its validation and compatibility evidence. |
| Active Operator Product work | none; `#124` qualified | Admin-new is promoted; select #124 only when session-template catalog work outranks the current Production slice. |
| Items 10-19: admin-new parity and promotion work | `#154`-`#163`, with session templates still tracked by `#124` | Use the matching focused issue and keep old admin regression scope explicit. |
| Items 20-26: scalability, runtime hygiene, docs, performance, and refactors | `#164`-`#170` | Use the matching focused issue and require validation evidence before broad refactors. |
| Pilot Value | `#174`, conditional `#172`, `#149`, `#71`, `#66`, `#154` | Use the Phase 0 plan and select only dependencies required by the agreed process. |
| Production/Enterprise and Innovation | `#233` in progress; broad issues plus `#175`-`#180`, `#172`, `#171` qualified | Complete the focused #233 operations-dashboard checkpoint; then reassess #178/#180/#66 and preserve the default #172-before-#171 Phase N order. |

## Issue Hygiene Notes

- #173 established priority, lane, state, and target-gate labels/milestones for
  every executable open issue. Umbrella tracker #6 deliberately has no delivery
  lane or target milestone. #151 and #173 have accountable assignees; assign
  other issues only when they enter Ready or In Progress.
- A GitHub Project with dependency/evidence fields is still optional follow-up.
  Until one is configured, issue labels and milestones are the live tracker
  state and `DELIVERY_ROADMAP.md` is the canonical sequencing source.
- Branch protection requires review/conversation resolution but no status
  checks. #151 must add real CI checks and make the selected minimal checks
  required before later issues treat them as evidence.
- `#142` is closed as completed. It remains the historical admin-new design
  lineage, and its companion-doc section was updated on 2026-07-10 to point at
  the consolidated docs instead of the removed
  `docs/ADMIN_APP_REDESIGN_FOUNDATION.md`.
- There is currently no open broad admin-new parent issue. Use `#124` for
  session-template catalog work, and use `#153`-`#163` for other focused
  admin-new implementation slices.
- `#6` remains useful as the umbrella tracker, but direct implementation PRs
  should reference the matching focused issue from `#145`-`#180` where one
  exists, plus the scoped local plan file.
- `#31`, `#71`, `#76`, `#79`, and `#80` are represented only at product
  backlog level in the docs. That is intentional for the current admin/security
  priority. If any of them becomes active, create a dedicated local plan file
  or expand the relevant domain doc before implementation starts.
- `#124` is the cleanest current issue for a focused admin-new resource slice.
- `#20`, `#21`, `#28`, `#47`, `#66`, `#69`, `#70`, `#72`, `#73`, `#74`, and
  `#75` remain valid but broad. Prefer the matching focused issue from
  `#145`-`#180` for implementation PRs when the slice is covered there.
- `#172` is the canonical external BPM Workflow Endpoint contract. It must not
  absorb workflow authoring/publishing from `#47`, direct session automation
  from `#69`, API key lifecycle from `#70`, generalized event export from
  `#28`, deployment/security/residency/DLP from `#66`, `#72`, `#76`, and `#80`,
  general HA/readiness/policy/CLI/scalability ownership from `#74`, `#79`,
  `#150`, `#161`, `#162`, and `#164`, Phase 0 delivery from `#174`,
  organization grant enforcement from `#176`, API conformance from `#179`, or
  Teach Mode from `#171`.
- Closed `#52` remains historical evidence for the implemented identity/access
  review and service-principal registry slices. Remaining organization/grant
  enforcement is owned by `#176`; provisioning/deprovisioning and break-glass
  lifecycle are owned by `#177`.
- `#173` owns delivery governance only. It must not become a catch-all product
  epic.
