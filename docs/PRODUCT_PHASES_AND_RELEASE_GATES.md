# BrowserPane Product Phases And Release Gates

Governance issue: [#173](https://github.com/ITmedes/browserpane/issues/173)

## Purpose

Define what BrowserPane means by Foundation, Phase 0, Phase 1, Production
Baseline, and Phase N. These are evidence gates, not calendar promises. Passing
one gate does not imply that every later capability exists.

## Gate 0: Foundation

Outcome: changes can be developed and reviewed on a trusted technical baseline.

Required evidence:

- critical/high dependency findings are remediated or have bounded reviewed
  exceptions,
- required CI checks cover Rust, Node, admin-new, workers, contracts, and docs,
- coverage baselines and regression rules are recorded,
- token-purpose and admin-auth trust boundaries are hardened,
- webhook/callback SSRF controls exist before production callback use,
- dependency-aware readiness and bounded shutdown behavior exist,
- in-memory and Postgres behavior is contract-tested,
- OpenAPI lint and compatibility checks protect the public contract.

Primary owners: #151, #145, #146, #147, #150, #152, #179.

Permitted claim after passing: `Foundation controls validated`.

Not permitted: `Production-ready`, `enterprise-ready`, or unsupported scale.

## Gate 1: Phase 0 Operational Proof

Outcome: one selected browser-only workflow delivers accepted value in one
bounded environment with explicit operational responsibility.

Required evidence:

- process qualification selected one candidate and documented alternatives,
- one manually authored or engineering-assisted immutable Playwright workflow
  package is reviewed and source-pinned,
- inputs, outputs, side effects, credentials, files, retention, timeout,
  cancellation, and external-intervention behavior are frozen,
- an authorized external service principal can invoke one stable project
  endpoint, poll it, and reconcile typed terminal outcomes,
- input is validated before runtime creation and output before success,
- idempotency and browser-side-effect certainty prevent unsafe duplicate work,
- happy path and agreed failures are reproducible,
- operators can start, monitor, intervene, stop, and recover using a runbook,
- terminal state and agreed artifacts/evidence are explainable,
- the deployment and tested capacity envelope are explicit,
- the review records Stop, bounded Operate, or a separately scoped Phase 1.

Primary owners: #47, #172, and #174. #180 is an external-Pilot governance
gate. Conditional owners include #21, #66, and process-selected inspection,
security, recovery, and telemetry gaps.

Permitted claim: `Pilot-ready for the named workflow and environment`.

Not permitted: generic SLA, broad multi-tenancy, HA, or platform-scale claim.
BrowserPane subprocesses, Human Handoff, Teach Mode, training, generation, and
automatic repair are not Phase 0 capabilities.

## Gate 2: Phase 1 Operate

Outcome: the proven workflow is supportable and repeatable for an agreed scope.

Required evidence:

- admin-new has the operator paths required by the operating model,
- identity, policy, run detail, artifacts, and intervention are supportable,
- incident, upgrade, rollback, backup, and change-management procedures exist,
- telemetry and SLO proposals are validated against observed operation,
- deployment and support ownership are contractual and technically testable,
- productization gaps from Phase 0 are accepted, fixed, or explicitly deferred.

Primary owners: #153-#163, #178, #73, #75, #166, plus gaps selected by #174.

Permitted claim: `Operational for the agreed Phase 1 scope`.

## Gate 3: Production Baseline

Outcome: BrowserPane has a documented support, security, compatibility,
observability, recovery, and release contract for named deployment profiles.

Required evidence:

- threat model and hardening baseline are reviewed,
- signed/provenanced releases and component compatibility are defined,
- supported deployment profiles, upgrades, rollback, backup, and DR are tested,
- SLOs, alerts, runbooks, and capacity envelopes are published,
- API/protocol compatibility and deprecation policy are enforced,
- artifact, audit, retention, and security-event behavior is complete for the
  supported scope,
- open-source license and contribution governance is consistent.

Primary owners: #72-#75, #66, #70, #178-#180, #21, #28, #175.

Permitted claim: `Production-ready for named profiles and limits`.

## Gate 4: Phase N Scale

Outcome: capabilities proven in Phase 1 are generalized for multiple teams,
projects, integration systems, and governed deployment environments.

Required evidence:

- organization/project authorization and identity lifecycle are enforced,
- stable Workflow Endpoints and connector compatibility are productized,
- policy, DLP, residency/encryption, HA, and scale requirements are selected by
  validated demand,
- reusable capabilities have explicit isolation, quota, billing/usage,
  support, and compatibility models,
- Teach Mode or controlled repair, when selected, publishes only reviewed
  immutable workflow versions.

Primary owners: #237, #176, #177, #76, #79, #80, #74, and #171. The bounded
#172 polling endpoint is the Phase 0 foundation consumed by #237.

Permitted claim: only the exact Phase N capabilities whose gates passed.

## Gate Decision Record

Every gate review records:

- date and reviewed commit/release,
- named scope and deployment profile,
- evidence links,
- accepted risks and expiry dates,
- unresolved blockers and owners,
- decision: Reject, Rework, Pass for bounded scope, or Superseded,
- next gate or deliberate stop.

No gate can be passed solely by updating this document or closing issues.
