# BrowserPane Risk Register

Governance issue: [#173](https://github.com/ITmedes/browserpane/issues/173)

Last reviewed: 2026-08-13

## Scale

- Likelihood: Low, Medium, High.
- Impact: Moderate, High, Critical.
- State: Open, Mitigating, Accepted with expiry, Closed.

## Active Risks

| ID | Risk | Likelihood | Impact | Owner | Mitigation / exit evidence | Gate |
| --- | --- | --- | --- | --- | --- | --- |
| R-001 | Changes can merge without required automated status checks. | High | Critical | #151 | Enforced CI, branch checks, controlled failure fixtures. | Foundation |
| R-002 | Open critical/high dependency advisories affect build or runtime dependencies. | High | Critical | #151 / #75 | Upgrade, reachability review, bounded exception with owner/expiry, recurring scan. | Foundation |
| R-003 | Different token purposes share signing material and credentials appear in URL/query paths. | High | Critical | #145 | Purpose-separated credentials, constant-time/library verification, redaction tests. | Foundation |
| R-004 | Shared admin authentication/browser security is insufficient for promotion. | High | Critical | #146 / #72 | CSP/origin/auth lifecycle/CSRF/frame policies and negative tests. | Foundation |
| R-005 | Webhook delivery can reach unsafe destinations or redirect targets. | Low | Critical | #147 | Mitigated and merged through PR #191: URL/DNS/IP validation, pinned delivery, no redirects/proxies, exact-origin exceptions, negative and compose tests. | Foundation |
| R-006 | Public OpenAPI can drift from code or break clients silently. | Low | High | #179 | Review-ready on `feature/BPANE-00179`: 131-operation inventory, pinned lint, executable examples, Axum route coverage, semantic diff, policy, CI stages, and full fast/Compose validation. Close after merge and required-check confirmation. | Foundation |
| R-007 | Protocol version is not enforced and compatibility is undocumented. | Medium | Critical | #175 | Version negotiation, shared vectors, fuzzing, compatibility matrix. | Production |
| R-008 | In-memory and Postgres behavior can diverge. | Medium | High | #152 | Shared store contract suite and compose verification. | Foundation |
| R-010 | Platform failures and saturation are not observable through standard telemetry/SLOs. | High | High | #178 | OTel/metrics contract, alerts, runbooks, load evidence. | Phase 1 / Production |
| R-011 | Organization/project mappings are descriptive rather than a complete enforced authorization model. | High | Critical | #176 | Role/grant matrix, enforcement, migration, denial/audit tests. | Phase N |
| R-012 | Deprovisioned identities or emergency access lack deterministic lifecycle controls. | Medium | Critical | #177 | Lifecycle API/SCIM path, stale review, break-glass controls. | Phase N |
| R-013 | Compose prototype is described beyond its supported deployment/capacity envelope. | High | High | #66 / #150 / #178 | #150 adds dependency readiness and bounded gateway drain; named deployment profiles, load evidence, telemetry, and explicit supported limits remain. | Phase 0 / Production |
| R-014 | AGPL root license conflicts with package metadata and contributor/IP policy is undefined. | High | Critical | #180 | Reviewed license decision, aligned metadata, contribution/security policy. | Phase 0 / Production |
| R-015 | Investor/product claims drift ahead of implementation maturity. | Medium | High | #173 and investment claim register | Claim maturity/evidence links and publication review. | Every external release |
| R-016 | A Phase 0 Pilot expands into an undefined enterprise platform commitment. | High | High | #174 | Qualification, explicit non-goals, bounded agreement, Stop/Operate/Phase 1 gate. | Phase 0 |
| R-017 | Large overlapping planning documents produce conflicting priorities. | High | High | #173 | Canonical roadmap; historical docs marked supporting; only bounded Ready plans are executable. | Governance |
| R-018 | Workflow/recording workers lack the same unit-test floor as core packages. | Low | High | #165 / #151 | Review-ready on `feature/BPANE-00165-worker-runtime-hardening`: 8 recording-worker and 11 workflow-worker tests, enforced package checks, finite request deadlines, bounded process output, single-flight polling evidence, and compose worker journeys. Close after merge and required-check confirmation. | Foundation / Phase 1 |

## Closed Risks

| ID | Risk | Closure evidence |
| --- | --- | --- |
| R-009 | Recording finalization could cross an unsafe filesystem/artifact boundary. | Closed by #149 through merged PR #212: purpose-scoped worker capability, exact regular-file staging contract, measured-byte accounting, negative API/store coverage, and real worker/admin/CLI compose evidence. |

## Review Rules

- Review this register whenever a slice enters Ready and before every gate.
- A Critical risk cannot be silently accepted. Record owner, rationale, scope,
  expiry, and compensating controls.
- Close a risk only with linked implementation and validation evidence.
- Add newly discovered Pilot risks to the product owner issue; do not hide them
  only in customer-specific runbooks.
- Reflect external-claim risks in the investment claim/evidence register.
