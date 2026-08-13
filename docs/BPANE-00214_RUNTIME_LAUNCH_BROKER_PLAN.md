# BPANE-00214 Runtime Launch Broker Plan

Issue: [#214 Implement a policy-validating runtime launch broker](https://github.com/ITmedes/browserpane/issues/214)

Status: active; checkpoints 1 through 3 complete; checkpoint 4 is next

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

Execution slices:

1. Add a broker-owned browser container adapter using a maintained Docker API
   client. Derive names, base environment, labels, volumes, network, security,
   and resource bounds from trusted broker configuration and typed resource ids;
   cover launch/inspect/stop/remove plus backend denial and sanitization tests.
2. Extend the typed browser intent and broker materializer for the currently
   supported reusable context, egress, proxy-auth/CA, extension, and
   session-data selections. Keep arbitrary maps, host paths, raw Docker models,
   and caller-selected privilege fields out of the wire contract.
3. Add the opt-in gateway client/backend, preserve existing admission and
   persistence ownership, and run lifecycle/reconnect/MCP parity before the
   compose default or gateway Docker network changes.

Progress: all three execution slices are complete on
`feature/BPANE-00214-gateway-runtime-broker`.

Slice 1 evidence:

- The broker owns browser container names, fixed image/command, private network,
  named-volume mounts, base environment, ownership labels, no-new-privileges,
  and CPU/memory/PID/shared-memory bounds. Lifecycle targets are derived from
  typed resource ids rather than accepted as Docker names.
- Bollard provides the private Docker Engine transport; Docker request models
  do not enter the BrowserPane wire contract or gateway client.
- Seven adapter tests cover base and reusable-context launch materialization,
  inspect/stop absence semantics, partial-launch cleanup, unsupported-family
  denial, malformed trusted configuration, sanitized failures, and actual
  Bollard HTTP route/body behavior.
- Broker unit tests, strict Clippy/Rustdoc/formatting, dependency safety,
  release-image build, and the existing live fail-closed authentication smoke
  pass. The compose service still selects `RejectingRuntimeExecutor`.
- Commit: `93a829c5`.

Execution slice 2 scope:

- Extend the browser launch intent with bounded, typed network-identity and
  egress selections, approved extension-version identifiers, and fixed
  session-data prerequisites.
- Keep proxy credentials, CA bytes, arbitrary environment maps, extension
  install paths, host paths, and Docker request models out of the wire
  contract. Proxy-auth, custom-CA, and session-file selections refer only to
  data prepared at broker-owned fixed paths by the later typed storage path.
- Resolve extension-version identifiers through trusted broker configuration;
  reject unknown or duplicate identifiers instead of accepting caller-selected
  paths.
- Derive every environment key and egress correlation label in the broker,
  validate proxy URLs and bounded identity values, and extend launch policy
  only for those broker-derived keys.
- Keep the compose executor fail-closed and the gateway on its existing path.
  Gateway integration remains execution slice 3 after storage prerequisites
  and feature parity are testable.

Slice 2 acceptance:

- positive materialization tests cover identity, metadata-only proxy, TLS
  interception prerequisites, authenticated proxy prerequisites, reusable
  context, approved extensions, and session-file bindings;
- negative contract and adapter tests cover malformed identity, credentialed
  or unsupported proxy URLs, unsafe bypass rules, invalid combinations,
  duplicate or unknown extension ids, and policy drift;
- contract golden tests prove that secrets, install paths, host paths, raw
  Docker arguments, and Docker API fields are absent from browser launch JSON;
- broker unit, strict Clippy/Rustdoc/formatting, dependency, image-build, and
  existing fail-closed authentication smoke checks pass.

Slice 2 evidence:

- `BrowserRuntimeLaunchRequest` now carries bounded semantic feature intent for
  locale/languages, timezone, fixed-point geolocation, user agent, browser
  identity, egress profile/proxy/observation prerequisites, approved extension
  version ids, and prepared session-file bindings. Empty features retain the
  original v1 JSON shape.
- Contract validation matches the existing gateway constraints for HTTP/HTTPS
  proxies, inline-credential denial, BCP-47-like locale tags, IANA-style
  timezones, geolocation bounds, TLS-interception prerequisites, bypass-rule
  safety, and extension count/identity uniqueness.
- The broker derives the exact environment and safe egress-correlation labels
  already consumed by the browser runtime. Proxy-auth and trusted-CA material
  use fixed broker-owned session-data paths; values and paths cannot be chosen
  by the caller.
- Extension install paths remain outside the wire contract and resolve only
  through a trusted broker registry keyed by extension-version id. Unknown ids
  fail before any Docker request.
- Policy authorization now supports an explicit set of broker-derived label
  keys while still requiring exact ownership/static labels and rejecting every
  unexpected key, control character, or oversized value.
- Feature contract and materialization were split into focused modules to keep
  production files and functions within `RUST_STANDARDS.md` guidance.
- Validation passed: 28 contract unit tests, 3 wire/golden tests, 27 broker
  tests, 8 client tests, the full Rust workspace, strict changed-crate
  Clippy/Rustdoc/formatting, repository and dependency gates, five runtime-path
  integration tests, release-image build, compose topology inspection, and the
  live Keycloak authentication/denial smoke.
- Changed-crate LLVM coverage reports 87.21% total line coverage; the new
  broker materializer reports 99.01%, broker browser config 100%, runtime
  policy 98.37%, and the extracted browser-feature contract module 95.60%.
- Compose remains intentionally fail-closed through
  `RejectingRuntimeExecutor`; no gateway behavior or user-facing API changed,
  so `README.md` does not require an update in this slice.
- Commits: `afd31dbf`, `99c86285`, `74b0fdbf`.

Execution slice 3 scope:

- Load the broker-side extension registry from a bounded, read-only JSON file at
  startup. Treat it as an immutable process snapshot: changes require a broker
  restart and invalid, duplicate, nil-id, or unsafe-path entries fail startup.
  The gateway sends extension-version ids only and cannot populate broker
  install paths through launch requests.
- Add an explicit `broker_pool` gateway runtime backend. Reuse the current
  Docker pool's capacity, reusable-context writer exclusion, assignment
  persistence, startup readiness, idle cleanup, egress/session-file
  preparation, and reconciliation state machine, but route browser container
  launch/inspect/stop/remove through `bpane-runtime-client`.
- Keep `docker_pool` as the compose and CLI default. Enable the broker Docker
  adapter and gateway broker backend only through a dedicated local compose
  overlay until parity and manual checkpoints pass.
- Preserve stable gateway runtime errors. OAuth, HTTP, broker, and Docker
  response details must remain sanitized and broker request retries must use
  operation-specific idempotency keys.
- Keep existing gateway storage preparation for proxy-auth, custom CA, and
  session files during browser launch parity. Those Docker helper operations
  move behind typed broker storage operations in checkpoint 5 before the
  gateway loses Docker-control access.

Slice 3 example use case:

An operator starts two project-scoped sessions with different reusable browser
contexts and egress profiles through the opt-in broker topology. The gateway
still enforces capacity and single-writer context admission and persists each
assignment, while the broker alone chooses the browser image, container name,
network, mounts, labels, extension install paths, and security settings. A
reconnect returns to the existing runtime and MCP resolves the same session CDP
endpoint; stopping a session removes only its broker-owned browser container.

Slice 3 acceptance:

- broker startup rejects missing or malformed adapter configuration and unsafe
  extension registry entries before opening the operation service;
- default compose remains fail-closed and `docker_pool` behavior is unchanged;
- `broker_pool` maps stored session intent to typed launch/lifecycle requests
  without install paths, secrets, arbitrary environment maps, or Docker models;
- launch, readiness wait, reconnect, persisted-assignment recovery, idle stop,
  explicit stop, capacity, context exclusion, egress, extensions, and prepared
  session-file behavior have direct-vs-broker parity coverage;
- OAuth/token, unreachable broker, denial, timeout, malformed response, and
  conflicting idempotency cases map to stable sanitized gateway failures;
- the opt-in compose overlay completes two-session, reconnect, stop, and MCP
  delegation smoke checks before any default topology change.

Slice 3 smoke sequence:

1. Validate base compose and prove the broker still has no Docker-control
   network and rejects browser operations.
2. Start the broker overlay and verify malformed registry/configuration startup
   fails closed, then start it with the approved immutable registry.
3. Create and connect two sessions through `broker_pool`; verify distinct
   persisted assignments, sockets, CDP endpoints, and broker-owned containers.
4. Reconnect one session and delegate it to MCP; verify both use the same live
   runtime and no replacement container is launched.
5. Exercise reusable-context exclusion, egress metadata, approved and unknown
   extension ids, and prepared session-file bindings.
6. Stop both sessions and verify browser containers and ephemeral data are
   removed while reusable context data remains.
7. Run runtime contract/client/broker/gateway unit tests, full Rust workspace,
   compose API, multi-session, reconnect, MCP, repository, and dependency gates.

Slice 3 evidence:

- The broker loads a bounded immutable extension registry and trusted browser
  environment snapshot at startup, selects an explicit `docker-browser`
  executor, and rejects mutable images or invalid adapter configuration.
- The gateway exposes `broker_pool` with file-backed OAuth client credentials,
  typed feature mapping, stable sanitized failures, operation-specific
  idempotency keys, and unchanged Docker pool admission, assignment,
  reconciliation, reusable-context, and storage-preparation behavior.
- The dedicated Compose overlay pins the built browser image by immutable image
  id, isolates gateway-to-broker and broker-to-proxy access, and leaves base
  Compose on the fail-closed broker plus default `docker_pool` path.
- Broker readiness probes the selected Docker dependency through `/readyz`
  without token acquisition, idempotency-ledger writes, or runtime-operation
  audit noise. Launch, inspect, stop, reconciliation, and idle lifecycle remain
  explicit typed operations.
- Focused Rust suites pass with 439 gateway tests, 35 broker tests, and 10
  runtime-client tests. The live broker overlay passes the gateway Compose API
  suite, multi-session/MCP delegation smoke, and admin create/connect/release/
  reconnect/stop smoke.
- Commits: `075442c1`, `73d93269`, `48959508`, `958c8054`, `a0115b0c`,
  `476ee727`, `1a441ddd`, `6aa22063`.

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
