# BPANE-00172 BPM Workflow Endpoint Integration Plan

> Historical combined specification, retained for design evidence. Do not use
> this file as an executable implementation plan. On 2026-08-20 the capability
> was split into the bounded Phase 0 polling contract in
> `BPANE-00172_PHASE_0_WORKFLOW_ENDPOINT_PLAN.md` and the deferred production
> expansion in `BPANE-00237_WORKFLOW_ENDPOINT_PRODUCTIZATION_PLAN.md`.

Issue: `#172` Add project-scoped workflow endpoints for BPM and orchestration
integrations.

Status: Superseded as an execution plan; preserved as source context.

Lane: Pilot Value P0, then Production/Enterprise P1-P2

Target gates: Phase 0 for bounded polling P0; Production/Phase N for P1-P2

This document specifies the complete capability. Before each implementation
PR, create a smaller slice-specific `*_PLAN.md` from `PLAN_TEMPLATE.md` for the
selected P0/P1/P2 slice.

## Purpose

BrowserPane should be usable as a governed browser-automation activity inside
an external business process. A BPM, workflow, iPaaS, or durable-execution
system should be able to invoke one approved browser workflow through a stable
integration endpoint, monitor it, handle Human Handoff, and consume typed
results without depending on:

- an interactive BrowserPane user token,
- owner-internal workflow ids and version selection,
- BrowserPane-specific log parsing,
- binary files embedded in process variables,
- or a promise that BrowserPane itself will become the process orchestrator.

The missing product boundary is a project-scoped Workflow Endpoint/Deployment
resource. It binds an approved immutable workflow version to authorized
callers, schemas, runtime limits, result handling, and lifecycle callbacks.

## Example Use Case

A business process reaches the activity `Retrieve supplier compliance report`.
The process engine invokes the stable endpoint key
`supplier-compliance-report` with:

- its process-instance and activity/job references,
- reporting period input,
- an idempotency key stable across delivery retries,
- a W3C Trace Context,
- and a completion deadline.

BrowserPane authenticates the process engine's service principal, verifies its
project and endpoint grant, validates the request, and creates one browser
workflow run. The process engine receives an asynchronous run resource and
either polls it or consumes signed lifecycle callbacks.

Expected paths:

- success returns bounded structured output and an authorized artifact
  reference for the downloaded report,
- MFA or consent moves the run to `awaiting_input` so the process can create a
  Human Task,
- a portal outage returns a stable retryable technical failure,
- invalid business input returns a stable non-retryable validation or business
  failure,
- repeated delivery with the same idempotency key returns the original run
  without repeating browser side effects.

## Current Implementation Audit

The audit was performed against:

- `openapi/bpane-control-v1.yaml`,
- gateway workflow API and store code,
- workflow worker types and lifecycle,
- current Admin-New requirements and status,
- the operator CLI and README workflow documentation.

### Implemented Foundation

| Area | Current implementation |
| --- | --- |
| Workflow definitions | Owner-scoped definitions and immutable versions |
| Source | Git-backed, resolved-commit pinning and per-run source snapshots |
| Runs | Persisted run resource bound to a session and automation task |
| Project governance | Project-scoped runs, quotas, admission, and queue visibility |
| Correlation | Optional `source_system`, `source_reference`, and `client_request_id` |
| Idempotency | Owner-scoped request fingerprint and original-run return |
| State | `pending`, `queued`, `starting`, `running`, `awaiting_input`, and terminal states |
| Control | Cancel, submit input, resume, and reject |
| Evidence | Logs, events, produced files, recordings, and source/workspace references |
| Human interaction | Durable intervention state and runtime hold/release |
| Event delivery | Signed HTTP callbacks with retry/backoff and persisted delivery attempts |
| State-event persistence | Postgres run creation/state transitions persist the run event and enqueue matching deliveries in the same transaction |
| Authentication | OIDC bearer validation plus local HMAC development mode |
| Identity metadata | Service-principal registry, allowed-project metadata, and identity mappings |
| UI | Workflow definition/detail/launch and run overview in Admin-New |
| CLI | Owner-token workflow definition/run operations |

### Material Gaps

