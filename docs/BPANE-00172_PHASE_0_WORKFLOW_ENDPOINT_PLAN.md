# BPANE-00172 Phase 0 Workflow Endpoint Plan

Issue: [#172](https://github.com/ITmedes/browserpane/issues/172)

Status: In Progress

Lane: Pilot Value

Target gate: Phase 0 Operational Proof

Last reviewed: 2026-08-21

Verified baseline: `ab9dc5648df01b166d4eccde7a2aeffaa40675b2`

## Business Outcome

Expose one approved BrowserPane workflow as one stable asynchronous activity
that an external BPM can invoke, poll, and cancel without an interactive owner
token or knowledge of internal workflow ids.

## Example Use Case

A process engine invokes `retrieve-supplier-report` with a reporting period,
process correlation, and idempotency key. BrowserPane authenticates the caller,
validates input, runs one immutable workflow in one isolated browser session,
and returns schema-valid JSON plus an authorized report artifact. A portal
challenge returns terminal `external_intervention_required` to the process
engine instead of creating a BrowserPane human task.

## Current Baseline

Implemented:

- immutable Git commit-pinned workflow versions,
- workflow runs, session bindings, admission, cancellation, logs, events, and
  produced files,
- owner-scoped client request idempotency,
- OIDC validation and service-principal registry metadata,
- Admin-New workflow and run inspection.

Missing:

- stable project endpoint resource/key,
- enforced endpoint grants,
- input/output schema enforcement,
- endpoint/caller idempotency with payload fingerprint,
- typed outcomes and side-effect certainty,
- external invoke/status/cancel routes and conformance fixture.

## Scope

1. Add project-scoped endpoint resources with `draft`, `active`, and `disabled`
   lifecycle and one approved immutable workflow version binding.
2. Authorize external OIDC client-credentials callers through active registered
   service principals and explicit invoke/read/cancel project-endpoint grants.
3. Add asynchronous invoke, status, and cancel routes with stable links and a
   configured execution timeout.
4. Enforce JSON Schema Draft 2020-12 input before runtime creation and output
   before success.
5. Use RFC 9457 for HTTP request errors and stable domain outcomes for run
   completion.
6. Scope idempotency to endpoint/caller and reject key reuse with changed input.
7. Expose side effects as `none`, `confirmed`, or `uncertain`.
8. Separate bounded JSON from authorized artifact references with checksum,
   media type, retention, and expiry metadata.
9. Add matching Admin-New, CLI, OpenAPI, and fake-BPM polling surfaces.

## Non-Goals

- BrowserPane subprocesses or BPMN orchestration.
- BrowserPane-managed Human Handoff.
- Teach Mode or workflow generation.
- Webhooks, callbacks, endpoint revision promotion, connector generation,
  tracing expansion, throttling, or production SLOs; these belong to `#240`.
- BrowserPane-issued API keys.

## Dependencies And Risks

- `#47` is the required workflow package/publishing contract.
- `#174` consumes this endpoint for the selected Pilot process; endpoint
  development uses a deterministic fixture and does not wait for candidate
  selection.
- `#180` must pass before external Pilot use relies on open-source governance.
- `#21` and `#66` are conditional on selected artifact and deployment needs.
- `#176` is not an implementation dependency. This issue owns the narrow
  Workflow Endpoint grant needed by the Phase 0 machine routes; `#176` owns
  generalized organization membership, project roles, ownership transfer, and
  authorization across all BrowserPane resource families.
- An uncertain mutating side effect must never be presented as safely
  retryable.

## Contract And Ownership Decisions

### Resource And Route Boundary

- An endpoint key is unique within its project and immutable after creation.
  Activation changes lifecycle, not identity.
- Owner/operator CRUD uses the project-scoped
  `/api/v1/projects/{project_id}/workflow-endpoints` resource family.
- Machine invocation uses
  `POST /api/v1/projects/{project_id}/workflow-endpoints/{endpoint_key}/invocations`.
  Status and cancellation use the returned invocation link under the same
  project and endpoint key.
- One invocation creates or reuses exactly one existing workflow-run resource.
  The external invocation representation is a restricted projection and must
  not expose owner-only labels, credentials, logs, or unrelated artifacts.
- Existing owner-scoped `/api/v1/workflow-runs` routes remain supported and do
  not gain implicit machine access.

### Endpoint Grant Versus `#176`

- `#172` adds one enforceable Workflow Endpoint grant keyed by project,
  endpoint, and registered service principal. Its only Phase 0 operations are
  `invoke`, `read`, `cancel`, and `artifact.read`.
- The grant is evaluated only for the new endpoint invocation/status/cancel and
  endpoint-produced artifact routes. It grants no project catalog visibility,
  session access, workflow editing, credential access, or general resource
  role.
- Token validation still resolves issuer/client identity to an active
  registered service principal before evaluating the endpoint grant. Disabled,
  missing, cross-project, and operation-mismatched grants deny before runtime
  side effects.
- `#176` later adds organization membership and generalized project/resource
  authorization. When present, its decision is an additional gate; it must not
  silently broaden or delete the explicit endpoint grants created by `#172`.
- Admin-New manages the narrow grant on the endpoint detail route. The Identity
  route may link to it but must not describe it as complete RBAC.

### Surface Impact Decisions

- API/OpenAPI: additive project endpoint, grant, invocation, status, cancel,
  and authorized artifact schemas/routes. HTTP failures use RFC 9457 and all
  public shapes are added to `openapi/bpane-control-v1.yaml` with compatibility
  fixtures.
- BrowserPane protocol: N/A. No `bpane-protocol`, WebTransport, tile, media, or
  interactive-session wire change is required. Endpoint state is an HTTP
  control-plane contract above existing workflow/session execution.
- Event delivery: no new external callback or CloudEvents contract. Existing
  retained workflow events may receive additive typed outcome/correlation
  fields needed by polling; #240 owns external event delivery and replay.
- Persistence: additive endpoint/grant records plus nullable endpoint caller,
  request fingerprint, outcome, and side-effect evidence on endpoint-backed
  workflow runs. In-memory and Postgres stores must implement the same contract.
- Admin-New: add project endpoint catalog/detail, lifecycle, schema/limit
  preview, narrow caller-grant controls, invocation example, and related-run
  inspection. Existing workflow/run routes remain available.
- CLI: extend the maintained workflow CLI with endpoint create/list/get/update,
  activate/disable, grant/revoke, invoke/status/cancel, and safe artifact
  retrieval operations.
- SDK: N/A for Phase 0. BrowserPane has no separately supported generated SDK;
  the canonical OpenAPI contract, CLI, curl example, and fake-BPM fixture are
  the integration surfaces. SDK generation remains a later compatibility
  decision rather than hidden scope in this issue.

## Deployment And Configuration

- No new mandatory service is introduced. The Phase 0 deployment reuses the
  gateway, Postgres, external OIDC issuer, workflow worker, session runtime,
  credential provider, and artifact store already present in Compose.
- Local Compose must seed a confidential Keycloak test client, matching active
  service-principal registration, project endpoint grant, immutable workflow
  package, and deterministic fake-BPM caller. Secrets remain in the identity
  provider/secret setup and are never persisted on the endpoint resource.
- Endpoint execution timeout, inline JSON result limit, and artifact behavior
  are explicit resource fields bounded by documented gateway safety ceilings.
  Missing or out-of-range values fail activation rather than becoming
  unbounded runtime configuration.
- Endpoint activation checks the immutable workflow version, JSON schemas,
  project state, grant model, worker/runtime readiness contract, and required
  artifact dependencies without starting a browser.
- The supported Phase 0 profile is the existing bounded single-node Compose
  deployment with docker-backed workflow/session workers. Multi-node admission,
  regional failover, private connectivity profiles, and production SLOs remain
  conditional/later work under #66 and the Production lane.

## Migration, Compatibility, And Rollback

- The change is additive to public v1. Existing owner workflow definitions,
  versions, runs, automation tokens, and `/api/v1/workflow-runs` clients retain
  their current behavior.
- Database migrations create endpoint/grant storage and add nullable endpoint
  correlation/outcome fields to workflow runs. Existing rows require no
  backfill and remain owner/internal runs.
- Uniqueness is enforced for project/endpoint key, endpoint/principal grant,
  and endpoint/caller/idempotency key plus request fingerprint. Concurrent
  identical invocation must resolve to one run.
- Rollout order is additive migration, gateway/store deployment, Admin-New/CLI,
  then explicit endpoint activation. Merely deploying the code exposes no
  active machine endpoint.
- Rollback first disables endpoint invocation, then restores the prior
  application version. Additive tables/nullable fields and retained run
  evidence stay in place for forward recovery; rollback must not delete
  endpoint-backed runs or artifacts.
- A prior binary may ignore the new records but cannot safely administer active
  endpoints. Operators must disable endpoints before application rollback.
- #240 may add immutable endpoint revisions later. Phase 0 does not pretend
  that updating an active endpoint is environment promotion: workflow binding,
  schemas, and limits are editable only while draft/disabled, and a changed
  production contract requires disable/update/reactivate with audit evidence.

## Observability And Operator Feedback

- Audit endpoint lifecycle, grant lifecycle, caller principal, project,
  endpoint key, invocation/idempotency identifiers, authorization outcome, and
  terminal category without recording tokens, request secrets, unrestricted
  inputs, browser payloads, or artifact contents.
- Reuse workflow-run events/logs/metrics for execution while adding bounded
  endpoint admission, validation-denial, idempotent-replay/conflict, outcome,
  timeout, cancellation, and side-effect-state evidence.
- Admin-New and CLI must distinguish authentication failure, authorization
  denial, inactive endpoint, schema validation, admission/technical failure,
  timeout, cancellation, artifact expiry, and uncertain side effects.
- Metrics use bounded operation/outcome labels and must not use endpoint keys,
  principal ids, project ids, request ids, URLs, or workflow inputs as labels.

## Documentation And Claim Impact

- Update OpenAPI, README integration guidance, ARCH control-plane ownership,
  Admin-New help/API companion, CLI help/examples, the validation matrix, and
  AGENTS architecture map when modules/routes are added.
- Update capability maturity and roadmap evidence only after the real Compose
  OIDC/Postgres/worker/fake-BPM smoke passes.
- Do not claim production-ready BPM integration, guaranteed exactly-once
  browser effects, Human Handoff, callback delivery, generalized RBAC, or
  connector compatibility from this slice.
- Investor/management material requires no implementation-time claim update.
  A Phase 0 evidence claim may be added only after #174 proves one selected
  activity through this endpoint contract.

## Implementation Steps

1. Freeze endpoint, grant, request, result, outcome, and side-effect schemas.
2. Add in-memory/Postgres resources and shared contract tests.
3. Implement service-principal authorization and negative cross-project tests.
4. Add schema validation, idempotency fingerprinting, and outcome mapping.
5. Add invoke/status/cancel API routes and workflow/session lifecycle binding.
6. Add Admin-New endpoint catalog/detail and caller-grant controls.
7. Add CLI operations, OpenAPI contract, examples, and fake-BPM fixture.
8. Run unit, store-contract, API, worker, compose, admin, CLI, and negative
   security validation.

## Test Strategy

### Unit

- endpoint lifecycle and key validation,
- JSON Schema compilation and bounded validation errors,
- request fingerprint and idempotency conflicts,
- outcome, retryability, timeout, cancellation, and side-effect mapping,
- artifact result bounds and redaction.

### Integration

- in-memory/Postgres parity,
- service-principal/project/endpoint authorization,
- input denial before runtime creation,
- invalid output preventing success,
- cancellation propagation and retained terminal evidence.

### Smoke And E2E

- valid client-credentials invocation through success,
- identical replay and changed-payload conflict,
- invalid input, invalid output, policy denial, runtime failure, timeout, and
  cancellation,
- challenge mapped to `external_intervention_required`,
- ambiguous submit mapped to `side_effect_state=uncertain`,
- API/Admin-New/CLI/OpenAPI/fake-BPM consistency.

### Regression And Quality Gates

- `cargo fmt --all -- --check`, workspace clippy, focused gateway tests, full
  workspace tests, and the enforced Rust coverage stage must pass.
- Store contract tests cover in-memory/Postgres parity, concurrent
  idempotency, migration defaults, disabled/missing/cross-project grants, and
  rollback-preserved historical reads.
- API tests cover every endpoint lifecycle and invocation status plus RFC 9457
  validation, conflict, authentication, authorization, timeout, cancellation,
  artifact, and not-found responses.
- Existing owner `/api/v1/workflow-runs`, workflow package publication, worker
  execution, recording/artifact, and project policy suites remain green.
- Admin-New component/view-model/client tests cover loading, empty, denied,
  invalid, submitting, success, disablement, and stale-refresh states for
  catalog/detail/grant/invocation surfaces.
- CLI parser/client tests and smoke cover every new command, structured output,
  non-zero failures, redaction, and parity with OpenAPI examples.
- Compose E2E uses real Keycloak client credentials, Postgres, the workflow
  worker, docker-backed browser session, and fake-BPM polling. It includes
  valid execution and every negative case in the post-implementation smoke.
- Changed code must not reduce repository-enforced Rust or Node coverage gates;
  exclusions and unexecuted target-like tests must be stated in the PR rather
  than counted as evidence.

## Post-Implementation Smoke Sequence

1. Create a project, service principal, immutable workflow version, and active
   endpoint.
2. Invoke with valid input and poll through schema-valid success.
3. Replay the same request and verify no duplicate browser action.
4. Reuse the key with changed input and verify conflict.
5. Verify invalid input creates no session or worker.
6. Verify invalid output cannot produce success.
7. Exercise denial, technical failure, timeout, and cancellation.
8. Verify challenge and uncertain-side-effect terminal outcomes.
9. Verify artifacts, correlation, redaction, retention, and authorization.
10. Run API, Admin-New, CLI, and fake-BPM conformance smokes.

## Definition Of Done

- The issue acceptance criteria and this plan agree.
- Every public surface exposes one endpoint contract.
- Negative authorization and pre-side-effect validation are proven.
- A target-like BPM can invoke, poll, reconcile, and cancel the activity.
- No Human Handoff, Teach Mode, subprocess, or advanced `#240` scope is added.
- Narrow endpoint grants are proven without implementing or claiming the
  generalized `#176` authorization model.
- Migration, activation, rollback, deployment, OpenAPI, Admin-New, CLI,
  documentation, regression, and negative security evidence are recorded.
