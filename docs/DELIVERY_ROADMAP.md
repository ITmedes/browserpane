# BrowserPane Delivery Roadmap

Status: Canonical execution roadmap

Governance issue: [#173](https://github.com/ITmedes/browserpane/issues/173)

Last implementation audit: 2026-08-04 on `feature/BPANE-00150`

## How To Use This Document

This file answers what should be worked on next and why. GitHub issues own
scope and execution state. A bounded implementation plan becomes executable
only when its focused slice enters Ready or In Progress. Existing higher-level
feature and qualification `docs/*_PLAN.md` documents remain specifications and
must produce a bounded slice plan before coding. The capability matrix owns
product maturity, and the release-gate document owns promotion criteria.

The following documents are supporting references, not competing execution
queues:

- `IMPLEMENTATION_WORK_ORDER.md`: detailed rationale and topic inventory,
- `OPEN_ISSUES_CONTEXT.md`: issue-to-document map,
- `NEXT_WORKING_ROADMAP.md`: admin-new transition context,
- `REVIEW_FINDINGS_*` and `LEGACY_*`: historical audit evidence,
- domain and admin requirement documents: target behavior by subsystem.

## Delivery States

| State | Meaning |
| --- | --- |
| Backlog | Valid outcome, but not sufficiently qualified for scheduling. |
| Qualified | Scope and owner issue exist; dependencies or decisions remain. |
| Ready | Dependencies are cleared, owner is known, and a bounded `*_PLAN.md` exists. |
| In Progress | One branch/PR is actively implementing the bounded plan. |
| Review | Implementation is complete and gate evidence is under review. |
| Blocked | A named external decision or dependency prevents meaningful progress. |
| Done | Acceptance and evidence are complete; the issue is merged/closed. |

At most one issue is the recommended first Ready implementation slice. Several
lanes may contain Qualified work, but starting them requires an explicit gate
or capacity decision.

## Delivery Lanes

| Lane | Outcome | Current entry point | Promotion target |
| --- | --- | --- | --- |
| Foundation | Trusted build, auth, contracts, storage, and lifecycle baseline. | #152 | Foundation Gate |
| Pilot Value | One bounded reference workflow with accepted evidence and runbook. | #174 | Phase 0 Gate |
| Operator Product | Complete and promote `/admin-new/` as the default operator console. | #153 | Phase 1 Gate |
| Production | Harden deployment, security, recovery, supply chain, and telemetry. | #72 / #66 / #178 | Production Baseline |
| Enterprise | Organization controls, policy, residency, HA, and governed integrations. | #176 / #70 / #79 | Phase N Gate |
| Innovation | Teach Mode and controlled repair after stable execution contracts. | #171 | Phase N capability gate |

## Immediate Sequence

### Current Foundation Slice

| Order | Issue | State | Dependency | Outcome |
| --- | --- | --- | --- | --- |
| 0 | #151 | Done | #173 governance baseline | Required CI, dependency, coverage, and validation checks. |
| 1 | #145 | Done | #151 required checks | Separate credential domains and remove bearer credentials from URLs/logs. |
| 2 | #146 | Done | #145 secure credential contract | Harden shared admin auth, CSP, and browser security. |
| 3 | #147 | Done | #145 credential/log baseline | Callback and webhook SSRF controls merged through PR #191. |
| 4 | #150 | Done | #147 callback boundary | Lifecycle, dependency readiness, and bounded drain merged through PR #192. |
| 5 | #152 | In Progress | #150 stable readiness contract | Shared in-memory/Postgres store contract parity. |

### Next Three Product Slices

| Order | Issue | State | Dependency | Outcome |
| --- | --- | --- | --- | --- |
| 1 | #150 | Done | #147 callback boundary | Lifecycle, dependency readiness, and bounded drain merged through PR #192. |
| 2 | #152 | In Progress | #150 stable readiness contract | Add shared in-memory/Postgres store contract tests. |
| 3 | #179 | Qualified | #152 persistent behavior baseline | Add control API conformance and compatibility governance. |

#151, #184, and #185 established and accelerated the required validation
baseline. #145 is merged through
`docs/BPANE-00145_TOKEN_DOMAIN_SEPARATION_PLAN.md`; #146 is merged through PR
#190 and documented in `docs/BPANE-00146_ADMIN_AUTH_SECURITY_PLAN.md`. #147
merged through PR #191. #150 merged through PR #192 after passing the canonical
full 46-stage validation profile. #152 is now the active Foundation slice.

## Foundation Gate Sequence

| Sequence | Issue | Required outcome | Notes |
| --- | --- | --- | --- |
| F1 | #151 | Dependency remediation, CI, coverage baseline, required checks. | First product slice. |
| F2 | #145 | Token-purpose separation and URL/log credential cleanup. | Security prerequisite. |
| F3 | #146 | Admin auth and browser security baseline. | Required before promotion/exposure. |
| F4 | #147 | Callback/webhook SSRF controls. | Required before production callbacks. |
| F5 | #150 | Lifecycle, health, dependency readiness, bounded drain. | Required for accepting external work. |
| F6 | #152 | In-memory/Postgres store contract parity. | Required before relying on persistent behavior. |
| F7 | #179 P0 | OpenAPI lint and compatibility/conformance ratchet. | May be delivered with or immediately after #151. |

Conditional Foundation work:

- #148 is required before a Pilot imports untrusted browser contexts.
- #149 is required before recordings form part of Pilot acceptance evidence.
- #167 is required before a Docker runtime boundary is promoted as a production
  deployment contract.

## Phase 0 Pilot Value Sequence

Phase 0 does not wait for every admin-new or enterprise issue. It may begin
after the minimum Foundation dependencies selected by its threat/data profile
are complete.

| Sequence | Issue | Required outcome |
| --- | --- | --- |
| P0-1 | #174 | Qualify candidates and freeze one reference-workflow agreement. |
| P0-2 | #172 P0 | Stable project-scoped asynchronous polling endpoint when external invocation is required. |
| P0-3 | #154 | Route-backed run detail sufficient to operate and inspect the Pilot. |
| P0-4 | #149 / #21 | Recording/artifact evidence selected by the agreement. |
| P0-5 | #71 | Signed/private Human Handoff if required by the process. |
| P0-6 | #66 | Bounded target deployment and operator runbook. |

The exit is not automatic expansion. The Phase 0 review chooses Stop, bounded
Operate, or a separately scoped Phase 1.

## Operator Product Sequence

Use the existing focused issues in this order unless Pilot evidence changes the
priority:

1. #153 shared admin-new patterns and API client,
2. #154 workflow-run detail,
3. #155 and #156 session subareas,
4. #157 identity/access review,
5. #158 API/coverage/docs companion,
6. #159 resource catalogs,
7. #160 context lifecycle parity,
8. #161 project governance evidence,
9. #162 operator CLI parity,
10. #163 promotion and fallback gate.

#124 remains the focused session-template catalog owner and should be placed
beside #159 when that resource group is selected.

## Production And Phase N Sequence

Production and enterprise work is gate-driven, not one large precondition for a
bounded Pilot:

- #72 threat model and enterprise hardening baseline,
- #178 platform telemetry, SLOs, alerts, and capacity evidence,
- #66 deployment packaging and validated runtime targets,
- #73 backup/restore and disaster recovery,
- #74 high availability and zero-downtime operations,
- #75 SBOM, signing, provenance, and release governance,
- #180 open-source license/contribution/IP governance,
- #70 credential lifecycle, immutable audit, and retention,
- #176 organization/project authorization and enforced grants,
- #177 provisioning/deprovisioning and break-glass lifecycle,
- #76 residency, encryption, and BYOK,
- #79 central policy engine,
- #80 DLP/content inspection.

#172 P1/P2 provides production integration semantics and connector exports.
#171 Teach Mode follows stable publishing and endpoint semantics by default.
#175 productizes the BrowserPane protocol independently of either feature and
must be complete before broad compatibility claims.

## Issue And Plan Rules

- One canonical issue owns each shippable outcome.
- Broad issues may remain roadmap owners; focused issues own implementation
  slices and link back to them.
- Do not create bounded implementation plans for all Backlog issues. Create or
  update one when a focused slice moves to Ready.
- Feature specifications such as #171 and #172 must produce a smaller
  slice-specific plan before each implementation PR.
- Issue body is canonical for business case, scope, acceptance, and smoke.
- Plan file is canonical for code boundaries, decisions, migration/rollback,
  test decomposition, commits, and implementation evidence.
- Until a GitHub Project is configured, issue labels and milestones are
  canonical for live state, priority, lane, and target gate; this roadmap owns
  dependency order. GitHub Project fields supersede that interim mechanism once
  configured.

## Required GitHub Fields

Current issues use matching labels and milestones. If a GitHub Project is
added, use:

- State: Backlog, Qualified, Ready, In Progress, Review, Blocked, Done,
- Priority: P0 through P4,
- Lane: Foundation, Pilot Value, Operator Product, Production, Enterprise,
  Innovation,
- Target gate: Foundation, Phase 0, Phase 1, Production Baseline, Phase N,
- Owner,
- Depends on,
- Evidence link,
- Target release or milestone.

Recommended milestones:

- Foundation Gate,
- Phase 0 Reference Workflow,
- Admin-New Phase 1 Promotion,
- Production Baseline,
- Phase N Enterprise Controls.

## Change Control

When implementation evidence changes:

1. update the affected issue and plan,
2. update capability maturity and risk state,
3. reassess the target gate,
4. update README/ARCH/OpenAPI when applicable,
5. update investor claim evidence when externally visible,
6. change this roadmap only when sequencing or ownership changes.