| Gap | Current consequence | Required direction |
| --- | --- | --- |
| No Workflow Endpoint resource | Callers must know owner-internal workflow id and version | Stable project-scoped endpoint key bound to an approved version |
| No caller grant | A machine token does not gain scoped access to an owner's workflow | Enforced service-principal endpoint/project grants |
| Owner identity is the storage boundary | Client-credentials tokens naturally resolve to their own subject | Resolve machine caller separately from owning project/endpoint |
| Schemas are descriptive only | Invalid input can create a session/worker; invalid output can be marked successful | Publish-time schema checks and run-time input/output validation |
| Failure is mostly a string | BPM callers cannot distinguish business and technical handling | Typed outcome with stable code/category/retryability |
| `timed_out` is not a complete policy | No caller deadline, queue deadline, or gateway execution watchdog | Enforced deadline and timeout contract |
| No run heartbeat/progress contract | An external orchestrator cannot distinguish slow work from a dead worker | Bounded progress and stale-worker detection |
| Cancellation is not fully acknowledged | The caller cannot see requested vs. worker-acknowledged cancellation | Explicit cancellation lifecycle |
| No distributed trace propagation | Process, gateway, worker, callbacks, and artifacts form separate evidence chains | W3C Trace Context plus BrowserPane request id |
| Proprietary unversioned callback envelope | Receivers need BrowserPane-specific code and cannot negotiate evolution | Versioned CloudEvents-compatible envelope and AsyncAPI |
| No callback replay controls | Failed terminal delivery can only be inspected | Cursor/replay, manual redelivery, secret rotation, dead-letter state |
| Unbounded lists | Runs, events, logs, and deliveries do not scale as integration history grows | Stable filters and cursor pagination |
| Result boundary is implicit | Process variables may receive too much or too little data | Explicit bounded JSON vs. artifact-reference contract |
| No deployment revision/environment contract | A stable endpoint cannot be promoted safely through test and production or rolled back predictably | Immutable endpoint revisions, environment binding, compatibility checks, and audited promote/rollback |
| No integration completion profiles | Every process engine must invent polling/callback behavior and long HTTP calls can outlive connector limits | Canonical asynchronous run plus documented poll, webhook, and callback-token adapters |
| No caller-level overload contract | Project queueing exists, but callers cannot plan for endpoint throttling or service degradation | Endpoint/caller quotas, `Retry-After`, load-shedding, and readiness/maintenance semantics |
| No attempt/side-effect evidence | A retryable failure does not tell an orchestrator whether external browser side effects may already have happened | Attempt identity, semantic checkpoints, side-effect uncertainty, and compensation guidance |
| No connector compatibility profiles | One canonical OpenAPI document does not fit every process platform's import/auth limitations | Canonical contract plus generated, tested compatibility exports and platform-neutral conformance fixtures |
| Connector and browser credentials are not clearly separated | Process variables or endpoint config can become an accidental secret transport | Explicit integration-credential, target-system Credential Binding, and variable-mapping boundaries |
| State-event recovery contract is incomplete | Postgres enqueue is transactional, but callers have no per-run sequence, cursor/replay, or formal reconciliation contract | Preserve the transactional foundation and add public sequence, replay, reconciliation, and store-parity guarantees |
| Human Handoff ownership is ambiguous | Both systems may create competing tasks or expose unsafe resume links | Explicit external-managed and BrowserPane-managed handoff profiles |
| Private connectivity and data handling profile is incomplete | Enterprise process systems may not reach the endpoint safely or may receive excess evidence | Deployment-owned private ingress, endpoint data classification, callback allowlist, and retention/redaction policy |
| No connector conformance package | Each process-system integration can interpret the API differently | Generated discovery plus reference wrappers and fake orchestrator |

## Product Boundary

BrowserPane owns:

- the browser workflow endpoint,
- machine authorization for that endpoint,
- browser-native admission and execution,
- run state and browser evidence,
- Human Handoff around the browser activity,
- typed output and artifact references,
- idempotent invocation,
- and lifecycle callback delivery.

The external process system owns:

- schedules and timers across business steps,
- BPMN/DAG/process state,
- broad retry policy,
- cross-system compensation,
- business-level escalation,
- and process-instance retention.

BrowserPane reports whether a failure is retryable and may suggest a safe retry
delay. It does not silently retry the complete browser workflow because browser
actions can have external side effects.

## Delivery Priorities

The complete issue is intentionally split so the first release remains a
shippable integration boundary.

### P0: Minimum Governed Workflow Endpoint

- project-scoped endpoint bound to an immutable workflow version,
- explicit service-principal grant,
- asynchronous create/get/cancel contract,
- endpoint/caller-scoped idempotency,
- enforced input schema,
- typed terminal outcome,
- bounded inline result and artifact references,
- stable polling example and deterministic conformance smoke.

### P1: Production Integration Semantics

