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
owns the browser-native activity, its workflow run, normally one browser
session, runtime resources, and agreed evidence. An external process system
remains authoritative for the end-to-end business process.

BrowserPane follows an API-first decision order: native API, established
integration, then BrowserPane for the browser-only remainder.

Later Human Handoff and Teach Mode capabilities may reuse this boundary, but
they are not part of Phase 0. Publication remains explicit and immutable.
BrowserPane does not autonomously make high-impact business, medical, legal,
or employment decisions.

## Consequences

- #172 exposes one bounded polling-based browser-workflow endpoint for Phase 0,
  not a new BPM graph engine.
- Broad retry and compensation remain external; BrowserPane reports attempts,
  checkpoints, and uncertain browser-side effects.
- A Phase 0 challenge terminates as `external_intervention_required`; the
  external process owns any human task.
- #237 owns later endpoint revisions, callbacks, and connector compatibility.
- #171 may later compile candidates into the existing workflow publication
  boundary.
- Phase 0 selects one bounded browser-only process rather than promising a
  generic enterprise platform.
- AI models and process engines remain replaceable integrations.

## Alternatives Considered

- Build a complete BPM/orchestration platform: rejected because it duplicates
  mature systems and expands the trust/operating boundary unnecessarily.
- Expose only raw browser sessions: rejected because workflow systems need
  governed run state, evidence, and intervention semantics.
