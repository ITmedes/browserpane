# Open GitHub Issues Context

Created: 2026-07-10
Revalidated: 2026-08-22 after #277 merged through PR #279 and the resulting
protocol-bootstrap repair #280 closed through PR #281; #174 and #180 are
externally deferred, with ordered protocol issues #266-#268, then #124 as the
finite engineering fallback

This document maps the current `docs/` workspace to the live open GitHub
issues for `ITmedes/browserpane`. It is the bridge between the consolidated
local planning docs and the public issue tracker.

Source check:

- fetched and updated through the GitHub API on 2026-08-22,
- 34 open issues after closing #277 and #280, all represented in this
  documentation workspace,
- open issue range: `#6` through `#268`,
- focused docs-derived implementation issues created on 2026-07-10: `#145`
  through `#170`,
- focused Phase N Teach Mode issue created on 2026-07-31: `#171`,
- Workflow Endpoint issue `#172`, created on 2026-07-31 and narrowed to the
  bounded Phase 0 polling contract on 2026-08-20,
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
- focused Compose runner reliability issue `#235` merged through PR `#236`,
- deferred Workflow Endpoint productization follow-up created on 2026-08-20:
  `#240`,
- Codex-native local delivery-loop issue `#241` completed through PR `#243`;
  follow-ups through `#257` added disk safety, requirements specification,
  result and merge contracts, admin merge, smoke reliability, and external-gate
  deferral; `#260` owns finite ordered fallback traversal,
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
| Focused Pilot Value issues | `#47`, `#172`, `#174` | Use `#47` for the immutable Playwright package, `#172` for one polling-based BPM activity endpoint, and `#174` for the selected real Pilot process. |
| Deferred workflow productization | `#171`, `#240` | Use `#171` for Teach Mode and controlled repair. Use `#240` for endpoint revisions, callbacks, replay, tracing, throttling, and connector compatibility. Neither is a Phase 0 dependency. |
| Delivery governance | `#173` | Owns the canonical roadmap, maturity, gates, risks, plan template, and issue/claim reconciliation. It does not own runtime features. |
| Contributor automation | `#241`, `#244`, `#246`, `#248`, `#251`, `#253`, `#255`, `#257`, `#260`, `#273`, `#275`, `#277` | These issues own the optional bounded Codex loop, disk safety, requirements specification, result/merge handling, smoke reliability, exact-head/post-merge gates, external-gate deferral, and finite fallback traversal. None changes product priority or capability maturity. |
| Focused cross-product gaps | `#174` through `#180` | Use these for Phase 0 delivery, protocol conformance, authorization, identity lifecycle, platform telemetry, API compatibility, and open-source governance. |
| Focused validation performance | `#184` | Preserve the #151 validation baseline while reducing hosted compose feedback time through isolated execution lanes. |
| Focused Docker build acceleration | `#185` | Preserve #184 lane coverage while adding deterministic, supply-chain-safe Docker build reuse for trusted hosted runs. |
| Focused runtime authorization boundary | `#214` | Complete the typed, policy-validating broker and gateway-isolated Docker-host topology beyond the #167 direct compatibility boundary. |
| Completed threat-model baseline | closed `#223` | Durable evidence baseline merged through PR #224; #72 keeps residual security ownership. |
| Completed single-node deployment | closed `#225` | Independent single-node package merged through PR #226; #66 keeps broader deployment ownership. |
| Completed runtime tracing | closed `#227` | Gateway-to-broker browser lifecycle tracing merged through PR #228; #178 keeps broader telemetry ownership. |
| Completed subsystem metrics | closed `#229` | Workflow/recording operations counters merged through PR #230; #178 keeps broader telemetry ownership. |
| Completed SLI/alert baseline | closed `#231` | Prometheus recording rules, starter alerts, deterministic rule tests, and operator runbooks merged through PR #232. |
| Completed operations dashboard | closed `#233` | Provisioned aggregate Grafana dashboard merged through PR #234 under #178. |
| Product/platform backlog | `#20`, `#21`, `#28`, `#30`, `#31`, `#66`, `#69`, `#70`, `#71`, `#72`, `#73`, `#74`, `#75`, `#76`, `#79`, `#80` | Keep as roadmap and enterprise/product context. Prefer a bounded focused issue when an implementation slice is selected. |
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
| `#47` Supported Playwright workflow package and publishing contract | `BPANE-00047_WORKFLOW_PACKAGE_CONTRACT_PLAN.md`, `DOMAIN_REQUIREMENTS.md`, `ADMIN_NEW_API_COVERAGE.md`, `RUNTIME_OPERATOR_REQUIREMENTS.md` | First Ready Pilot Value slice | Freeze Git commit-pinned Playwright source, entrypoint, schemas, resources, credentials, and regression evidence. |
| `#66` Compose, Kubernetes, and AWS Fargate deployments | `RUNTIME_OPERATOR_REQUIREMENTS.md`, `SECURITY_RUNTIME_ROADMAP.md`, `IMPLEMENTATION_WORK_ORDER.md` | Items 6, 18, 22, 23, 27 | Parent deployment owner. #225 owns the current single-node Compose slice; Kubernetes/EKS, Fargate, shared-storage expansion, and cross-target architecture remain here. |
| `#69` Session-scoped automation connection APIs | `DOMAIN_REQUIREMENTS.md`, `ADMIN_NEW_API_COVERAGE.md`, `RUNTIME_OPERATOR_REQUIREMENTS.md` | Items 12, 18, 27 | Session automation route visibility is admin parity; productized external contract is later. |
| `#70` API key, audit log, and retention policy controls | `IDENTITY_ACCESS_REQUIREMENTS.md`, `PROJECT_GOVERNANCE_REQUIREMENTS.md`, `IMPLEMENTATION_WORK_ORDER.md` | Items 13, 17, 27 | Identity route can surface current access review now; API keys/audit are enterprise backlog. |
| `#71` Signed human handoff, challenge detection, and private fallback | `IMPLEMENTATION_WORK_ORDER.md`, `DOMAIN_REQUIREMENTS.md` | Deferred Operator Product | Explicitly not Phase 0. Challenges return `external_intervention_required` to the external BPM. |
| `#72` Enterprise security hardening baseline and threat model | `THREAT_MODEL.md`, `PRODUCTION_SECURITY_BASELINE.md`, `SECURITY_RUNTIME_ROADMAP.md`, `REVIEW_FINDINGS_RECONCILIATION.md`, `IMPLEMENTATION_WORK_ORDER.md` | Items 1, 2, 3, 4, 6, 22, 23 | Broad residual security-hardening owner. #223 is merged evidence; supported deployment packaging remains #66/#225. |
| `#73` Backup, restore, and disaster recovery runbooks | `SECURITY_RUNTIME_ROADMAP.md`, `IMPLEMENTATION_WORK_ORDER.md`, `RUNTIME_OPERATOR_REQUIREMENTS.md` | Items 22, 27 | Deferred production runbook work. |
| `#74` High availability and zero-downtime operations | `SECURITY_RUNTIME_ROADMAP.md`, `IMPLEMENTATION_WORK_ORDER.md` | Items 6, 20, 23, 27 | Graceful shutdown/readiness are near-term; full HA is later. |
| `#75` Supply chain security and release governance | `REVIEW_FINDINGS_COVERAGE_AUDIT.md`, `IMPLEMENTATION_WORK_ORDER.md`, `VALIDATION_MATRIX.md` | Items 7, 22, 27 | Current critical/high dependency remediation and the CI/validation ratchet are near-term through `#151`; broader release governance is later. |
| `#76` Data residency, encryption, and BYOK controls | `IMPLEMENTATION_WORK_ORDER.md`, `RESOURCE_LIFECYCLE_REQUIREMENTS.md` | Item 27 | Deferred enterprise storage/security capability. |
| `#79` Central enterprise policy engine | `PROJECT_GOVERNANCE_REQUIREMENTS.md`, `IMPLEMENTATION_WORK_ORDER.md` | Items 17, 27 | Current project-policy UX is nearer term; central policy engine is later. |
| `#80` DLP and content inspection hooks | `PROJECT_GOVERNANCE_REQUIREMENTS.md`, `IMPLEMENTATION_WORK_ORDER.md` | Items 17, 27 | Current file policy visibility is nearer term; DLP provider hooks are later. |
| `#124` Admin session template catalog management | `BPANE-00124_SESSION_TEMPLATE_CATALOG_PLAN.md`, `DOMAIN_REQUIREMENTS.md`, `RESOURCE_LIFECYCLE_REQUIREMENTS.md`, `ADMIN_NEW_REQUIREMENTS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md` | Final current fallback after #268 | Focused Admin-New catalog plus additive active/archived API state. Use as canonical issue for template catalog work. |
| `#171` Workflow Studio Teach Mode and controlled demonstration-to-workflow publishing | `BPANE-00171_WORKFLOW_TEACH_MODE_PLAN.md`, `DOMAIN_REQUIREMENTS.md`, `ADMIN_NEW_REQUIREMENTS.md`, `VALIDATION_MATRIX.md` | Deferred Phase N Innovation | Canonical owner for semantic demonstrations, candidate generation, replay, and repair. Explicitly not Phase 0. |
| `#172` Phase 0 project-scoped Workflow Endpoint for BPM polling | `BPANE-00172_PHASE_0_WORKFLOW_ENDPOINT_PLAN.md`, `DOMAIN_REQUIREMENTS.md`, `IDENTITY_ACCESS_REQUIREMENTS.md`, `ADMIN_NEW_REQUIREMENTS.md`, `PROJECT_GOVERNANCE_REQUIREMENTS.md`, `RUNTIME_OPERATOR_REQUIREMENTS.md`, `VALIDATION_MATRIX.md` | Second Pilot Value slice after #47 | Canonical owner for one stable endpoint, service-principal grant, schemas, idempotency, typed outcomes, timeout/cancel, side-effect certainty, artifacts, and polling conformance. |
| `#173` Executable delivery roadmap, capability maturity, and release gates | `BPANE-00173_DELIVERY_GOVERNANCE_PLAN.md`, `DELIVERY_ROADMAP.md`, `CAPABILITY_MATURITY_MATRIX.md`, `PRODUCT_PHASES_AND_RELEASE_GATES.md`, `RISK_REGISTER.md` | Governance baseline | Canonical owner for delivery structure and cross-reference integrity. No runtime scope. |
| `#174` Bounded Phase 0 BPM browser activity | `BPANE-00174_PHASE_0_REFERENCE_WORKFLOW_PLAN.md`, `DELIVERY_ROADMAP.md`, `PRODUCT_PHASES_AND_RELEASE_GATES.md` | Third Pilot Value slice after #47/#172 | Canonical owner for candidate qualification, one real activity, operating evidence, and Stop/Operate/Phase 1 exit. No subprocess, Human Handoff, or Teach Mode. |
| `#175` Remote protocol v1 productization tracker | `BPANE-00175_REMOTE_PROTOCOL_V1_PLAN.md`, `CAPABILITY_MATURITY_MATRIX.md`, `RISK_REGISTER.md`, ADR 0003 | Open blocked Production tracker | Non-executable program owner for the ordered implementation issues #263-#268; only #268 may close it. |
| `#263` Remote protocol contract and vector baseline | `BPANE-00263_REMOTE_PROTOCOL_CONTRACT_PLAN.md`, `BPANE-00175_REMOTE_PROTOCOL_V1_PLAN.md`, `VALIDATION_MATRIX.md` | Protocol slice 1 complete | Normative contract and current Rust/TypeScript vector consumption merged. |
| `#264` Protocol negotiation codecs | `BPANE-00264_PROTOCOL_NEGOTIATION_CODECS_PLAN.md`, `BPANE-00175_REMOTE_PROTOCOL_V1_PLAN.md`, `VALIDATION_MATRIX.md` | Protocol slice 2 complete | Pure Rust/TypeScript codecs, selection, and negotiation vectors merged. |
| `#265` Gateway protocol negotiation | `BPANE-00265_GATEWAY_PROTOCOL_NEGOTIATION_PLAN.md`, `BPANE-00175_REMOTE_PROTOCOL_V1_PLAN.md`, `VALIDATION_MATRIX.md` | Protocol slice 3 complete | Gateway pre-session negotiation enforcement and checked old-client compatibility merged. |
| `#266` Browser protocol negotiation | `BPANE-00266_BROWSER_PROTOCOL_NEGOTIATION_PLAN.md`, `BPANE-00175_REMOTE_PROTOCOL_V1_PLAN.md`, `VALIDATION_MATRIX.md` | Protocol slice 4 after #265 | Delay browser readiness, gate capabilities, preserve checked old gateways, and expose typed SDK errors. |
| `#267` Protocol fuzzing and malformed-state coverage | `BPANE-00267_PROTOCOL_FUZZING_PLAN.md`, `BPANE-00175_REMOTE_PROTOCOL_V1_PLAN.md`, `VALIDATION_MATRIX.md` | Protocol slice 5 after #266 | Add deterministic corpus replay, four Rust fuzz targets, sanitizer evidence, and isolation tests. |
| `#268` Protocol compatibility qualification | `BPANE-00268_PROTOCOL_COMPATIBILITY_QUALIFICATION_PLAN.md`, `BPANE-00175_REMOTE_PROTOCOL_V1_PLAN.md`, `VALIDATION_MATRIX.md` | Protocol slice 6 after #267 | Prove rolling upgrade/rollback, current features, Admin-New diagnostics, and close #175. |
| closed `#277` Exact-SHA post-merge workflow retry | `BPANE-00277_POST_MERGE_WORKFLOW_RETRY_PLAN.md`, `BPANE-00273_EXACT_HEAD_COMPOSE_GATE_PLAN.md`, `DELIVERY_ROADMAP.md` | Completed Foundation slice | Same-run exact-SHA failed-job retry merged through PR #279; repeated failure remains terminal. |
| closed `#280` Protocol bootstrap compatibility and session cleanup | `BPANE-00280_PROTOCOL_BOOTSTRAP_CLEANUP_PLAN.md`, `BPANE-00265_GATEWAY_PROTOCOL_NEGOTIATION_PLAN.md`, `VALIDATION_MATRIX.md` | Completed Foundation repair | Checked legacy bytes survive stream chunk boundaries and bootstrap write failure removes admitted registry clients; merged through PR #281. |
| `#176` Organization/project-role/service-principal grant enforcement | `IDENTITY_ACCESS_REQUIREMENTS.md`, `CAPABILITY_MATURITY_MATRIX.md`, `RISK_REGISTER.md` | Enterprise lane | Canonical owner for enforceable authorization and migration from owner-scoped deployments. |
| `#177` Provisioning/deprovisioning and break-glass lifecycle | `IDENTITY_ACCESS_REQUIREMENTS.md`, `CAPABILITY_MATURITY_MATRIX.md`, `RISK_REGISTER.md` | Enterprise lane | Canonical owner for remaining identity lifecycle scope from closed #52. |
| `#178` Platform telemetry, SLOs, and capacity evidence | `BPANE-00178_PLATFORM_TELEMETRY_PLAN.md`, `PLATFORM_TELEMETRY.md`, `DELIVERY_ROADMAP.md`, `CAPABILITY_MATURITY_MATRIX.md`, `RISK_REGISTER.md` | Production lane | Metrics, traces, rules/runbooks, and Grafana baseline merged through PRs #222, #228, #230, #232, and #234. Broader traces/metrics, calibrated SLOs, alert routing, synthetics, and tested envelopes remain. |
| `#179` Control API conformance and compatibility | `ADMIN_NEW_API_COVERAGE.md`, `CAPABILITY_MATURITY_MATRIX.md`, `RISK_REGISTER.md` | Foundation/Production lane | Canonical owner for OpenAPI lint, route/schema conformance, examples, and breaking-change policy. |
| `#180` Open-source license/contribution/IP governance | `PRODUCT_PHASES_AND_RELEASE_GATES.md`, `RISK_REGISTER.md`, ADR 0004 | P0 Foundation gate | Resolve AGPL root versus Cargo MIT and missing Node metadata plus contribution/security/IP policy before external Pilot reliance. |
| `#184` Reduce representative compose validation wall time | `BPANE-00184_COMPOSE_VALIDATION_PERFORMANCE_PLAN.md`, `VALIDATION_MATRIX.md` | Foundation maintenance | Canonical owner for hosted compose sharding and timing evidence without reducing the #151 scenario baseline. |
| `#185` Persist deterministic Docker build cache | `BPANE-00185_CI_RUST_BUILDER_PLAN.md`, `BPANE-00184_COMPOSE_VALIDATION_PERFORMANCE_PLAN.md`, `VALIDATION_MATRIX.md` | Foundation maintenance | Follow-up owner for a deterministic GHCR Rust builder after #184; it must preserve coverage and cache trust boundaries. |
| `#214` Policy-validating runtime launch broker | `BPANE-00214_RUNTIME_LAUNCH_BROKER_PLAN.md`, `BPANE-00167_DOCKER_RUNTIME_BOUNDARY_PLAN.md`, `DELIVERY_ROADMAP.md`, `CAPABILITY_MATURITY_MATRIX.md`, `RISK_REGISTER.md`, `RUNTIME_OPERATOR_REQUIREMENTS.md`, `VALIDATION_MATRIX.md` | Production lane, completion of item 23 | Canonical owner for authenticated typed launch/lifecycle/storage operations, request-body policy enforcement, and the gateway-isolated production-like Docker-host topology. |
| `#225` Hardened single-node Compose deployment | `BPANE-00225_SINGLE_NODE_COMPOSE_BASELINE_PLAN.md`, `SINGLE_NODE_DEPLOYMENT.md`, `THREAT_MODEL.md`, `PRODUCTION_SECURITY_BASELINE.md`, `RUNTIME_OPERATOR_REQUIREMENTS.md`, `DELIVERY_ROADMAP.md`, `RISK_REGISTER.md`, `VALIDATION_MATRIX.md` | Closed Production child of #66 | Independent broker-only manifest, preflight, live workflow/recording/restart qualification, and bounded runbook merged through PR #226. |
| `#227` OpenTelemetry broker runtime tracing | `BPANE-00227_OTEL_RUNTIME_TRACING_PLAN.md`, `PLATFORM_TELEMETRY.md`, `DELIVERY_ROADMAP.md`, `CAPABILITY_MATURITY_MATRIX.md`, `RISK_REGISTER.md`, `VALIDATION_MATRIX.md` | Closed Production child of #178 | W3C propagation and bounded browser runtime lifecycle spans across gateway and broker merged through PR #228 with private collector parentage, redaction, malformed-context, and outage/recovery smoke evidence. |
| `#229` Workflow and recording OpenMetrics | `BPANE-00229_WORKFLOW_RECORDING_METRICS_PLAN.md`, `PLATFORM_TELEMETRY.md`, `DELIVERY_ROADMAP.md`, `CAPABILITY_MATURITY_MATRIX.md`, `RISK_REGISTER.md`, `VALIDATION_MATRIX.md` | Closed Production child of #178 | Shared label-free workflow/recording operation counters merged through PR #230. |
| `#231` Prometheus SLI and alert baseline | `BPANE-00231_PROMETHEUS_SLI_ALERT_BASELINE_PLAN.md`, `PLATFORM_TELEMETRY.md`, `DELIVERY_ROADMAP.md`, `CAPABILITY_MATURITY_MATRIX.md`, `RISK_REGISTER.md`, `VALIDATION_MATRIX.md` | Closed Production child of #178 | Recording rules, conservative starter alerts, deterministic behavior tests, and operator runbooks merged through PR #232. |
| closed `#233` Grafana operations dashboard | `BPANE-00233_GRAFANA_OPERATIONS_DASHBOARD_PLAN.md`, `PLATFORM_TELEMETRY.md`, `DELIVERY_ROADMAP.md`, `CAPABILITY_MATURITY_MATRIX.md`, `RISK_REGISTER.md`, `VALIDATION_MATRIX.md` | Completed Production checkpoint | Provisioned aggregate dashboard merged through PR #234. |
| `#240` Workflow Endpoint lifecycle, callbacks, and connector compatibility | `BPANE-00240_WORKFLOW_ENDPOINT_PRODUCTIZATION_PLAN.md`, historical `BPANE-00172_BPM_WORKFLOW_ENDPOINT_INTEGRATION_PLAN.md` | Deferred Production/Enterprise | Owns endpoint revisions, callbacks, replay, tracing expansion, throttling, and connector compatibility after #172. |
| closed `#241` Codex-native local delivery loop | `BPANE-00241_CODEX_DEVELOPMENT_LOOP_PLAN.md`, `CURRENT_CONTEXT.md`, `DELIVERY_ROADMAP.md` | Foundation contributor tooling | Optional bounded qualification/proposal/repair orchestration. It may promote one already-scoped Qualified issue only after roadmap, dependency, plan, risk, acceptance, and test checks; it does not create backlog, replace plans, reprioritize work, or advance a product gate. |
| closed `#244` Local development-loop disk guard | `BPANE-00244_DEV_LOOP_DISK_GUARD_PLAN.md`, `CURRENT_CONTEXT.md`, `DELIVERY_ROADMAP.md` | Foundation contributor-tooling follow-up | Stops local Codex work below a configurable 50 GiB default without deleting Docker or user data. |
| closed `#246` Bounded requirements-specification cycle | `BPANE-00246_REQUIREMENTS_SPECIFICATION_LOOP_PLAN.md`, `BPANE-00241_CODEX_DEVELOPMENT_LOOP_PLAN.md`, `CURRENT_CONTEXT.md`, `DELIVERY_ROADMAP.md` | Foundation contributor-tooling follow-up | Lets qualification route one correctly ordered issue with evidence-backed contract gaps to a separate documentation PR; lifecycle promotion and implementation require a later post-merge qualification pass. |
| `#260` Ordered qualification fallback | `BPANE-00260_ORDERED_QUALIFICATION_FALLBACK_PLAN.md`, `CURRENT_CONTEXT.md`, `DELIVERY_ROADMAP.md` | Foundation contributor-tooling follow-up | Traverses only the finite roadmap-owned fallback queue when multiple earlier candidates wait for external decisions. |

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
| Phase 0 workflow package | `#47` Freeze the supported Playwright workflow package and publishing contract | `BPANE-00047_WORKFLOW_PACKAGE_CONTRACT_PLAN.md`, `DOMAIN_REQUIREMENTS.md`, `ADMIN_NEW_REQUIREMENTS.md`, `VALIDATION_MATRIX.md` |
| Phase 0 BPM Workflow Endpoint | `#172` Add a project-scoped Workflow Endpoint for BPM polling | `BPANE-00172_PHASE_0_WORKFLOW_ENDPOINT_PLAN.md`, `DOMAIN_REQUIREMENTS.md`, `IDENTITY_ACCESS_REQUIREMENTS.md`, `ADMIN_NEW_REQUIREMENTS.md`, `PROJECT_GOVERNANCE_REQUIREMENTS.md`, `RUNTIME_OPERATOR_REQUIREMENTS.md`, `VALIDATION_MATRIX.md` |
| Production/Enterprise Workflow Endpoint expansion | `#240` Productize endpoint lifecycle, callbacks, and connector compatibility | `BPANE-00240_WORKFLOW_ENDPOINT_PRODUCTIZATION_PLAN.md`, historical `BPANE-00172_BPM_WORKFLOW_ENDPOINT_INTEGRATION_PLAN.md` |
| Foundation contributor automation | `#241` Add a Codex-native local delivery loop | `BPANE-00241_CODEX_DEVELOPMENT_LOOP_PLAN.md`, `CURRENT_CONTEXT.md`, `DELIVERY_ROADMAP.md` |
| Foundation contributor safety | `#244` Stop the local Codex loop before low disk space becomes unsafe | `BPANE-00244_DEV_LOOP_DISK_GUARD_PLAN.md`, `CURRENT_CONTEXT.md`, `DELIVERY_ROADMAP.md` |
| Foundation requirements automation | `#246` Add bounded requirements-specification cycles | `BPANE-00246_REQUIREMENTS_SPECIFICATION_LOOP_PLAN.md`, `BPANE-00241_CODEX_DEVELOPMENT_LOOP_PLAN.md`, `CURRENT_CONTEXT.md`, `DELIVERY_ROADMAP.md` |
| Foundation ordered qualification | `#260` Traverse the documented fallback queue | `BPANE-00260_ORDERED_QUALIFICATION_FALLBACK_PLAN.md`, `CURRENT_CONTEXT.md`, `DELIVERY_ROADMAP.md` |
| Phase N. Workflow Studio Teach Mode | `#171` Add Workflow Studio Teach Mode and controlled demonstration-to-workflow publishing | `BPANE-00171_WORKFLOW_TEACH_MODE_PLAN.md`, `DOMAIN_REQUIREMENTS.md`, `ADMIN_NEW_REQUIREMENTS.md`, `VALIDATION_MATRIX.md` |
| Delivery governance | `#173` Establish executable delivery roadmap, capability maturity, and release gates | `BPANE-00173_DELIVERY_GOVERNANCE_PLAN.md`, `DELIVERY_ROADMAP.md`, `CAPABILITY_MATURITY_MATRIX.md`, `PRODUCT_PHASES_AND_RELEASE_GATES.md`, `RISK_REGISTER.md` |
| Phase 0 BPM browser activity | `#174` Qualify and deliver one bounded Phase 0 BPM browser activity | `BPANE-00174_PHASE_0_REFERENCE_WORKFLOW_PLAN.md`, `DELIVERY_ROADMAP.md`, `PRODUCT_PHASES_AND_RELEASE_GATES.md` |
| Remote protocol product contract | `#175` Track BrowserPane remote protocol v1 productization | `BPANE-00175_REMOTE_PROTOCOL_V1_PLAN.md`, `CAPABILITY_MATURITY_MATRIX.md`, `RISK_REGISTER.md`, `adr/0003-remote-protocol-product-contract.md` |
| Remote protocol implementation sequence | `#263` -> `#264` -> `#265` -> `#266` -> `#267` -> `#268` | `BPANE-00263_REMOTE_PROTOCOL_CONTRACT_PLAN.md`, `BPANE-00264_PROTOCOL_NEGOTIATION_CODECS_PLAN.md`, `BPANE-00265_GATEWAY_PROTOCOL_NEGOTIATION_PLAN.md`, `BPANE-00266_BROWSER_PROTOCOL_NEGOTIATION_PLAN.md`, `BPANE-00267_PROTOCOL_FUZZING_PLAN.md`, `BPANE-00268_PROTOCOL_COMPATIBILITY_QUALIFICATION_PLAN.md` |
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