- queue/execution/Human Handoff deadlines,
- progress/heartbeat and cancellation acknowledgement,
- W3C Trace Context,
- versioned signed callbacks with retry/redelivery,
- cursor pagination,
- caller-level throttling and overload signaling,
- output validation and side-effect/attempt evidence,
- endpoint revision promotion, rollback, and compatibility checks,
- process-variable/credential separation, public state-event recovery and
  sequencing, and explicit Human Handoff ownership.

### P2: Connector Ecosystem

- delegated-user compatibility where required,
- callback-token adapters,
- generated connector compatibility profiles,
- Admin-New and CLI completeness,
- additional reference wrappers after the generic contract passes conformance.

P0 must not claim production readiness without the P1 security, timeout,
overload, and callback work. It is nevertheless independently useful for a
bounded Pilot whose process system polls the run resource.

## Target Resource Model

### Workflow Endpoint

Recommended resource fields:

- `id`,
- `project_id`,
- stable `endpoint_key`,
- `name`,
- `description`,
- environment or stage binding,
- `state`: `draft`, `active`, `disabled`, or `deprecated`,
- immutable endpoint revision and compatibility version,
- `workflow_definition_id`,
- active immutable `workflow_definition_version_id`,
- contract version,
- input and output schema references/digests,
- maximum inline input/output bytes,
- allowed result/artifact classes,
- data-classification and retention-policy references,
- queue and execution timeout limits,
- caller concurrency/rate limits,
- supported controls,
- supported completion and connector compatibility profiles,
- lifecycle callback contract version,
- labels,
- creator/updater actor metadata,
- created/updated/activated/deprecated timestamps.

The endpoint key is stable while an authorized operator promotes a newer
approved immutable workflow version behind it. Promotion must be audited and
must not mutate historical runs. A promotion creates a new endpoint revision,
runs contract-compatibility checks, and supports an audited rollback to an
earlier compatible revision. Environment promotion must reference immutable
revisions rather than copying mutable draft state.

### Endpoint Invocation Grant

Recommended grant fields:

- endpoint id,
- registered service-principal id,
- project id,
- scopes,
- state,
- optional expiration,
- optional source-system restriction,
- creator actor,
- created/updated/last-used timestamps.

Initial scopes:

- `workflow:endpoint:invoke`,
- `workflow:run:read`,
- `workflow:run:cancel`,
- `workflow:run:intervene`,
- `workflow:artifact:read`.

An endpoint grant is authorization metadata. It must not contain a client
secret or bearer token.

### External Invocation

Recommended route:

`POST /api/v1/workflow-endpoints/{endpoint_key}/runs`

Recommended request data:

- contract version,
- input,
- idempotency key,
- source system,
- process-instance reference,
- activity/job reference,
- optional tenant reference within configured policy,
- requested deadline,
- optional callback subscription reference,
- bounded correlation metadata.

W3C `traceparent` and `tracestate` remain HTTP headers. Secrets and arbitrary
token claims must not be copied into run labels.

The response should identify:

- the created or previously existing run,
- whether the call was an idempotent replay,
- current state and admission,
- status, events, cancel, intervention, and artifact links,
- effective deadline,
- request/correlation id,
- contract version.

Use an asynchronous accepted-resource pattern for the external endpoint. Freeze
the final `201`/`202`, `Location`, and idempotent replay semantics in OpenAPI
contract tests before implementation. Do not change the existing owner-facing
`POST /api/v1/workflow-runs` behavior implicitly.

## Integration Completion Profiles

The canonical contract is always the asynchronous BrowserPane run resource.
Adapters may expose three process-engine integration patterns:

1. **Poll:** the process system invokes once and polls the run with bounded
   backoff, conditional requests, and a terminal-state deadline.
2. **Webhook:** BrowserPane sends signed at-least-once lifecycle events to a
   registered subscription while the process system can still reconcile by
   polling.
3. **Callback token:** a connector stores an upstream opaque task/job token
   outside ordinary run labels and reports terminal success, failure, or
   heartbeat back to the process engine.

BrowserPane must not keep an inbound HTTP request open for the complete browser
run as the primary integration model. Connector and gateway timeouts vary too
widely, and Human Handoff can make runs long-lived.

Callback tokens and upstream connector secrets are sensitive credentials. They
must use a dedicated encrypted reference or Credential Binding boundary, be
redacted from logs/events/UI, and expire with the upstream activity.

## Contract Requirements

### Idempotency

- Scope keys by endpoint and authenticated caller, not only owner.
- Set a documented maximum key length and retention period.
- Fingerprint the complete side-effect-relevant invocation.
- Return the original run for an identical retry.
- Return a stable conflict problem for the same key with a different request.
- Keep the mapping long enough to cover upstream retry and callback recovery.
- State clearly that HTTP delivery is at least once and browser side effects are
  protected through invocation idempotency, not an exactly-once claim.

