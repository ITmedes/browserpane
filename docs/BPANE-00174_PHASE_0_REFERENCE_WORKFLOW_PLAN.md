# BPANE-00174 Phase 0 Reference Workflow Plan

Issue: [#174](https://github.com/ITmedes/browserpane/issues/174)

Status: Qualified; not yet selected for implementation

Lane: Pilot Value

Target gate: Phase 0 Operational Proof

Depends on: #151 plus the minimum applicable controls selected from #145,
#146, #147, and #150; conditional #172, #149, #71, #66, #154

Last reviewed: 2026-07-31

## Business Outcome

Select and deliver one browser-only process whose value, risk, integration
boundary, operating model, and exit can be evaluated without committing to an
enterprise-wide BrowserPane platform rollout.

## Example Use Case

A business process repeatedly requires an operator to log into a third-party
portal, locate a case, upload or retrieve a document, record the resulting
status, and hand control to a human when authentication or a material exception
requires judgment. The surrounding BPM system remains responsible for the
overall business process. BrowserPane owns only the governed browser step,
session state, Human Handoff, and agreed evidence.

## Product Boundary

Phase 0 follows this integration order:

1. use a stable native API when it fully covers the process,
2. use an established connector/integration where it meets the controls,
3. use BrowserPane only for the remaining browser-only step.

BrowserPane does not become the enterprise BPM engine, system of record,
medical/legal decision maker, or general AI decision platform. The external
orchestrator owns process scheduling, broad retries, compensation, business
state, and cross-system workflow logic.

## Candidate Qualification

Score each candidate from 1 (poor) to 5 (strong) and record evidence:

| Criterion | Qualification question |
| --- | --- |
| Business value | Does the recurring manual step have material time, quality, or throughput impact? |
| Browser-only fit | Is the required function missing or incomplete in available APIs/connectors? |
| Process stability | Is the happy path understandable and bounded enough for a first proof? |
| Side-effect safety | Can writes/submissions be identified, reviewed, and stopped or compensated? |
| Human Handoff | Are judgment, MFA, CAPTCHA, or sensitive-input boundaries explicit? |
| Data sensitivity | Can identity, credentials, files, recordings, and retention be governed? |
| Vendor-change exposure | Can page changes be detected and handled through a controlled process? |
| Integration fit | Can the leading workflow system invoke and reconcile the browser step? |
| Deployment fit | Is there one feasible environment with accountable operations? |
| Reversibility | Can the proof be stopped without trapping data, source, or process ownership? |

Reject candidates with unresolved legal/contractual automation restrictions,
unbounded high-impact decisions, unknown side effects, or no responsible
process/operator owner.

## Required Phase 0 Agreement

Freeze before implementation:

- process owner, technical owner, operator, security/data owner, and escalation,
- start trigger and external correlation/idempotency key,
- versioned input schema and validation errors,
- expected outputs and artifact/file contract,
- browser context, egress, credentials, extensions, and workspace bindings,
- read-only versus mutating steps and side-effect checkpoints,
- Human Handoff trigger, authorization, expiry, and resume behavior,
- timeout, cancellation, failure, retry, and reconciliation semantics,
- recording/log/event policy, redaction, retention, and download authority,
- target deployment, availability window, capacity limit, and maintenance,
- acceptance evidence and Stop/Operate/Phase 1 review procedure.

## Implementation Slices

### Slice 0: Qualification And Contract Freeze

- score two or three candidates,
- document API/connectors considered,
- select one candidate or deliberately stop,
- produce process contract, threat/data profile, and target deployment,
- map every required platform gap to a canonical issue.

Manual checkpoint: stakeholders approve the bounded process and non-goals before
runtime implementation begins.

### Slice 1: Reference Workflow Package

- create or pin the versioned workflow source,
- define inputs, outputs, credentials, files, egress, context, and capabilities,
- add validation and safe error mapping,
- cover read-only and mutating checkpoints,
- publish only the reviewed immutable version.

Manual checkpoint: run the workflow in a non-production test environment and
review source, configuration, and expected side effects.

### Slice 2: Integration Entry Point

- use #172 P0 when the external system requires a stable endpoint,
- otherwise document the bounded existing workflow-run API integration,
- propagate correlation/idempotency and map terminal states,
- ensure cancellation and status reconciliation are explicit,
- avoid claiming callback or production connector support until its owner ships.

Manual checkpoint: start and reconcile one run entirely from the intended
external integration path.

### Slice 3: Operator And Human Handoff

- provide the minimum route-backed run/session detail required to operate,
- implement or bind the agreed Human Handoff path,
- show validation, policy, waiting, failed, canceled, and artifact states,
- document start, observe, intervene, stop, recover, and escalate procedures.

Manual checkpoint: an operator who did not implement the workflow follows the
runbook successfully.

### Slice 4: Evidence And Retention

- produce the agreed logs, events, files/artifacts, and recording segments,
- make unavailable/expired evidence explicit,
- verify checksums, authorization, redaction, and retention,
- preserve correlation from external invocation to run, session, and artifact.

Manual checkpoint: reviewers trace one successful and one failed run from
invocation through retained evidence.

### Slice 5: Operational Trial And Exit Review

- operate only within the agreed duration/capacity envelope,
- record interventions, failures, vendor changes, and operator effort,
- test stop/recovery and one controlled dependency failure,
- classify discovered gaps as process-specific or reusable product capability,
- decide Stop, bounded Operate, or separately scoped Phase 1.

## Security And Data Impact

- Use external identity and short-lived scoped credentials; never put long-lived
  secrets into source, URLs, logs, or artifacts.
- Bind credentials, egress, files, and artifacts to the selected project.
- Record browser-side effects and uncertainty instead of treating retries as
  inherently safe.
- Keep recordings disabled unless the agreement selects them and #149 safety is
  complete.
- Do not ingest full proxy/decrypted traffic into BrowserPane telemetry.
- Define data subject, retention, deletion, and download authority before using
  real sensitive data.

## Test Strategy

### Unit

- schema validation and error taxonomy,
- input normalization and output bounds,
- side-effect checkpoint and idempotency behavior,
- policy/grant denial,
- timeout/cancel/Handoff state mapping.

### Integration

- external invocation to workflow run/session binding,
- credentials, context, egress, workspace, and artifact boundaries,
- in-memory/Postgres contract where affected,
- recording/artifact finalization when selected,
- callback/polling reconciliation profile selected by the agreement.

### Smoke And E2E

- real supported runtime and target-like portal fixture,
- happy path,
- validation denial,
- authentication/Handoff path,
- target-site/runtime failure,
- timeout/cancel,
- post-side-effect uncertainty,
- evidence download and retention expiry,
- operator stop and recovery.

## Post-Implementation Smoke Sequence

1. Score at least two candidates and verify the selected candidate satisfies the
   documented threshold and exclusion rules.
2. Provision the agreed project, identity, context, egress, workspace,
   credentials, workflow version, and retention settings.
3. Start a run through the agreed external integration entry point using a
   stable correlation/idempotency key.
4. Observe the same bound browser session and complete the happy path.
5. Repeat with invalid input and verify no runtime starts.
6. Exercise Human Handoff or an agreed operator intervention.
7. Exercise timeout/cancel before a side effect and reconciliation after an
   intentionally ambiguous side-effect boundary.
8. Verify terminal state, logs, events, files/artifacts, recording when enabled,
   checksums, redaction, authorization, and retention.
9. Stop/restart one selected dependency and follow the recovery runbook.
10. Record the closing Stop, bounded Operate, or Phase 1 decision and link every
    remaining gap to a canonical issue.

## Exit Criteria

- One workflow has accepted bounded operational evidence.
- No broad Production or Enterprise claim is inferred from the proof.
- Operations and failure ownership are explicit.
- The external system can reconcile terminal and uncertain outcomes.
- The result has a reversible exit and a separately scoped continuation path.
