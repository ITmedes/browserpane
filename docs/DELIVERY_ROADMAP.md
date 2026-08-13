# BrowserPane Delivery Roadmap

Status: Canonical execution roadmap

Governance issue: [#173](https://github.com/ITmedes/browserpane/issues/173)

Last implementation audit: 2026-08-13 on
`feature/BPANE-00167-docker-runtime-boundary` through the complete proxy-boundary
and compose/browser regression checkpoint.

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
| Foundation | Trusted build, auth, contracts, storage, and lifecycle baseline. | Complete through #179 | Foundation Gate |
| Pilot Value | One bounded reference workflow with accepted evidence and runbook. | #174 | Phase 0 Gate |
| Operator Product | Complete and promote `/admin-new/` as the default operator console. | Default promoted through #163; #124 is the next focused catalog gap | Phase 1 Gate |
| Production | Harden deployment, security, recovery, supply chain, and telemetry. | #167 merged; #214 broker topology in completion, then #178 / #72 / #66 | Production Baseline |
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
| 5 | #152 | Done | #150 stable readiness contract | Shared in-memory/Postgres store contract parity merged through PR #193. |
| 6 | #179 | Done | #152 persistent behavior baseline | Control API lint, conformance, examples, compatibility policy, and CI enforcement merged through PR #194. |

### Next Three Product Slices

| Order | Issue | State | Dependency | Outcome |
| --- | --- | --- | --- | --- |
| 1 | #157 | Done | #156 session operations parity | Identity/access review and registry lifecycle merged through PR #199. |
| 2 | #158 | Done | #179 governed contract evidence | API, coverage, and docs companion routes merged through PR #200. |
| 3 | #159 | Done | #153 shared catalog patterns | Extensions, credential bindings, and workflow event-subscription catalogs merged through PR #201. |

#151, #184, and #185 established and accelerated the required validation
baseline. #145 is merged through
`docs/BPANE-00145_TOKEN_DOMAIN_SEPARATION_PLAN.md`; #146 is merged through PR
#190 and documented in `docs/BPANE-00146_ADMIN_AUTH_SECURITY_PLAN.md`. #147
merged through PR #191. #150 merged through PR #192 after passing the canonical
full 46-stage validation profile. #152 merged through PR #193 and the latest
scheduled Compose run on that `main` commit passed. #179 merged through PR #194
after all 40 fast and all 10 Compose validation stages passed. #153 has
merged through PR #195 after all required checks passed. #154 merged through
PR #196, #155 through PR #197, #156 through PR #198, and #157 through PR #199.
#158 merged through PR #200, #159 through PR #201, and #148 through PR #202.
#160 merged through PR #203 after required and manually dispatched Compose
validation passed. #161 merged through PR #204 and #162 through PR #209. #163
closed after its validation contract merged through PR #210 and the unified
admin became the default web-root route through PR #211. #149 merged through
PR #212, and #165 worker-runtime hardening merged through PR #213. #167 merged
through PR #215. #214 broker contracts, policy, browser/worker/storage adapters,
and gateway routing merged incrementally through PR #220; the final isolated
gateway topology is the current completion slice.

## Foundation Gate Sequence

| Sequence | Issue | Required outcome | Notes |
| --- | --- | --- | --- |
| F1 | #151 | Dependency remediation, CI, coverage baseline, required checks. | First product slice. |
| F2 | #145 | Token-purpose separation and URL/log credential cleanup. | Security prerequisite. |
| F3 | #146 | Admin auth and browser security baseline. | Required before promotion/exposure. |
| F4 | #147 | Callback/webhook SSRF controls. | Required before production callbacks. |
| F5 | #150 | Lifecycle, health, dependency readiness, bounded drain. | Required for accepting external work. |
| F6 | #152 | In-memory/Postgres store contract parity. | Required before relying on persistent behavior. |
| F7 | #179 P1 | OpenAPI lint and compatibility/conformance ratchet. | Final unconditional Foundation slice after #152. |

Conditional Foundation work:

- #148 merged through PR #202 and supplies the bounded import contract consumed
  by #160; no Pilot should bypass those archive safety limits.
- #149 merged through PR #212 and supplies the purpose-scoped recording-worker
  capability, exact staging boundary, and measured artifact finalization
  contract required when recordings form part of Pilot acceptance evidence.
- #167 removed the gateway's direct socket mount and added the checked direct
  compatibility proxy. #214 adds the typed launch broker and gateway-isolated
  production-like Docker-host topology.

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

- #167 Docker proxy boundary and #214 policy-validating runtime broker are the
  implemented Docker-host runtime trust baseline,
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