### Input And Output

- Support JSON Schema Draft 2020-12 as the initial dialect.
- Validate the schema itself before endpoint activation.
- Resolve or reject external schema references deterministically.
- Validate input before creating a session, automation task, or worker.
- Validate output before transitioning the run to `succeeded`.
- Return bounded validation details with JSON Pointer locations.
- Version schemas with the endpoint contract.
- Preserve the exact schema and digest used by every historical run.

The endpoint contract accepts only the declared input object, bounded
correlation metadata, and explicit artifact/workspace references. A connector
maps process variables into that object and maps bounded output back into the
process. BrowserPane must not ingest the complete process context by default.

Keep two credential domains separate:

- the integration credential authenticates the BPM connector to BrowserPane,
- a BrowserPane Credential Binding supplies target-website secrets to the
  browser workflow.

Neither credential belongs in process variables, endpoint schemas, run labels,
logs, events, callback bodies, or artifact metadata. The external process
system's secret store owns its BrowserPane connector credential; BrowserPane's
credential provider owns target-system secrets.

### HTTP And Domain Failures

Use RFC 9457 Problem Details for request-level HTTP failures. Include stable
extensions such as:

- BrowserPane error code,
- category,
- request id,
- retryable,
- optional retry-after seconds,
- validation errors with JSON Pointer,
- related resource link where safe.

Use a domain-specific run outcome for accepted executions:

- `success`,
- `validation_error`,
- `policy_denied`,
- `business_error`,
- `technical_error`,
- `cancelled`,
- `queue_timed_out`,
- `execution_timed_out`,
- `human_action_required`.

The outcome should contain:

- stable `type` and `code`,
- category,
- safe human-readable message,
- bounded details,
- retryable flag,
- optional retry delay,
- cause/event id,
- originating component,
- timestamp.

Do not require callers to parse a free-form `error` string.

### Deadlines And Progress

Separate:

- queue deadline,
- execution deadline,
- Human Handoff deadline,
- artifact retention deadline.

The endpoint defines maximums; a caller may request a shorter deadline. The
gateway, not only the worker, must enforce effective deadlines.

Progress should be bounded and semantic:

- phase/step name or stable step id,
- optional percentage only when meaningful,
- last heartbeat,
- safe summary,
- attempt/worker identity,
- optional resumable checkpoint reference.

Never stream unrestricted DOM, credentials, or page bodies as progress.

### Cancellation

Expose:

- `cancel_requested_at`,
- request actor,
- worker acknowledgement,
- runtime/session release state,
- final cancellation result,
- any explicitly documented non-cancellable section.

Cancellation should be idempotent. A terminal run remains terminal, and
repeated cancellation must not produce ambiguous failures.

### Human Handoff Ownership

Support two explicit profiles:

- **External-managed:** BrowserPane emits a typed `human_action_required`
  intervention resource. The external process creates and owns the Human Task,
  then calls the scoped intervention action.
- **BrowserPane-managed:** BrowserPane owns the operator task and provides a
  short-lived, audience-bound handoff entry point. The external process waits
  for the run lifecycle result.

Every endpoint revision selects one profile. A run must not create both tasks.
The intervention contract defines actor scope, expiry, claim/complete
semantics, duplicate completion behavior, cancellation, and audit linkage.
This contract reuses the security and private-fallback work in `#71`.

### Attempts, Browser Side Effects, And Compensation

Each execution attempt should expose:

- stable attempt id and ordinal,
- worker/session identity,
- start/end timestamps,
- last durable semantic checkpoint,
- whether an external side effect is known not to have happened, confirmed to
  have happened, or remains uncertain,
- optional business correlation captured by the workflow,
- retry-safety classification and operator guidance.

The workflow package may declare named idempotency or verification steps, but
BrowserPane must not infer that a clicked submit button is safe to repeat.
Compensation remains in the external process model. BrowserPane provides enough
evidence for that model to choose retry, verify, compensate, or escalate.

### Capacity, Rate Limits, And Availability

- Apply endpoint- and caller-level concurrency and request-rate policy in
  addition to existing project admission.
- Return stable `429`/`503` Problem Details and `Retry-After` where applicable.
- Distinguish accepted queueing from rejected overload and from dependency
  unavailability.
- Expose whether invocation is available, degraded, in maintenance, or blocked
  by a required dependency.
- Keep idempotent retries valid across transient gateway failures.
- Publish supportable limits for request/result size, run duration, callback
  throughput, and retained history.