The current live issue set includes broad roadmap owners, focused work-order and
governance issues, and the six protocol implementation children `#263` through
`#268`. Every open issue remains represented in this document and links back to
the consolidated docs.

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
- focused implementation issues `#124`, `#145` through `#180`, and `#263`
  through `#268` include an
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
| `#47` | First Ready Phase 0 slice. | Refocused on GitHub 2026-08-20 and backed by `BPANE-00047_WORKFLOW_PACKAGE_CONTRACT_PLAN.md`. | Freeze the immutable Playwright/Git package before #172 consumes it. |
| `#66` | Relevant parent; deployment packaging remains too broad for one PR. | Updated on GitHub 2026-08-14 to make admin-new the default console and delegate the bounded single-node package to #225. | #225 owns Compose; this parent retains Kubernetes/EKS, Fargate, shared storage, and cross-target deployment architecture. |
| `#69` | Relevant. Automation API productization remains broader than current MCP/workflow implementation. | Updated on GitHub 2026-07-10 with `/admin-new/` session-detail and API-companion alignment. | Session automation should surface in session detail and API companion routes. |
| `#70` | Relevant. Identity/access-review exists; API keys, immutable audit, and retention controls remain backlog. | Updated on GitHub 2026-07-10 with `/admin-new/identity` alignment. | Admin-new identity/access route is the near-term UI owner; API-key/audit/retention screens are later. |
| `#71` | Relevant but deferred product capability. | Relabeled and commented on GitHub 2026-08-20. | Not a Phase 0 dependency; future Admin-New handoff controls belong in session/workflow detail. |
| `#72` | Relevant as the broad enterprise-security owner. Several completed and remaining hardening slices map here. | #223 merged through PR #224 and remains the durable evidence baseline. | #72 remains open for residual ingress, MCP, identity, runtime, data, and deployment hardening; #225 consumes the baseline for single-node packaging. |
| `#73` | Relevant but deferred production runbook work. | Updated on GitHub 2026-07-10 with `/admin-new/` restore-validation alignment. | Restore validation should include route availability and admin-new smoke checks. |
| `#74` | Relevant. Graceful shutdown/readiness is nearer term; full HA is later. | Updated on GitHub 2026-07-31 with the `#172` endpoint-availability boundary in addition to `/admin-new/` event-stream continuity and readiness alignment. | Admin-new should be part of event-stream continuity and route availability validation; `#172` consumes but does not own general HA. |
| `#75` | Relevant. The 2026-07-31 audit found patched critical/high dependency alerts, including runtime dependencies. Remediation and the minimal validation ratchet are nearer term; broader release governance is later. | Updated on GitHub 2026-07-31 to route the current dependency baseline and enforced Rust/Node lockfile scanning through `#151`. | Admin-new build/unit/smoke coverage and dependency safety must be included in the validation ratchet. |
| `#76` | Relevant but deferred enterprise storage/security capability. | Updated on GitHub 2026-07-31 with the `#172` data-classification/residency boundary in addition to `/admin-new/` effective-policy display alignment. | Effective residency/encryption policy belongs in admin-new once implemented; `#172` only references it. |
| `#79` | Relevant but broad. Current project-policy visibility is nearer term than a central policy engine. | Updated on GitHub 2026-07-31 with the narrow `#172` endpoint-grant boundary in addition to `/admin-new/` effective-policy diagnostics alignment. | Admin-new policy diagnostics should be part of the eventual effective-policy surface; generalized policy evaluation remains here. |
| `#80` | Relevant but broad. Current file-policy visibility is nearer term than pluggable DLP. | Updated on GitHub 2026-07-31 with the `#172` process-variable/artifact boundary in addition to `/admin-new/` file/artifact scan-state alignment. | Admin-new file/artifact indicators and filters are the expected operator UI; generalized inspection remains here. |
| `#124` | Relevant and focused final fallback after #268. | Revalidated on 2026-08-22 against the current template API, CLI, create-session flow, and missing catalog. | Add the Admin-New catalog and additive active/archived API state; do not imply delete or version-history support. |
| `#171` | Relevant and focused Phase N Teach Mode capability. | Explicitly deferred from Phase 0 on GitHub 2026-08-20. | Future Workflow Studio and training routes only after validated demand and stable execution contracts. |
| `#172` | Required Phase 0 polling endpoint. | Refocused on GitHub 2026-08-20 and backed by a bounded plan. | One endpoint, machine grant, schemas, idempotency, typed outcomes, side-effect certainty, artifacts, and polling; advanced lifecycle moved to #240. |
| `#173` | Current governance slice. | Created on GitHub 2026-07-31 with business case, use case, scope, acceptance, and smoke. | No runtime UI. It governs roadmap, maturity, gates, risks, and claim traceability. |
| `#174` | Qualified bounded Pilot Value slice. | Refocused on GitHub and locally on 2026-08-20. | One external BPM activity and normally one session; no subprocess, internal handoff, or training. |
| `#175` | Open blocked non-executable protocol program tracker. | Reopened and reconciled on GitHub 2026-08-22 after premature manual closure; completed children #263-#265 are checked. | Final child #268 adds diagnostics to existing session surfaces and is the only child that may close #175. |
| `#263` | Completed first protocol slice. | Closed with its normative contract/current-vector evidence. | No Admin-New impact. |
| `#264` | Completed second protocol slice. | Closed with codec/conformance evidence. | No Admin-New impact; no runtime wiring. |
| `#265` | Completed third protocol slice. | Closed with gateway enforcement and rollback evidence. | Diagnostics are runtime inputs only; presentation waits for #268. |
| `#266` | Fourth focused protocol slice after #265. | Created on GitHub 2026-08-21 with browser enforcement and SDK boundaries. | Produces typed errors/snapshot consumed by #268. |
| `#267` | Fifth focused protocol slice after #266. | Created on GitHub 2026-08-21 with bounded fuzz/sanitizer evidence. | No Admin-New change. |
| `#268` | Final focused protocol qualification slice after #267. | Created on GitHub 2026-08-21 as the #175 closure owner. | Integrates safe live diagnostics into existing session preview/observability surfaces. |
| `#277` | Completed Foundation reliability slice. | Merged through PR #279; run `32584253673` proved the one-retry fail-closed bound. | Adds one bounded exact-SHA failed-job rerun after merge; no product behavior or gate suppression. |
| `#280` | Completed focused Foundation repair. | Created from both failed attempts of Compose run `32584253673` and merged through PR #281. | Corrected byte-stream handoff and post-admission cleanup without broadening legacy compatibility or absorbing browser negotiation. |
| `#176` | Relevant authorization enforcement gap. | Created on GitHub 2026-07-31 to own organization/project roles and enforced service-principal grants. | Admin-new identity/project views must eventually show effective grants and denial reasons. |
| `#177` | Relevant later identity-lifecycle gap. | Created on GitHub 2026-07-31 to own provisioning/deprovisioning and safeguarded break-glass controls. | Admin-new identity/access review becomes the operator surface when implemented. |
| `#178` | Relevant production observability gap. | Created on GitHub 2026-07-31 to separate platform telemetry/SLOs from per-session inspection and readiness. | Admin-new observability consumes the common telemetry contract; it does not define a separate model. |
| `#179` | Merged public API governance baseline. | 131-operation inventory, pinned lint, executable examples, Axum route recognition, semantic compatibility, policy, and CI enforcement merged through PR #194. | #158 consumes the canonical generated evidence in admin-new without creating a second API truth. |
| `#180` | P0 open-source trust/governance gate. | Qualified against `BPANE-00180_OPEN_SOURCE_GOVERNANCE_PLAN.md`; the reviewed legal/business decision remains an external input before implementation. | Resolve metadata, notices, contribution/IP governance, and claim consistency before external Pilot reliance; no dedicated admin route. |
| `#214` | Implemented production-like runtime authorization boundary. | Typed browser/worker/storage operations and the gateway-isolated broker topology merged through PRs #220-#221. | Supplies runtime-boundary evidence to #223/#72/#66; it is not a supported production package by itself. |
| `#223` | Implemented and closed Production checkpoint. | Merged through PR #224 on 2026-08-14. | Durable threat model, responsibility baseline, composed static contract, negative-evidence audit, and synchronized security docs. |
| `#225` | Implemented and closed Production deployment checkpoint. | Independent broker-only single-node package and local qualification merged through PR #226 on 2026-08-14. | Target acceptance, Kubernetes/Fargate, HA, and broader production controls remain with #66 and focused owners. |
| `#227` | Closed Production telemetry checkpoint. | Created from #178 after #225 merged and closed through PR #228 on 2026-08-14. | Owns the merged W3C/OpenTelemetry gateway-to-broker browser runtime lifecycle trace checkpoint. |
| `#229` | Closed Production telemetry checkpoint. | Created from #178 after #227 merged and closed through PR #230 on 2026-08-16. | Owns shared label-free workflow/recording operations counters and their OpenMetrics qualification. |
| `#231` | Closed Production telemetry checkpoint. | Created from #178 after #229 merged and closed through PR #232 on 2026-08-16. | Owns merged Prometheus recording rules, starter alerts, rule tests, and operator runbooks. |
| closed `#233` | Completed Production telemetry checkpoint. | Merged through PR #234. | Provisioned aggregate Grafana dashboard without final SLO/alert-routing scope. |
| `#240` | Deferred endpoint productization. | Created on GitHub 2026-08-20 when #172 was narrowed. | Owns revisions, callbacks, replay, trace expansion, throttling, and connector compatibility. |
| `#241` | Closed Foundation contributor tooling. | Qualification, proposal, repair, bounded CI convergence, and opt-in merge orchestration merged through PR #243. | The loop may promote exactly one roadmap-prioritized candidate but does not alter product priority or capability maturity. |
| `#244` | Closed Foundation contributor-safety follow-up. | Deterministic free-capacity gate merged through PR #245. | Defaults to 50 GiB, fails closed, and never deletes operator data. |
| `#246` | Closed Foundation contributor-tooling follow-up. | Bounded specification session and structured requirements-PR transition merged through PR #247. | It may resolve evidence-backed issue/plan omissions, but cannot make product decisions, promote issues, or implement product code. |
| `#260` | In-progress Foundation contributor-tooling follow-up. | Adds finite ordered traversal after multiple external gates. | It preserves each blocker, selects only the first eligible documented fallback, and cannot inspect arbitrary backlog work. |

