# BPANE-00214 Runtime Launch Broker Plan

Issue: [#214 Implement a policy-validating runtime launch broker](https://github.com/ITmedes/browserpane/issues/214)

Status: active; checkpoints 1 and 2 complete; checkpoint 3 next

## Business Case

The #167 compose boundary prevents the public gateway from mounting the host
Docker socket and denies unrelated Docker API families. It cannot inspect the
body of an allowed container or volume request. A compromised gateway could
therefore still ask the generic Docker API for an unapproved image, host bind,
network, environment, capability, privileged mode, or unrelated resource.

BrowserPane needs a purpose-specific authorization boundary that accepts only
typed BrowserPane lifecycle operations and constructs Docker requests from
broker-owned policy. The gateway must describe the intended product operation;
it must not be able to submit raw Docker arguments or Docker API models.

## Example Use Case

A production-like Docker host runs a public BrowserPane gateway and an
internal runtime broker. The gateway requests a browser runtime for session
`S`. The broker verifies the authenticated caller, validates the session id,
approved browser image, fixed network, owned volume names, container prefix,
labels, resource limits, and non-privileged security settings, then launches
the runtime. A request that adds `/` as a host bind or selects another image is
rejected before Docker sees it. Workflow, recording, and storage-helper
operations use separate typed request variants and separate allowlists.

## Current Code Inventory

- Browser runtime launch, readiness, stop, inspect, and context/session-file
  helper operations are implemented in `runtime_manager/docker/`.
- Workflow worker launch, wait, bounded output, and removal are implemented in
  `workflow_lifecycle/workers.rs` and currently invoke the Docker CLI directly.
- Recording worker launch and bounded process supervision are implemented in
  `recording_lifecycle/workers.rs`; compose currently supplies raw Docker CLI
  arguments through gateway flags.
- Browser-context and session-file helpers use short-lived containers, named
  volumes, stdin/stdout archive streams, and exact storage measurements.
- `SessionManager` is the control-plane boundary for browser session runtimes,
  but worker and storage operation ownership is not yet represented by one
  shared launch contract.

## Architecture Decisions

1. Add a shared Rust contract crate for typed broker requests, responses,
   stable error codes, ownership metadata, and policy validation inputs.
2. Do not expose raw command arguments, arbitrary environment maps, host paths,
   Docker API models, or caller-selected privilege fields in the contract.
3. Implement a dedicated broker process. The broker owns Docker request
   assembly and is the only application service on the Docker-control network.
4. Keep the #167 Docker proxy behind the broker as defense in depth. The
   gateway-to-broker network is separate from the broker-to-proxy network.
5. Authenticate gateway-to-broker requests with short-lived, audience-bound
   service credentials and replay identifiers. Keep the transport replaceable
   by workload identity or mTLS in orchestrated deployments.
6. Use bounded request bodies, deadlines, concurrency, output capture, and
   idempotency keys. Never log credentials, environment values, archive
   content, raw Docker errors, or untrusted request bodies.
7. Preserve the current direct/proxy compose mode until every operation family
   passes parity tests. The final topology switch is a distinct checkpoint.
8. Keep Docker-specific construction inside the broker adapter so a future
   Kubernetes or cloud adapter can implement the typed operation contract
   without leaking Docker models back into control-plane callers.

## Typed Operation Families

### Browser Runtime

- launch one owned session runtime,
- inspect readiness/existence,
- stop and force-remove an owned runtime,
- reconcile persisted owned assignments.

The request contains BrowserPane identifiers and approved feature selections.
The broker derives container name, image, labels, network, volumes, security
settings, resource bounds, entrypoint, and allowed environment keys.

### Workflow Worker

- launch one worker for an immutable workflow run,
- wait for terminal status with bounded stdout/stderr evidence,
- cancel/remove an owned worker,
- reconcile stale assignments.

Secrets are transmitted only in the authenticated request and materialized as
approved environment keys. They are excluded from errors, audit records, and
response bodies.

### Recording Worker

- launch one worker for a session/recording pair,
- wait for terminal status with bounded output,
- stop/remove an owned worker,
- reconcile stale assignments.

The broker owns the image, network, output-volume mount, entrypoint, and
environment allowlist. Gateway configuration no longer supplies arbitrary
Docker CLI arguments in broker mode.

### Storage Helpers

- initialize and write approved session-data volume paths,
- materialize session-file data,
- clone/export/import/delete browser-context profile data,
- measure requested context profile storage,
- inspect/remove owned named volumes.

Archive and file payloads are streamed with explicit byte, entry, path, and
deadline limits. Helper containers are network-disabled, non-privileged, and
mount only broker-derived owned volumes at fixed paths.

## Policy Contract

The broker policy must validate or derive all of the following:

- operation kind and lifecycle transition,
- request schema version, idempotency key, caller audience, expiry, and replay id,
- UUID resource identifiers and ownership correlation,
- exact approved image references, preferably immutable digests,
- fixed container-name prefixes and collision behavior,
- fixed ownership labels and rejection of caller-supplied labels,
- approved internal networks only,
- named-volume prefixes and fixed container mount targets,
- no arbitrary host binds, devices, capabilities, privileged mode, or host
  namespace sharing,
- fixed entrypoints/commands per operation,
- operation-specific environment-key allowlists and value bounds,
- CPU, memory, process, shared-memory, timeout, and concurrency limits,
- bounded output and archive transfer,
- stable sanitized denial and dependency-failure responses.

## Implementation Checkpoints

### 1. Shared Contract And Policy Evaluator

- Add the shared contract crate without changing the default runtime path.
- Define versioned operation/request/response and stable error resources.
- Define broker-owned policy shapes for every operation family.
- Implement deny-by-default validation with exhaustive negative unit tests for
  image, name, network, mount, environment, labels, privilege, resource, and
  lifecycle violations.
- Add serialization golden tests that prove no raw Docker arguments or secret
  values appear in audit resources.

Manual checkpoint: run contract tests and inspect representative accepted and
denied policy decisions. Existing compose behavior must remain unchanged.

Completed evidence:

- `bpane-runtime-contract` provides all four typed operation families, stable
  versioned wire resources, bounded redacted secrets, and deny-by-default
  launch/lifecycle policy without Docker request models.
- 25 contract unit tests and 2 wire/golden tests cover accepted policy plus
  image, mount, network, environment, label, privilege, resource, lifecycle,
  malformed identifier, unknown-field, and redaction denials.
- Commits: `bf0aee0f`, `13957bdd`.

### 2. Authenticated Broker Foundation

- Add the broker binary with liveness/readiness and bounded JSON request handling.
- Add a gateway client behind an internal trait with strict deadlines.
- Add audience/expiry/replay validation and sanitized audit events.
- Add concurrency/backpressure and idempotent operation-result storage.
- Add a compose service and private gateway/broker network, but keep the
  default runtime path on the #167 proxy until lifecycle parity exists.

Manual checkpoint: validate health, valid authentication, expired/wrong-audience
credentials, replay denial, malformed/oversized bodies, overload, and redaction.

Completed evidence:

- `bpane-runtime-broker` exposes bounded health and versioned operation routes,
  validates asymmetric OIDC/JWKS service credentials, enforces audience/client
  identity, and provides bounded idempotency, replay, timeout, and concurrency
  behavior. Its adapter remains intentionally fail-closed.
- `bpane-runtime-client` uses the maintained `oauth2` client-credentials flow,
  redirect-disabled HTTP clients, short-lived token caching, typed requests,
  strict deadlines, response bounds, media-type validation, and sanitized
  errors. It is not connected to the gateway runtime path until checkpoint 3.
- Compose places the broker on isolated gateway API and Keycloak auth networks
  with no host port, Docker network/socket, capabilities, writable root, or
  additional network peers. The current gateway-to-proxy path remains active.
- 17 broker tests, 8 client tests, 4 topology-contract tests, full Rust workspace
  tests, strict changed-crate Clippy/Rustdoc/formatting, Dockerfile checks,
  repository document validation, and the live Keycloak authentication/denial
  smoke pass.
- Commits: `8aaad7c6`, `61ec3dd2`.

### 3. Browser Runtime Migration

- Move browser launch argument construction and lifecycle operations into the
  broker Docker adapter.
- Add an optional broker-backed session manager configuration.
- Preserve startup readiness, exact-live reconnect, idle stop, capacity,
  context writer exclusion, egress, extensions, and session-file behavior.
- Run browser session unit, compose API, multi-session, reconnect, and MCP smokes.

Manual checkpoint: opt into broker mode locally and operate two concurrent
sessions plus reconnect and MCP delegation before changing the default.

### 4. Workflow And Recording Worker Migration

- Replace gateway-side raw Docker worker construction with typed operations.
- Preserve bounded output, cancellation, restart reconciliation, recording
  finalization, artifact staging, and playback behavior.
- Remove arbitrary recording Docker args from production broker mode.
- Run worker package, API, cancellation, restart, recording, playback, and
  download tests.

Manual checkpoint: execute a workflow and an always-on recording through the
broker, inspect their evidence, and verify cleanup after normal and forced stop.

### 5. Storage Helper Migration

- Move all context/session-data helper operations behind typed broker requests.
- Add bounded streaming for file/archive inputs and outputs.
- Preserve clone/export/import, storage quotas, materialization, and cleanup.
- Add negative tests for wrong prefixes, unexpected volumes, unsafe paths,
  malformed archives, excessive payloads, and interrupted transfers.

Manual checkpoint: clone/export/import a context, bind a workspace file, run a
session, and verify storage usage and cleanup without gateway Docker access.

### 6. Production-Like Topology Switch

- Remove the gateway from the Docker-control network.
- Make broker mode the production-like compose topology and retain an explicit
  local direct/proxy compatibility profile only where needed.
- Prove the gateway cannot reach the proxy or Docker daemon.
- Add restart/reconciliation, drain, timeout, backpressure, and denial audits.
- Update README, architecture, remote deployment, runtime requirements,
  capability maturity, risk, and delivery state.

Manual checkpoint: execute the complete post-implementation smoke sequence and
inspect safe broker audit metadata before closing #214.

## Validation Strategy

- Shared contract and policy unit tests, including table-driven rejection cases.
- Broker auth, replay, schema, size, timeout, concurrency, idempotency, and
  sanitization tests.
- Gateway adapter tests with deterministic fake broker responses.
- Docker adapter integration tests against the #167 proxy boundary.
- Existing gateway unit/integration and full compose API suites.
- Browser smokes for sessions, reconnect, MCP, admin-new contexts, recording,
  workflows, workspace inputs, and CLI operations.
- Coverage baselines for every new Rust crate and affected frontend/worker code.
- Static topology validation proving the gateway has no socket, proxy network,
  proxy port, or generic Docker endpoint path in broker mode.

## Post-Implementation Smoke Sequence

1. Start the broker topology and verify gateway, broker, proxy, Postgres, Vault,
   Keycloak, MCP bridge, and web readiness.
2. Prove unauthenticated, expired, wrong-audience, replayed, malformed,
   oversized, and overloaded broker requests are denied without secret leakage.
3. Prove unapproved images, names, labels, networks, mounts, environment keys,
   entrypoints, capabilities, privilege flags, resources, and lifecycle targets
   are denied with no Docker residue.
4. Create, connect, resize, disconnect, reconnect, stop, restart, and delete a
   browser session through the broker.
5. Run two sessions, join one twice, delegate/switch MCP, and verify browser
   side effects reach only the selected session.
6. Execute, cancel, and restart-reconcile workflow workers; verify logs, events,
   produced files, workspace policy, and cleanup.
7. Record a session, exercise disconnect/stop/kill finalization, and verify
   WebM/playback/export download plus worker cleanup.
8. Clone/export/import/delete a browser context, enforce storage quota, and
   materialize/read a session-file binding.
9. Restart gateway and broker independently and verify persisted runtime,
   workflow, and recording assignment reconciliation.
10. Run unit, integration, coverage, full compose, admin-new, compatibility,
    client, worker, CLI, documentation, and topology-validation suites.

## Acceptance Criteria

- The production-like gateway has no Docker socket, Docker API proxy network,
  or generic Docker API access.
- Every supported Docker-backed operation uses a versioned typed broker request.
- Broker policy derives or validates every Docker-sensitive field and denies
  unknown fields and lifecycle targets.
- Gateway-to-broker authentication is audience-bound, short-lived, replay
  resistant, deadline bounded, and redaction tested.
- Browser, workflow, recording, context, and session-file behavior retains
  compose and browser-smoke parity.
- Safe audit evidence distinguishes accepted, denied, failed, and reconciled
  operations without exposing credentials or payload content.
- A non-Docker adapter can implement the shared operation contract without
  importing Docker request models.

## Rollback

Each pre-switch checkpoint keeps the #167 proxy path available. If a broker
operation family fails parity validation, disable broker mode for that family
and retain the typed contract/policy tests while fixing the adapter. After the
topology switch, rollback requires an explicit operator choice of the documented
local proxy compatibility profile; production documentation must not present
that profile as an equivalent authorization boundary.

## Out Of Scope

- Kubernetes, Fargate, Cloud Run, or Azure adapter implementations.
- Arbitrary operator-provided images, commands, mounts, networks, or privileges.
- A public general-purpose container service.
- Replacing BrowserPane resource authorization or project policy.
- Claiming the #167 generic proxy alone validates permitted request bodies.