- Integrate gateway readiness from `#150`; an HTTP listener alone is not proof
  that Postgres, runtime dispatch, worker images, credential providers, and
  artifact storage can accept a run.

### Correlation And Tracing

- Accept and validate W3C `traceparent` and optional `tracestate`.
- Generate a BrowserPane request id when none is supplied.
- Persist trace/correlation identifiers on the run.
- Propagate them to the worker using explicit environment or request fields.
- Include them in structured logs, events, artifact provenance, and callback
  deliveries.
- Do not place personal, secret, or business payload data in trace headers.
- Define which parts survive retention cleanup.

### Lifecycle Events And Callbacks

Adopt a versioned CloudEvents-compatible envelope. At minimum expose:

- specification version,
- stable event id,
- event type,
- source,
- subject/run id,
- event time,
- data content type and schema,
- run/endpoint/project correlation,
- trace context where permitted,
- event-specific data.

Publish an AsyncAPI document for callback delivery while retaining OpenAPI for
control-plane HTTP routes.

Preserve the existing Postgres behavior that persists a run transition, its
event, and matching delivery records in one transaction. Formalize the same
observable guarantee across supported stores. Events need a monotonic per-run
sequence in addition to a globally unique id. A receiver that misses or
reorders delivery must be able to reconcile from the run/event API and request
redelivery within retention.

Callback semantics must define:

- at-least-once delivery,
- ordering only within an explicitly documented boundary,
- receiver idempotency by event/delivery id,
- retryable HTTP statuses,
- exponential backoff and maximum attempts,
- signature timestamp tolerance and replay protection,
- signing-secret rotation,
- subscription update/disable,
- failed/dead-letter state,
- manual redelivery,
- retention and cursor pagination.

Webhook target validation, DNS resolution, redirect handling, and allowlisting
remain blocked on `#147`.

### Results And Artifacts

Inline results are only bounded JSON suitable for process variables. Large or
binary outputs use an artifact reference with:

- artifact id and class,
- media type,
- byte size,
- checksum,
- provenance,
- created and expiry timestamps,
- retention state,
- authorized content link.

Expired content must be distinguishable from missing and unauthorized content.
Recordings, screenshots, raw browser evidence, and downloaded files must not be
embedded in callback payloads or process variables.

## Private Connectivity And Data Governance

- Treat private ingress, private DNS, load balancer/API gateway, source
  allowlisting, and optional mTLS as deployment profiles owned with `#66` and
  security issue `#72`.
- Keep browser egress configuration separate from control-plane ingress.
- Bind endpoint and callback data classification, field allowlists, redaction,
  retention, residency, and encryption policy to project governance.
- Do not copy workflow input/output into general audit logs.
- Keep callback payloads minimal; consumers fetch authorized details and
  artifacts when needed.
- Reuse `#70` for immutable audit/retention controls, `#76` for residency/BYOK,
  and `#80` for DLP/content inspection rather than implementing parallel policy
  systems in `#172`.

## Endpoint Revision And Environment Promotion

- Keep draft configuration separate from immutable active revisions.
- Validate workflow version, schemas, grants, runtime requirements, callback
  targets, and project policy before activation.
- Provide a no-side-effect contract test using deterministic fixtures.
- Classify a revision as backward compatible, conditionally compatible, or
  breaking based on input/output/error/control changes.
- Require an explicit contract-version change for breaking revisions.
- Support audited promotion and rollback without changing the endpoint key.
- Keep dev/test/prod bindings explicit. Do not let a test endpoint silently
  consume production credentials, projects, file workspaces, or callback
  subscriptions.
- Retain the exact endpoint revision on every run and delivery.

## Connector Discovery And Compatibility

The canonical BrowserPane API should remain one versioned OpenAPI contract, but
connector imports need generated compatibility artifacts:

- OpenAPI 3.1 as the canonical endpoint/control description,
- an OpenAPI 2.0 compatibility export when a target platform requires it,
- flattened/bounded schemas where a connector importer has documented limits,
- auth profiles for client credentials, delegated OAuth, and governed API keys,
- an operation manifest that marks asynchronous behavior, polling links,
  callbacks, errors, and artifact outputs,
- generated examples and a conformance report tied to the endpoint contract
  version.

Compatibility exports are generated from the canonical contract and tested for
drift. They must not become independent handwritten APIs.

## Authentication Compatibility

Primary production path:

- external OIDC/OAuth 2.0 issuer,
- confidential client using client credentials,
- registered BrowserPane service principal,
- explicit endpoint/project grant and scopes.

