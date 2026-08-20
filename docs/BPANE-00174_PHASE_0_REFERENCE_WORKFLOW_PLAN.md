# BPANE-00174 Phase 0 Reference Workflow Plan

Issue: [#174](https://github.com/ITmedes/browserpane/issues/174)

Status: Qualified; candidate process not yet selected

Lane: Pilot Value

Target gate: Phase 0 Operational Proof

Depends on: `#47`, `#172`, and `#180`; conditional `#21`, `#66`, and selected
inspection/security/recovery/telemetry owners

Last reviewed: 2026-08-20

## Business Outcome

Prove that one recurring browser-only activity can be invoked and operated as a
reusable step inside an external business process without committing to a
general enterprise BrowserPane rollout.

## Example Use Case

A process engine invokes `retrieve-supplier-report` with a reporting period and
correlation id. BrowserPane starts one workflow run, opens one isolated browser
session, authenticates through a Credential Binding, downloads the report, and
returns typed JSON plus an authorized artifact reference. A portal challenge
ends with `external_intervention_required`; the external process engine owns
any human task.

## Product Boundary

Use a native API first, an established connector second, and BrowserPane only
for the remaining browser-only activity.

The external BPM owns scheduling, surrounding subprocesses, broad retry,
compensation, human tasks, and business state. BrowserPane owns the approved
workflow, its run, normally one browser session, and agreed evidence.

Phase 0 excludes BrowserPane subprocess orchestration, Human Handoff, Teach
Mode, workflow generation, model training, and autonomous repair. The workflow
is manually or engineering-assisted authored, reviewed, regression-tested, and
published as an immutable Git-backed Playwright version.

## Candidate Qualification

Score at least two candidates from 1 (poor) to 5 (strong) and record evidence:

| Criterion | Qualification question |
| --- | --- |
| Business value | Does the recurring manual step have material time, quality, or throughput impact? |
| API/connector fallback | Is the required function missing or incomplete in available APIs and established integrations? |
| Process stability | Is the happy path bounded enough for a first proof? |
| Side-effect safety | Can writes/submissions be identified, reconciled, and stopped or compensated externally? |
| External intervention | Are MFA, CAPTCHA, consent, sensitive input, and judgment boundaries explicit? |
| Data sensitivity | Can identity, credentials, files, recordings, and retention be governed? |
| Vendor-change exposure | Can page changes be detected and handled through a controlled engineering process? |
| Integration fit | Can the selected BPM invoke and reconcile the browser activity through polling? |
| Deployment fit | Is there one feasible environment with accountable operations? |
| Reversibility | Can the proof stop without trapping data, source, or process ownership? |

Reject candidates with unresolved automation restrictions, unbounded
high-impact decisions, unknown side effects, mandatory BrowserPane handoff, or
no accountable process/operator owner.

## Required Agreement

Freeze before implementation:

- process, technical, operator, security/data, and escalation owners,
- endpoint key, external caller, trigger, correlation, and idempotency,
- versioned input and output schemas,
- bounded JSON and artifact/file results,
- read-only and mutating browser steps,
- context, egress, credentials, extensions, workspace, and recording policy,
- side-effect checkpoints and `none`/`confirmed`/`uncertain` reconciliation,
- challenge mapping to terminal `external_intervention_required`,
- timeout, cancellation, retry guidance, and recovery,
- evidence, redaction, authorization, retention, and download authority,
- deployment, availability window, capacity, maintenance, and exit review.

## Delivery Slices

### 1. Qualification And Contract Freeze

- compare candidates and API/connector alternatives,
- select one or deliberately stop,
- freeze process, data/threat profile, deployment, owners, and non-goals,
- map selected product gaps to canonical issues.

Manual checkpoint: stakeholders approve the bounded activity and non-goals
before runtime implementation begins.

### 2. Reusable Workflow Package

- apply `#47`,
- pin reviewed immutable Playwright source and entrypoint,
- define schemas, credentials, files, egress, context, capabilities, assertions,
  and timeouts,
- add deterministic positive and negative scenarios.

Manual checkpoint: reviewers inspect source, configuration, declared resources,
and expected side effects before publication.

### 3. BPM Polling Endpoint

- apply `#172`,
- bind one stable endpoint key to the approved version,
- authorize the selected service principal,
- enforce schemas, idempotency, typed outcomes, timeout, cancellation, and
  side-effect certainty,
- return bounded JSON and artifact references.

Manual checkpoint: the target-like BPM invokes and reconciles one run without
an interactive BrowserPane owner token.

### 4. Operator Path And Evidence

- configure and inspect through Admin-New,
- expose matching API/CLI/OpenAPI behavior,
- document start, monitor, stop, recover, and escalate,
- preserve BPM-to-endpoint-to-run-to-session-to-artifact correlation.

Manual checkpoint: an operator who did not implement the workflow follows the
runbook and traces successful and failed evidence.

### 5. Bounded Trial And Exit

- operate only within the agreed environment and capacity,
- record failures, external intervention, portal changes, and operator effort,
- test dependency failure, shutdown, and recovery,
- decide Stop, bounded Operate, or separately scoped Phase 1.

## Security And Data Impact

- Use external identity and short-lived scoped credentials; never put long-lived
  secrets into source, URLs, logs, events, or artifacts.
- Bind credentials, egress, contexts, files, recordings, and artifacts to the
  selected project and prove cross-project denial.
- Record browser-side effects and uncertainty instead of treating retries as
  inherently safe.
- Keep recordings disabled unless the agreement selects them; preserve the
  purpose-scoped recording-worker and exact staging boundary.
- Do not ingest full proxy or decrypted traffic into BrowserPane telemetry.
- Define data subject, retention, deletion, artifact download authority, and
  external-intervention ownership before using real sensitive data.
- Reject candidates with unresolved automation restrictions or high-impact
  decisions that cannot remain outside BrowserPane.

## Test Strategy

### Unit

- schema and package validation,
- endpoint/caller authorization and idempotency,
- outcome, timeout, cancellation, and side-effect mapping,
- input normalization, output bounds, and stable validation problems.

### Integration

- in-memory/Postgres behavior,
- external invocation through endpoint, run, and session binding,
- credential, context, egress, workspace, recording, and artifact boundaries,
- terminal result and retained evidence reconciliation.

### Smoke And E2E

- target-like happy path,
- invalid input before runtime creation,
- duplicate and conflicting invocation,
- portal/runtime failure,
- timeout and cancellation,
- challenge to `external_intervention_required`,
- ambiguous post-submit side effect,
- evidence access, retention, shutdown, and recovery.

## Post-Implementation Smoke Sequence

1. Score at least two candidates and record the selection.
2. Provision project, service principal, credentials, context, egress,
   workspace, workflow, endpoint, and retention.
3. Invoke through the fake or target BPM and poll through success.
4. Replay the same idempotency key and verify no duplicate action.
5. Reject changed input under the same key.
6. Reject invalid input before session/worker creation.
7. Exercise runtime failure, timeout, and cancellation.
8. Exercise challenge and uncertain-side-effect outcomes.
9. Verify authorization, JSON, artifacts, logs, redaction, and retention.
10. Follow the runbook through shutdown, dependency restart, and recovery.
11. Record Stop, bounded Operate, or Phase 1.

## Exit Criteria

- One workflow has accepted bounded operational evidence.
- The external BPM can reconcile every terminal outcome without an internal
  BrowserPane human task.
- Operations, evidence, failure ownership, and exit are explicit.
- No Production, Enterprise, Teach Mode, or subprocess claim is inferred.
