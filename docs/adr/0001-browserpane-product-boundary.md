# ADR 0001: BrowserPane Product Boundary

Status: Accepted

Date: 2026-07-31

Related issues: #47, #71, #171, #172, #174

## Context

BrowserPane can execute versioned workflows, retain browser state, support
Human Handoff, and produce run/session evidence. External BPM, iPaaS, and
durable-execution systems already own broader business-process scheduling,
state, retries, compensation, and cross-system orchestration.

## Decision

BrowserPane is a governed browser execution and session-control endpoint. It
owns the browser-native step, its runtime resources, same-session human/agent
control, and agreed evidence. An external process system remains authoritative
for the end-to-end business process.

BrowserPane follows an API-first decision order: native API, established
integration, then BrowserPane for the browser-only remainder.

Teach Mode may produce reviewable workflow candidates, but publication remains
explicit and immutable. BrowserPane does not autonomously make high-impact
business, medical, legal, or employment decisions.

## Consequences

- #172 exposes stable asynchronous browser-workflow endpoints, not a new BPM
  graph engine.
- Broad retry and compensation remain external; BrowserPane reports attempts,
  checkpoints, and uncertain browser-side effects.
- #171 compiles candidates into the existing workflow publication boundary.
- Phase 0 selects one bounded browser-only process rather than promising a
  generic enterprise platform.
- AI models and process engines remain replaceable integrations.

## Alternatives Considered

- Build a complete BPM/orchestration platform: rejected because it duplicates
  mature systems and expands the trust/operating boundary unnecessarily.
- Expose only raw browser sessions: rejected because workflow systems need
  governed run state, evidence, and intervention semantics.