The gateway must validate issuer, audience, authorized party/client id,
expiration/not-before, signature, and endpoint scopes. Where supported, prefer
private-key or workload-identity client authentication over another static
shared secret. Mutual TLS and workload identity are later compatibility
profiles, not reasons to weaken bearer-token validation.

Compatibility path:

- user-delegated OIDC for connector platforms that only support an interactive
  authorization-code connection,
- scoped BrowserPane-issued API keys only after the lifecycle, audit, rotation,
  and revocation contract in `#70` exists.

Do not add Basic Authentication as the default integration model. Do not
repurpose session automation access tokens as long-lived BPM credentials.

## Admin-New Requirements

Recommended routes:

- `/admin-new/workflow-endpoints`,
- `/admin-new/workflow-endpoints/new`,
- `/admin-new/workflow-endpoints/[endpoint_id]`,
- `/admin-new/workflow-endpoints/[endpoint_id]/runs`,
- `/admin-new/workflow-endpoints/[endpoint_id]/deliveries`.

The catalog should show:

- endpoint key and state,
- project,
- active workflow/version,
- contract/schema version,
- allowed caller count,
- recent run health,
- deprecation status.

The detail view should provide:

- immutable version binding and controlled promotion,
- environment, active revision, compatibility assessment, and rollback,
- input/output schema preview,
- caller grants and scopes,
- deadlines/result limits and caller throttling,
- polling/webhook/callback-token completion profiles,
- invocation examples,
- callback/subscription health,
- correlated recent runs,
- attempt/checkpoint and side-effect uncertainty evidence,
- readiness, degraded/maintenance, and admission diagnostics,
- audit evidence,
- disable/deprecate controls.

The UI must not expose client secrets, signing secrets, raw bearer tokens, or
unbounded callback payloads.

## CLI Requirements

Recommended commands:

- `workflow endpoint create`,
- `workflow endpoint list`,
- `workflow endpoint get`,
- `workflow endpoint update`,
- `workflow endpoint activate`,
- `workflow endpoint disable`,
- `workflow endpoint promote`,
- `workflow endpoint rollback`,
- `workflow endpoint validate`,
- `workflow endpoint test`,
- `workflow endpoint export-connector`,
- `workflow endpoint grant`,
- `workflow endpoint revoke-grant`,
- `workflow endpoint invoke`,
- `workflow endpoint runs`,
- `workflow endpoint deliveries`,
- `workflow endpoint redeliver`.

CLI JSON output must remain stable. Human summary output must be opt-in. Secret
material must never be printed.

## Implementation Slices

Each slice ends with an independently testable state.

### Slice 0: Freeze The Contract

Deliver:

- resource and authorization decision records,
- OpenAPI route/schema draft,
- AsyncAPI callback draft,
- lifecycle and failure state diagrams,
- completion-profile, idempotency, timeout, cancellation, overload, attempt,
  promotion, compatibility, and pagination decisions,
- deterministic fake-orchestrator fixture.

Manual checkpoint:

- review generated API examples and state transitions without starting a real
  browser.

### Slice 1: Endpoint Registry And Grants

Deliver:

- Postgres and in-memory endpoint/grant store contract,
- CRUD and lifecycle APIs,
- immutable workflow-version binding,
- immutable endpoint revisions and environment binding,
- project and service-principal validation,
- activate/disable/deprecate, promotion, rollback, and compatibility behavior.

Manual checkpoint:

- create an endpoint, grant a service principal, activate it, promote a version,
  and verify denied cross-project/disabled-principal cases.

### Slice 2: External Invocation And M2M Authorization

Deliver:

- endpoint invocation route,
- service-principal token-to-grant authorization,
- endpoint/caller-scoped idempotency,
- source/process/activity correlation,
- accepted-resource links,
- caller rate limits and overload errors,
- filters and cursor pagination for endpoint runs.

Manual checkpoint:

- invoke with a client-credentials token, repeat the request, and verify one run
  and one browser side-effect path.

### Slice 3: Schemas And Typed Outcomes

Deliver:

- schema dialect declaration and schema validation,
- pre-side-effect input validation,
- pre-success output validation,
- RFC 9457 request failures,
- typed terminal run outcomes and retryability.

Manual checkpoint:

- exercise valid input, invalid input, invalid output, business error, retryable
  technical error, and permanent technical error.

### Slice 4: Deadlines, Progress, And Cancellation

Deliver:

- queue/execution/Human Handoff deadline calculation,
- gateway watchdog enforcement,
- worker heartbeat/progress endpoint,
- stale-worker handling,
- cancellation requested/acknowledged/terminal state,
- attempt/checkpoint and side-effect uncertainty evidence.