## Docs-To-Issue Context

Open issues are listed as `#N`. The historical closed admin redesign lineage is
shown as `closed #142`.

| Docs file | Primary issue context |
| --- | --- |
| `CURRENT_CONTEXT.md` | current baseline and immediate `#47` -> `#172` -> `#174` sequence, with parallel `#180` and deferred `#240`/`#171` |
| `ADMIN_INTERACTION_REQUIREMENTS.md` | `#20`, `#28`, `#153`, `#156`, closed `#142` |
| `ADMIN_NEW_API_COVERAGE.md` | `#69`, `#70`, `#124`, `#153`, `#158`, `#172`, `#240`, closed `#142` |
| `ADMIN_NEW_IMPLEMENTATION_GUARDRAILS.md` | `#153`, `#163`, closed `#142` |
| `ADMIN_NEW_MANUAL_CHECKPOINTS.md` | `#124`, `#154`-`#163`, closed `#142` |
| `ADMIN_NEW_REQUIREMENTS.md` | `#20`, `#21`, `#47`, `#69`, `#124`, `#153`-`#159`, `#161`, `#163`, `#171`, `#172`, `#240`, closed `#142` |
| `ADMIN_NEW_STATUS.md` | `#153`, `#163`, `#171`, `#172`, `#240`, closed `#142` |
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
| `BPANE-00047_WORKFLOW_PACKAGE_CONTRACT_PLAN.md` | `#47`, first Ready Phase 0 slice |
| `BPANE-00171_WORKFLOW_TEACH_MODE_PLAN.md` | `#171`, deferred Phase N; not a Phase 0 dependency |
| `BPANE-00172_PHASE_0_WORKFLOW_ENDPOINT_PLAN.md` | `#172`, bounded polling endpoint after `#47` |
| `BPANE-00172_BPM_WORKFLOW_ENDPOINT_INTEGRATION_PLAN.md` | historical combined #172/#240 design context; not executable |
| `BPANE-00240_WORKFLOW_ENDPOINT_PRODUCTIZATION_PLAN.md` | `#240`, deferred Production/Enterprise endpoint expansion |
| `BPANE-00241_CODEX_DEVELOPMENT_LOOP_PLAN.md` | `#241`, optional Foundation contributor automation; no product capability claim |
| `BPANE-00244_DEV_LOOP_DISK_GUARD_PLAN.md` | `#244`, local contributor disk-capacity safety; no product capability claim |
| `BPANE-00246_REQUIREMENTS_SPECIFICATION_LOOP_PLAN.md` | `#246`, local contributor requirements-specification cycle; no product capability claim |
| `BPANE-00260_ORDERED_QUALIFICATION_FALLBACK_PLAN.md` | `#260`, finite ordered qualification fallback; no product capability claim |
| `BPANE-00173_DELIVERY_GOVERNANCE_PLAN.md` | `#173` |
| `BPANE-00174_PHASE_0_REFERENCE_WORKFLOW_PLAN.md` | `#174`, after `#47` and `#172`, with `#180` and selected conditional controls; explicitly excludes `#71` and `#171` |
| `BPANE-00180_OPEN_SOURCE_GOVERNANCE_PLAN.md` | `#180`, open-source license, contribution, third-party notice, and IP-governance contract; reviewed approval remains an external gate |
| `DELIVERY_ROADMAP.md` | all open issues organized into Foundation, Pilot Value, Operator Product, Production, Enterprise, and Innovation lanes |
| `CAPABILITY_MATURITY_MATRIX.md` | all product capability owners, especially `#163`, `#171`-`#180`, and `#240` |
| `PRODUCT_PHASES_AND_RELEASE_GATES.md` | `#151`, `#145`-`#150`, `#152`, `#47`, `#172`, `#174`, `#153`-`#163`, `#180`, `#240`, and other Production/Phase N owners |
| `RISK_REGISTER.md` | active and closed risk evidence, especially `#66`, `#72`, `#75`, `#175`-`#180`, and focused checkpoint `#223` |
| `THREAT_MODEL.md` | `#223` current evidence baseline, `#72` broader hardening, and residual owners `#28`, `#66`, `#69`, `#70`, `#73`-`#80`, `#175`-`#178` |
| `PRODUCTION_SECURITY_BASELINE.md` | `#223` application/deployment responsibility baseline, with production packaging owned by `#66` and broader hardening by `#72` |
| `PLAN_TEMPLATE.md` | every focused issue entering Ready or In Progress |
| `adr/` | `#173`, `#174`, `#175`, and `#180` plus related product-boundary issues |
| `DOMAIN_REQUIREMENTS.md` | `#20`, `#21`, `#28`, `#31`, `#47`, `#69`, `#124`, `#154`-`#157`, `#159`, `#161`, `#171`, `#172`, `#240`, closed `#142` |
| `IDENTITY_ACCESS_REQUIREMENTS.md` | `#70`, `#157`, `#172`, `#176`, `#177`, `#240`, closed `#142` |
| `IMPLEMENTATION_WORK_ORDER.md` | detailed issue/topic rationale; `DELIVERY_ROADMAP.md` owns current execution order |
| `OPEN_ISSUES_CONTEXT.md` | all open issues, plus closed `#142` lineage |
| `PROJECT_GOVERNANCE_REQUIREMENTS.md` | `#70`, `#79`, `#80`, `#161`, `#172`, closed `#142` |
| `RESOURCE_LIFECYCLE_REQUIREMENTS.md` | `#21`, `#66`, `#76`, `#80`, `#124`, `#148`, `#159`, `#160`, closed `#142` |
| `OPERATOR_CLI_AND_LOCAL_DIAGNOSTICS.md` | `#162`, canonical CLI support boundary and local troubleshooting sequence |
| `RUNTIME_OPERATOR_REQUIREMENTS.md` | `#66`, `#69`, `#72`, `#74`, `#150`, `#162`, `#167`, `#172`, `#214` |
| `SECURITY_RUNTIME_ROADMAP.md` | `#28`, `#66`, `#72`, `#74`, `#75`, `#145`-`#150`, `#164`-`#170` |
| `VALIDATION_MATRIX.md` | all implementation issues selected from this matrix, especially `#151`, `#152`, focused admin-new issues `#153`-`#163`, Phase 0 `#47`/`#172`/`#174`, deferred `#171`/`#240`, runtime boundary `#214`, and security baseline `#223` |
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
| Pilot Value | closed `#47` and `#172`; `#174` is externally deferred; `#180` is a parallel external-use gate | Select and deliver one real activity after stakeholder decisions. While waiting, follow `#263`-`#268`, then `#124`. |
| Production/Enterprise and Innovation | #233 completed; #175 tracks focused children #263-#268; other broad issues plus `#176`-`#178`, `#240`, and `#171` remain qualified | Reassess after Phase 0 evidence; #240 and #171 must not displace the current Pilot/fallback sequence. |

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
- `#20`, `#21`, `#28`, `#66`, `#69`, `#70`, `#72`, `#73`, `#74`, and
  `#75` remain valid but broad. Prefer the matching focused issue from
  `#145`-`#180` for implementation PRs when the slice is covered there.
- `#172` is the canonical Phase 0 polling Workflow Endpoint contract. It must
  not
  absorb workflow authoring/publishing from `#47`, direct session automation
  from `#69`, API key lifecycle from `#70`, generalized event export from
  `#28`, deployment/security/residency/DLP from `#66`, `#72`, `#76`, and `#80`,
  general HA/readiness/policy/CLI/scalability ownership from `#74`, `#79`,
  `#150`, `#161`, `#162`, and `#164`, Phase 0 delivery from `#174`, generalized
  organization grants from `#176`, API conformance from `#179`, Teach Mode from
  `#171`, or endpoint lifecycle/callback/connector expansion from `#240`.
- Closed `#52` remains historical evidence for the implemented identity/access
  review and service-principal registry slices. Remaining organization/grant
  enforcement is owned by `#176`; provisioning/deprovisioning and break-glass
  lifecycle are owned by `#177`.
- `#173` owns delivery governance only. It must not become a catch-all product
  epic.
