# BrowserPane Delivery Roadmap

Status: Canonical execution roadmap

Governance issue: [#173](https://github.com/ITmedes/browserpane/issues/173)

Last requirements audit: 2026-08-22 after protocol slices #263 through #265
merged and #273 entered implementation to require exact-head Compose evidence
before Codex automatic merge. The remaining focused #175 protocol issues are
`#266` through `#268`, based on `main` at `6dd4838c`. The Workflow
Endpoint has passing focused, contract, and
real-Compose fake-BPM evidence. #174 remains the next Pilot Value outcome but
waits for real candidate and stakeholder decisions. #180 is specified but
waits for reviewed legal/business decisions. #175 is a blocked non-executable
tracker; #263 is the first engineering qualification candidate.

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
or capacity decision. The optional `dev_loop/` qualifier may execute that gate
for exactly one documented next slice when its dependencies, focused plan,
risks, acceptance criteria, and test evidence are already complete; it may not
choose a different lane or infer a missing decision. If a candidate is
externally gated, the loop may traverse only the finite ordered fallback queue
named by this roadmap and must stop at the first eligible candidate. If current
evidence can resolve concrete
issue/plan omissions without changing product direction, the loop may create
one documentation-only specification PR under #246/#257; the issue remains
Qualified until a later post-merge audit passes.

## Delivery Lanes

| Lane | Outcome | Current entry point | Promotion target |
| --- | --- | --- | --- |
| Foundation | Trusted build, auth, contracts, storage, and lifecycle baseline. | #273 exact-head Compose gate in progress after #235 reliability | Foundation Gate |
| Pilot Value | One bounded reusable BPM browser activity with accepted evidence and runbook. | #172 merged; #174 awaits real candidate selection | Phase 0 Gate |
| Operator Product | Complete and promote `/admin-new/` as the default operator console. | Default promoted through #163; #124 is the next focused catalog gap | Phase 1 Gate |
| Production | Harden deployment, security, recovery, supply chain, and telemetry. | #233 dashboard baseline merged; #178 retains broader telemetry scope | Production Baseline |
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
| 7 | #241 | Done | #173 delivery governance | Bounded Codex qualification/proposal/repair delivery loop merged through PR #243. |
| 8 | #244 | Done | #241 local delivery loop | 50 GiB default repository-filesystem capacity guard merged through PR #245. |
| 9 | #246 | Done | #241 qualification loop | Requirements gaps route through a separate documentation PR before later requalification; merged through PR #247. |
| 10 | #248 | Done | #246 structured qualification | Optional successful-qualification rationale accepted through PR #249. |
| 11 | #251 | Done | #241 merge convergence | Review-required and branch-policy states stop cleanly through PR #252. |
| 12 | #253 | Done | #251 merge gates | Explicit admin merge remains opt-in and constrained; merged through PR #254. |
| 13 | #255 | Done | #172 main smoke evidence | Admin promotion-gate smokes stabilized through PR #256. |
| 14 | #257 | Done | #246 requirements loop | Defer external decisions and evaluate only documented parallel/fallback requirements work; merged through PR #258. |
| 15 | #260 | Done | #257 deferred-candidate contract | Finite roadmap-owned fallback traversal merged through PR #261. |
| 16 | #273 | In Progress | #241 loop and Compose manual dispatch | Require exact-head Compose before automatic merge so failures remain repairable on the open PR. |

#241, #244, #246, #248, #251, #253, #255, #257, #260, and #273 are contributor
tooling or validation reliability and do not supersede the Pilot sequence.
#172 is complete. #174 and #180 remain externally gated.

### Next Three Pilot Value Slices

| Order | Issue | State | Dependency | Outcome |
| --- | --- | --- | --- | --- |
| 1 | #47 | Done | Foundation and workflow baseline merged | Supported immutable Git-backed Playwright workflow package and regression contract merged through PR #242. |
| 2 | #172 | Done | #47 contract complete | Project-scoped, service-principal-authorized polling endpoint merged through PR #250 with real-Compose conformance evidence. |
| 3 | #174 | Qualified / externally deferred | #47 and #172 complete; real candidate selection, #180 external-use gate, and selected conditional controls remain | Select, deliver, operate, and review one real bounded activity without inferring stakeholder decisions. |

#180 is a parallel P0 Foundation/governance gate and must complete before an
external Pilot relies on the repository's open-source posture. Its engineering
contract is specified through PR #259, but implementation waits for the
reviewed legal/business decision.

### Ordered Qualification Fallback Queue

When no Ready work exists, use this finite order:

`#174` -> `#180` -> `#263` -> `#264` -> `#265` -> `#266` -> `#267` -> `#268` -> `#124`

The qualifier must preserve the external blockers on `#174` and `#180`.
Protocol slices `#263` through `#265` are complete, so `#266` is the next
engineering fallback after the bounded #273 contributor-loop correction.
Issues `#263` through `#268` are independently shippable protocol slices with
focused plans; each successor depends on closure of its predecessor. `#175`
retains the complete contract as a blocked program tracker and is never a
direct implementation target. `#124` is the final current fallback and may be
considered only if the protocol chain is blocked, exhausted, or complete. This
queue does not make protocol work or `#124` an external-Pilot dependency and
does not authorize any other backlog issue. If all candidates are blocked, the
loop stops with the complete chain.

### Current Production Slice

| Order | Issue | State | Dependency | Outcome |
| --- | --- | --- | --- | --- |
| 1 | #223 | Done | #214 runtime boundary and #178 metrics checkpoint | Threat model, responsibility baseline, executable security contract, and negative-evidence inventory merged through PR #224. |
| 2 | #225 | Done | #223 evidence baseline | Independent, broker-only single-node Compose profile merged through PR #226. |
| 3 | #227 | Done | #178 metrics checkpoint, #214, #225 | W3C/OpenTelemetry trace propagation for gateway-to-broker browser runtime operations merged through PR #228. |
| 4 | #229 | Done | #178 metrics checkpoint, #227 | Shared label-free workflow, event-delivery, recording, playback, and retention OpenMetrics counters merged through PR #230. |
| 5 | #231 | Done | #178 metrics checkpoint, #229 | Validated Prometheus recording rules, conservative starter alerts, and operator runbooks merged through PR #232. |
| 6 | #233 | Done | #178 metrics checkpoint, #231 | Provisioned Grafana operations dashboard merged through PR #234. |
| 7 | #235 | Done | Existing Compose validation contract | Deterministic observer fixtures, admin evidence alignment, cleanup, and hosted runner reliability merged through PR #236. |

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
and gateway routing merged incrementally through PRs #220 and #221. The first
bounded #178 gateway OpenMetrics/runtime-capacity checkpoint merged through PR
#222; #178 remains open for broader runtime and worker/store tracing,
subsystem metrics beyond #229, calibrated SLOs, dashboards, synthetics, alert
routing, and load evidence. #223 merged
through PR #224 and now links the
implemented controls and negative evidence into one threat model and executable
deployment-security baseline. #225 applies that baseline to an independent
single-node Compose package without absorbing Kubernetes, Fargate, HA, or
compliance scope. It merged through PR #226. #227 merged through PR #228 as a
bounded gateway-to-broker browser lifecycle trace checkpoint. #229 merged
through PR #230 and adds existing workflow/recording operations counters to the
shared scrape. #231 merged through PR #232 and turns shipped metrics into tested
Prometheus indicators, starter alerts, and runbooks. #233 merged through PR
#234 and provisions an aggregate operations dashboard without absorbing final
SLOs, alert routing, synthetics, or load scope. #235/PR #236 then restored
deterministic hosted Compose validation for all five promotion lanes.
#180 remains a governance decision rather than an implicit engineering license
change.

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
| P0-1 | #47 | Freeze the supported immutable Git-backed Playwright workflow package, resource, credential, and regression contract. |
| P0-2 | #172 | Add the stable project-scoped polling endpoint, machine grant, schemas, idempotency, typed outcomes, cancellation, and side-effect certainty. |
| P0-3 | #174 | Qualify candidates, freeze one agreement, deliver the workflow, operate it, and record the exit decision. |
| P0-4 | #180 | Resolve license/package/contribution inconsistency before external Pilot reliance. |
| P0-C | #21 / #66 / selected #20, #72, #73, #178 gaps | Pull in only artifact, deployment, inspection, security, recovery, or telemetry work required by the selected process. |

Phase 0 explicitly excludes BrowserPane subprocesses, Human Handoff, Teach
Mode, workflow training/generation, and automatic repair. A challenge returns
terminal `external_intervention_required` to the external BPM. #240 owns later
endpoint revisions, callbacks, replay, tracing expansion, throttling, and
connector compatibility.

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
- #178 platform telemetry foundation is implemented; #227 adds the merged
  gateway-to-broker browser lifecycle trace checkpoint, #229 adds merged
  workflow/recording subsystem counters, #231 adds the merged initial
  Prometheus SLI/alert/runbook baseline, and #233 adds the merged aggregate
  operations-dashboard checkpoint, while broader traces, calibrated SLOs,
  synthetics, alert routing, and capacity evidence remain under #178,
- #223 evidence-linked threat model and hardening baseline merged through PR
  #224,
- #72 remains the broader enterprise security-hardening owner after #223,
- #225 is the merged bounded single-node Compose package under #66,
- #227 is merged as the gateway-to-broker OpenTelemetry checkpoint under #178,
- #66 retains Kubernetes/Fargate packaging and cross-target deployment work,
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

#240 provides later production endpoint lifecycle, callback, and connector
semantics after the bounded #172 polling contract.
#171 Teach Mode follows stable publishing and endpoint semantics by default.
#263-#268 productize the BrowserPane protocol under tracker #175 independently
of either feature and must be complete before broad compatibility claims.

## Issue And Plan Rules

- One canonical issue owns each shippable outcome.
- Broad issues may remain roadmap owners; focused issues own implementation
  slices and link back to them.
- Do not create bounded implementation plans for all Backlog issues. Create or
  update one when a focused slice moves to Ready.
- Feature specifications such as #171 and #240 must produce a smaller
  slice-specific plan before each implementation PR. #172 now has the bounded
  Phase 0 plan `BPANE-00172_PHASE_0_WORKFLOW_ENDPOINT_PLAN.md`.
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