Manual checkpoint:

- run deterministic slow, stalled, queued, cancelled, and Human Handoff
  fixtures.

### Slice 5: Traceable Lifecycle Delivery

Deliver:

- W3C Trace Context propagation,
- BrowserPane request id,
- versioned CloudEvents-compatible payload,
- AsyncAPI contract,
- subscription update/disable and secret rotation,
- cursor/replay/manual redelivery/dead-letter handling,
- `#147` destination controls.

Manual checkpoint:

- drop a callback, inspect retry/dead-letter state, redeliver it, and correlate
  process request, run, worker log, artifact, and callback.

### Slice 6: Result And Artifact Contract

Deliver:

- inline JSON size enforcement,
- typed artifact references,
- checksums and media types,
- authorization and expiry behavior,
- callback redaction and bounded payload enforcement.

Manual checkpoint:

- return one inline result and one large file, then verify checksum, download,
  expiry, and absence of binary content in process variables.

### Slice 7: Discovery, Admin-New, CLI, And Reference Adapters

Deliver:

- endpoint discovery representation,
- generated connector compatibility profiles,
- Admin-New catalog/detail and promotion/grant UX,
- CLI parity,
- generic OpenAPI/cURL sample,
- one reference BPM connector,
- one durable-activity wrapper,
- one callback-token adapter,
- conformance documentation.

Manual checkpoint:

- execute the same endpoint through raw API, CLI, Admin-New, reference
  connector, and durable-activity wrapper.

### Slice 8: Production Validation

Deliver:

- unit coverage for every policy/state transition,
- in-memory/Postgres store parity,
- API/OpenAPI contract tests,
- worker integration tests,
- callback receiver and outage tests,
- state-transition/delivery atomicity and store-parity tests plus event
  sequencing, replay, and reconciliation tests,
- compose smoke,
- Admin-New E2E,
- CLI smoke,
- fake-orchestrator conformance suite,
- retention and restart recovery validation,
- documentation and operational runbook updates.

## Test Strategy

### Unit

- endpoint/grant validation,
- scope and project authorization,
- idempotency fingerprint and conflict behavior,
- schema compilation and validation output,
- failure classification and retryability,
- deadline calculation and state transitions,
- heartbeat staleness,
- cancellation state machine,
- trace-header validation,
- CloudEvents payload and HMAC signature,
- pagination cursors,
- artifact result limits.

### Integration

- in-memory and Postgres endpoint/grant store contract,
- external service-principal invocation,
- project isolation,
- endpoint version promotion,
- endpoint revision validation, environment promotion, and rollback,
- duplicate invocation under concurrent delivery,
- endpoint/caller throttling, overload, and `Retry-After`,
- gateway-worker progress and cancellation,
- attempt/checkpoint and side-effect uncertainty,
- schema rejection before side effects,
- webhook retry/replay/rotation,
- restart recovery,
- retention and expired idempotency/event records.

### Smoke And E2E

- local Keycloak service account,
- deterministic browser workflow fixture,
- fake process engine/receiver,
- polling, webhook, and callback-token completion profiles,
- external-managed and BrowserPane-managed Human Handoff profiles,
- connector/browser credential separation and process-variable minimization,
- Admin-New endpoint management,
- CLI/raw API parity,
- Human Handoff,
- artifact output,
- denied authorization and invalid schema/error cases.

## Post-Implementation Smoke Sequence

1. Start local compose and create a project, immutable workflow version, active
   service principal, and Workflow Endpoint.
2. Request a client-credentials token and invoke the endpoint with valid input,
   trace context, deadline, source correlation, and idempotency key.
3. Verify accepted-resource links, project/caller authorization, input
   validation, run creation, worker start, trace continuity, and typed success.
4. Repeat the identical invocation and confirm the original run is returned
   with no duplicate browser action.
5. Reuse the key with changed input and confirm a bounded conflict.
6. Send invalid input and confirm RFC 9457 plus JSON Pointer details before any
   session or worker is created.
7. Return invalid worker output and confirm the run cannot reach `succeeded`.
8. Exercise business error, retryable technical failure, permanent technical
   failure, queue timeout, execution timeout, and cancellation acknowledgement.
9. Pause for Human Handoff, submit input with the required scope, and deny the
   same action without that scope. Verify the selected ownership profile creates
   exactly one Human Task.
10. Drop the first callback, verify signed retry and receiver deduplication,
    then manually redeliver within retention.
11. Verify event contract, trace propagation, pagination, ordering boundaries,
    and redaction.
