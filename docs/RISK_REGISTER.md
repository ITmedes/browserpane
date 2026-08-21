# BrowserPane Risk Register

Governance issue: [#173](https://github.com/ITmedes/browserpane/issues/173)

Last reviewed: 2026-08-22

## Scale

- Likelihood: Low, Medium, High.
- Impact: Moderate, High, Critical.
- State: Open, Mitigating, Accepted with expiry, Closed.

## Active Risks

| ID | Risk | Likelihood | Impact | Owner | Mitigation / exit evidence | Gate |
| --- | --- | --- | --- | --- | --- | --- |
| R-002 | Open critical/high dependency advisories affect build or runtime dependencies. | High | Critical | #151 / #75 | Upgrade, reachability review, bounded exception with owner/expiry, recurring scan. | Foundation |
| R-004 | Application auth/header controls exist, but a deployment could still expose admin, MCP, metrics, or internal surfaces without the required TLS, origin, ingress, and network policy. | Medium | Critical | #223 / #72 / #66 | Executable application/header baseline plus target-specific ingress, exact-origin, private-network, and MCP transport controls. | Production |
| R-005 | Webhook delivery can reach unsafe destinations or redirect targets. | Low | Critical | #147 | Mitigated and merged through PR #191: URL/DNS/IP validation, pinned delivery, no redirects/proxies, exact-origin exceptions, negative and compose tests. | Foundation |
| R-007 | Protocol version enforcement is incomplete across both peers and runtime compatibility is not qualified. | Medium | Critical | #175 | #263 publishes the normative v1 contract, #264 adds bounded byte-identical Rust/TypeScript codecs and a 66-case corpus, and #265 enforces authenticated gateway negotiation before runtime/hub effects with typed isolation and explicit legacy overlap. Browser enforcement, fuzzing, and the real rolling matrix remain #266-#268. | Production |
| R-010 | Platform failures and saturation are not yet observable end to end through standard telemetry/SLOs. | High | High | #178 | Gateway OpenMetrics RED/runtime capacity merged through PR #222, bounded W3C/OTLP gateway-to-broker traces through PR #228, workflow/recording counters through PR #230, Prometheus rules/alerts/runbooks through PR #232, and the Grafana dashboard through PR #234. Broader worker/store coverage, queue/state and latency signals, calibrated SLOs/error budgets, synthetics, alert routing, and load evidence remain. | Phase 1 / Production |
| R-011 | Organization/project mappings are descriptive rather than a complete enforced authorization model. | High | Critical | #176 | Role/grant matrix, enforcement, migration, denial/audit tests. | Phase N |
| R-012 | Deprovisioned identities or emergency access lack deterministic lifecycle controls. | Medium | Critical | #177 | Lifecycle API/SCIM path, stale review, break-glass controls. | Phase N |
| R-013 | Local, broker-validation, or bounded single-node evidence is described beyond its supported deployment/capacity envelope. | Medium | High | #66 / #178 / #225 | Profiles are explicitly classified; the single-node package adds immutable inputs, preflight, live qualification, and a runbook. Target sandbox/network acceptance, real restore drills, load envelopes, HA, and managed-runtime support remain. | Phase 0 / Production |
| R-014 | AGPL root license conflicts with package metadata and contributor/IP policy is undefined. | High | Critical | #180 | Reviewed license decision, aligned metadata, contribution/security policy. | Phase 0 / Production |
| R-015 | Investor/product claims drift ahead of implementation maturity. | Medium | High | #173 and investment claim register | Claim maturity/evidence links and publication review. | Every external release |
| R-016 | A Phase 0 Pilot expands into an undefined enterprise platform commitment. | High | High | #174 | Qualification, explicit non-goals, bounded agreement, Stop/Operate/Phase 1 gate. | Phase 0 |
| R-017 | Large overlapping planning documents produce conflicting priorities. | High | High | #173 | Canonical roadmap; historical docs marked supporting; only bounded Ready plans are executable. | Governance |
| R-020 | Security controls and deployment obligations can drift across code, manifests, tests, and prose, causing an unsafe topology to appear accepted. | Medium | Critical | #223 / #72 / #225 | Canonical threat model, responsibility baseline, composed fast-profile contract, single-node structured preflight, negative evidence, operator runbook, and required review on boundary changes. | Production |
| R-021 | An external BPM could duplicate or misinterpret browser side effects because no stable endpoint grant, schema enforcement, caller-scoped idempotency, typed outcome, or side-effect certainty contract exists. | High | Critical | #47 / #172 / #174 | Freeze the immutable Playwright package, implement the bounded polling endpoint and negative conformance fixture, then qualify one real activity. | Phase 0 |

## Closed Risks

| ID | Risk | Closure evidence |
| --- | --- | --- |
| R-001 | Changes could merge without required automated status checks. | Closed by #151: strict required GitHub checks, pinned validation workflows, dependency/document/coverage contracts, and controlled failure evidence. |
| R-003 | Different token purposes shared signing material and credentials appeared in URL/query paths. | Closed by #145: purpose-separated credentials, wrong-purpose/expiry tests, constant-time/library verification, and log/URL redaction. |
| R-006 | Public OpenAPI could drift from code or break clients silently. | Closed by #179 through PR #194: pinned lint, 131-operation inventory, executable examples, Axum route recognition, semantic compatibility policy, and CI enforcement. |
| R-008 | In-memory and Postgres behavior could diverge. | Closed by #152 through PR #193: one shared store contract runs against both implementations and in Compose validation. |
| R-009 | Recording finalization could cross an unsafe filesystem/artifact boundary. | Closed by #149 through merged PR #212: purpose-scoped worker capability, exact regular-file staging contract, measured-byte accounting, negative API/store coverage, and real worker/admin/CLI compose evidence. |
| R-018 | Workflow/recording workers lacked the same unit-test floor as core packages. | Closed by #165 through PR #213: package test floors, finite request deadlines, bounded output, single-flight finalization polling, and compose worker journeys. |
| R-019 | A compromised gateway could use permitted Docker container/volume APIs with unsafe request fields. | Closed by #167 and #214: the production-like broker topology removes gateway Docker reachability and routes typed browser, worker, context, and session-data operations through broker-owned policy. Static/live isolation, denial, storage, restart, compose API, admin, MCP, workflow, and recording evidence passed. The direct proxy topology remains explicitly local compatibility only. |

## Review Rules

- Review this register whenever a slice enters Ready and before every gate.
- A Critical risk cannot be silently accepted. Record owner, rationale, scope,
  expiry, and compensating controls.
- Close a risk only with linked implementation and validation evidence.
- Add newly discovered Pilot risks to the product owner issue; do not hide them
  only in customer-specific runbooks.
- Reflect external-claim risks in the investment claim/evidence register.
