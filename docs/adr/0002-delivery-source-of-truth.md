# ADR 0002: Delivery Source Of Truth

Status: Accepted

Date: 2026-07-31

Related issue: #173

## Context

BrowserPane accumulated detailed work orders, review reconciliations, domain
requirements, admin plans, GitHub issues, and investor plans. Each contains
useful context, but several documents have acted as overlapping priority lists.

## Decision

- GitHub issues own live execution state, owner, priority, dependencies, and
  target milestone. A GitHub Project supersedes issue-label state once one is
  configured.
- `DELIVERY_ROADMAP.md` owns the cross-lane sequence and next Ready slice.
- `CAPABILITY_MATURITY_MATRIX.md` owns product maturity claims.
- `PRODUCT_PHASES_AND_RELEASE_GATES.md` owns promotion evidence.
- `RISK_REGISTER.md` owns active risk state.
- Domain requirements own target behavior.
- A bounded implementation plan is executable only for a Ready or In Progress
  slice. Higher-level feature or qualification plans remain specifications.
- Historical work orders and audit documents remain reference evidence.

## Consequences

- Status is not copied into every document.
- A feature-level specification such as #171/#172 must create a smaller plan
  for each implementation PR.
- Every externally visible claim is traceable to maturity evidence or a
  canonical issue.
- The roadmap can change sequencing without rewriting requirement history.

## Alternatives Considered

- Keep one very large numbered work order: rejected because Pilot,
  Productization, Production, and Enterprise work do not form one strict line.
- Use only GitHub issues: rejected because architecture decisions, evidence,
  and phase gates need durable reviewable context in the repository.