12. Produce a large file, retrieve it through the authorized artifact link,
    verify metadata/checksum/expiry, and confirm it is absent from inline output.
13. Promote a second immutable workflow version behind the endpoint key and
    verify compatibility checks, immutable revision history, and unchanged
    caller integration; roll back and verify new runs use the prior revision.
14. Disable the endpoint or service principal and confirm new invocation is
    denied while retained evidence follows policy.
15. Saturate endpoint/caller capacity and confirm queued acceptance is distinct
    from `429`/`503`, includes bounded `Retry-After`, and remains idempotent.
16. Exercise polling, webhook, and callback-token adapters and confirm upstream
    credentials are absent from run labels, logs, events, and UI.
17. Force a dispatch failure after the transactional state/event/delivery
    commit; verify reconciliation emits one sequenced logical event and the
    receiver deduplicates any delivery retry.
18. Run raw API, CLI, Admin-New, canonical and compatibility contract exports,
    reference connector, durable wrapper, and fake-orchestrator conformance
    suites.

## Dependencies And Issue Ownership

- `#151`: enforced CI, dependency safety, and validation baseline.
- `#145` / `#146`: credential-purpose and admin/browser auth foundations.
- `#47`: workflow source, immutable versions, publishing, packaging, and
  executor strategy.
- `#66`: deployment profiles and private connectivity.
- `#70`: BrowserPane-issued API key/service-credential lifecycle, immutable
  audit, and retention policy.
- `#72`: enterprise security baseline and endpoint threat model.
- `#76`: data residency, encryption, and BYOK.
- `#80`: DLP and content inspection.
- `#28`: generalized resource events and security-event export.
- `#69`: direct session automation connection descriptors.
- `#71`: Human Handoff, challenge detection, and private fallback.
- `#74`: high availability and zero-downtime operation.
- `#79`: central policy evaluation beyond the narrow endpoint grant.
- `#150`: dependency-aware readiness for accepting new work.
- `#161`: project governance evidence and policy UX.
- `#162`: operator CLI parity and diagnostics.
- `#164`: catalog pagination and scale.
- `#147`: webhook SSRF, redirect, DNS, and allowlist hardening.
- `#174`: process qualification and bounded Phase 0 reference-workflow
  delivery. It decides whether #172 P0 is required by the selected Pilot.
- `#176`: generalized organization/project authorization and enforced
  service-principal grants consumed by production endpoint access.
- `#178`: platform SLO, telemetry, and capacity evidence for production
  endpoint operation.
- `#179`: canonical OpenAPI conformance, compatibility, and generated contract
  governance.
- `#171`: Teach Mode candidate generation and immutable workflow publication.
- `#172`: Workflow Endpoint resource, endpoint grants, and BPM-facing run
  contract.

Implement `#172` before `#171` by default. A bounded Phase 0 may implement #172
P0 after the minimum Foundation dependencies selected by #174 pass; it must not
describe that polling contract as production-ready before P1 security,
lifecycle, observability, and compatibility semantics are complete.

## Non-Goals

- BPMN/DAG engine implementation,
- schedules and cross-system orchestration,
- exactly-once delivery claims,
- automatic whole-workflow retries,
- implementing compensation logic in BrowserPane,
- a long-lived synchronous HTTP request as the primary completion model,
- storing connector or target-system credentials in process variables,
- creating both an external and BrowserPane Human Task for one intervention,
- anonymous public endpoints,
- embedding binary data in process variables,
- implementing every vendor-specific connector,
- replacing the owner workflow API in the first migration,
- autonomous workflow mutation through Teach Mode.

## Exit Criteria

The slice is complete when:

- a machine caller can invoke a project-scoped stable endpoint without an
  interactive owner token,
- endpoint authorization, schema validation, idempotency, deadlines,
  cancellation, outcomes, callbacks, and artifacts are frozen and tested,
- external callers can change neither the workflow version nor project scope,
- an operator can promote a new immutable version without changing the endpoint
  key and can roll back through retained endpoint revisions,
- polling, webhook, and callback-token adapters preserve the same run and
  outcome semantics,
- accepted queueing, caller throttling, dependency unavailability, and
  maintenance are machine-distinguishable,
- attempt/checkpoint and side-effect uncertainty are visible enough for the
  external process to choose retry, verification, compensation, or escalation,
- the API, AsyncAPI, CLI, Admin-New, and reference adapters expose the same
  contract,
- generated connector compatibility exports are proven not to drift from the
  canonical contract,
- production-negative cases are covered by deterministic tests,
- and README, architecture, OpenAPI, issue, and operator documentation match
  the implementation.
