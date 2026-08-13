# BPANE-00214 Runtime Launch Broker Plan

Issue: [#214 Implement a policy-validating runtime launch broker](https://github.com/ITmedes/browserpane/issues/214)

Status: active; checkpoints 1 through 5 complete; checkpoint 6 is in progress

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
- Workflow worker launch, supervision, cancellation, and removal use a shared
  gateway worker-control boundary. `docker_pool` retains the direct Docker CLI
  path; `broker_pool` sends typed operations through the existing authenticated
  broker client.
- Recording worker launch and supervision use the same direct/broker boundary.
  Base compose still supplies direct Docker CLI arguments; the broker path uses
  typed credentials and broker-owned container materialization.
- Browser-context and session-file helpers use short-lived broker-owned
  containers, derived named volumes, bounded archive transfers, and exact
  storage measurements in `broker_pool`; `docker_pool` retains direct helpers.
- `SessionManager` is the control-plane boundary for browser session runtimes,
  and the shared worker-control boundary selects direct or broker-backed
  workflow and recording lifecycle. Storage helper ownership is routed through
  typed broker operations in `broker_pool`.

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

Execution slices:

1. Extend the typed worker lifecycle result so a detached broker-owned worker
   can report running, exited, and absent state without returning Docker models
   or raw logs. Add a policy-validating Bollard adapter for workflow and
   recording launch/inspect/stop/remove. The broker derives immutable images,
   names, network, recording artifact volume, commands, environment keys,
   security settings, resource limits, and bounded Docker log retention from
   trusted startup configuration. Worker credentials remain redacted typed
   request fields and never enter audit metadata or errors.
2. Add a gateway worker-control boundary with direct-process and broker-backed
   implementations. In broker mode the existing lifecycle managers keep
   admission, persisted assignments, cancellation, terminal resource updates,
   recording finalization waits, and restart reconciliation, while a bounded
   poller observes typed broker worker state. Detailed workflow logs and
   recording artifacts continue to arrive through the existing worker-to-
   control-plane APIs; raw container logs are not copied into API errors.
3. Extend the opt-in Compose overlay with immutable workflow/recording image
   ids and read-only trusted worker configuration. Run workflow success,
   failure, cancellation, restart safety, runtime hold, produced-file, always-
   on recording, playback/export/download, disconnect/stop/kill finalization,
   and cleanup smokes before considering any default switch.

Progress: all three execution slices are complete on
`feature/BPANE-00214-gateway-runtime-broker`.

Slice 1 evidence:

- The v1 contract reports detached workers as typed `running`, `exited`, or
  `absent` state with an optional exit code. Recording launch credentials now
  preserve the direct path's optional static gateway bearer without exposing
  it through debug output or audit resources.
- The broker derives immutable images, owned names, fixed networks, exact
  commands, worker-specific environment allowlists, no-new-privileges,
  CPU/memory/PID/shared-memory bounds, and bounded local Docker logs.
- Workflow workers receive no mounts. Recording workers receive only the fixed
  configured artifact volume at the fixed output root. Host binds, devices,
  capabilities, privileged mode, host namespaces, mutable images, unknown
  operation families, and unowned lifecycle targets remain denied.
- Docker inspect responses are normalized behind the adapter and retain a
  detached worker's exit code without returning Docker models or raw logs.
- Validation passed with 29 contract tests, 4 exact wire tests, 42 broker tests,
  strict changed-crate Clippy/Rustdoc/formatting, and a real HTTP-shaped Bollard
  inspect regression test.
- Commits: `f62f144f`, `f4b8ee9c`.

Slice 2 evidence:

- `broker_pool` reuses the browser runtime's existing cached OAuth broker
  client for workflow and recording workers. It does not create a second token
  implementation or expose the client outside the gateway runtime boundary.
- The lifecycle managers retain admission, persisted assignments, project
  quotas, direct-process supervision, workflow terminal-state reconciliation,
  recording finalization waits, and direct Docker behavior. Broker mode adds
  typed launch/inspect/remove plus bounded detached-worker polling.
- Worker exit, cancellation, and gateway-restart reconciliation remove only
  the broker-owned resource derived from its workflow-run or recording id.
  Three consecutive broker inspection failures become a stable sanitized
  lifecycle failure rather than leaving an assignment indefinitely healthy.
- Workflow logs/events/outputs and recording artifacts still flow through the
  existing worker APIs. Broker-side raw container logs are not copied into
  gateway errors.
- Direct workflow and recording lifecycle regression tests pass alongside
  broker launch/monitor/remove integration tests, credential-redaction tests,
  broker unavailable/invalid-result tests, and bounded monitor retry tests.
- The full gateway run passed 445 tests with one external Postgres contract
  test ignored, all integration test binaries, the source-size gate, and strict
  Clippy. README does not change yet because the opt-in compose worker topology
  is not available until execution slice 3.

Slice 3 evidence:

- The opt-in overlay loads a bounded read-only worker policy snapshot,
  file-backed worker OIDC secret, and recording certificate input. Browser,
  workflow, and recording images are resolved to immutable local image ids
  before Compose interpolation or service startup.
- Broker worker policy owns fixed images, networks, commands, resource bounds,
  environment allowlists, and the single recording artifact-volume mount.
  Docker local logs are capped to one file with explicit compression disabled;
  a live start regression caught and fixed Docker's incompatible default of
  compression with `max-file=1`.
- The overlay topology contract now rejects mutable worker images, writable
  worker policy/secret/certificate mounts, missing file-backed OIDC bootstrap,
  direct socket mounts, and gateway routing that bypasses `broker_pool`.
- Broker-mode live smokes passed for workflow success, expected failure,
  cancellation, produced-file and CLI operations, workspace input policy,
  runtime hold/release, always-on recording, disconnect finalization, retained
  WebM, playback manifest/export, CLI downloads, UI downloads, and worker
  cleanup with no container residue.
- Gateway restart safety passed under broker worker backpressure: the stale
  in-flight run failed deterministically, its durable queued follower started
  once and succeeded, and an awaiting-input run survived a second gateway
  restart and completed after operator resume.
- Base Compose remained on `docker_pool`; its fail-closed broker rejected
  browser, workflow, and recording launch requests with sanitized responses.
  Direct workflow cancellation and direct always-on recording/playback/download
  smokes also passed.
- Workflow-worker and recording-worker package tests/builds pass, along with 45
  broker tests, 447 gateway tests, strict changed-crate Clippy/formatting, and
  the expanded Compose contract tests. README, ARCH, and AGENTS now describe
  the transitional browser-and-worker broker topology; storage remains the
  reason the gateway still joins Docker control.
- Commits: `f62f144f`, `f4b8ee9c`, `746d38f0`, `bcdd98c7`, `d93df734`,
  `40fd323e`, `0e993f28`, `ceca89bc`, `3913413d`.

Checkpoint 4 design constraints:

- Base Compose and direct worker behavior remain unchanged.
- `broker_pool` selects broker worker control only when complete trusted worker
  adapter configuration is present; partial configuration fails startup.
- Broker launch is detached and idempotent. Gateway supervision uses typed
  inspect results and bounded deadlines rather than an unbounded broker HTTP
  request.
- Docker worker logs use broker-owned bounded rotation. The worker APIs remain
  the source of workflow logs, events, outputs, and recording artifacts.
- OIDC bootstrap values, gateway URLs, image references, mounts, and commands
  come from broker configuration. Launch requests carry only BrowserPane ids
  and purpose-scoped short-lived credentials.
- A broker or Docker outage maps to stable sanitized lifecycle failures and
  cannot leave a persisted assignment pretending to be healthy.

Checkpoint 4 example use case:

An operator starts a Playwright workflow against a broker-backed session and
opens the live preview while it runs. The gateway creates the workflow run and
short-lived automation credential, while the broker alone chooses and launches
the approved worker image. The worker posts logs and produced files through the
existing run-scoped APIs. If the operator cancels the run, the gateway sends a
typed stop/remove request and records the normal cancelled state. The same
topology starts an always-on recorder, waits for finalization during disconnect
or stop, and exposes the retained WebM/playback export without giving the
gateway permission to construct either worker container.

Checkpoint 4 smoke sequence:

1. Prove base Compose still launches workers directly and the base broker
   rejects worker operations.
2. Start the broker overlay and verify worker images are immutable, the
   recording artifact mount is broker-derived, and malformed/partial worker
   configuration fails closed.
3. Execute successful and failing workflows; inspect logs, events, structured
   outputs, produced files, session targeting, and broker audit correlation.
4. Cancel queued and running workflows, restart the gateway during a run, and
   exercise awaiting-input runtime hold/release.
5. Record a session through connect, disconnect, reconnect, explicit stop, and
   kill; verify contiguous segment state, WebM download, playback/export, and
   worker/container cleanup.
6. Deny unknown images, names, mounts, environment keys, lifecycle targets,
   replay conflicts, unavailable broker, and malformed responses without raw
   backend or secret leakage.
7. Run contract/client/broker/gateway unit and integration tests, worker builds,
   Compose API, workflow/recording browser smokes, full workspace, dependency,
   documentation, and topology gates.

Manual checkpoint: execute a workflow and an always-on recording through the
broker, inspect their evidence, and verify cleanup after normal and forced stop.

### 5. Storage Helper Migration

- Move all context/session-data helper operations behind typed broker requests.
- Add bounded streaming for file/archive inputs and outputs.
- Preserve clone/export/import, storage quotas, materialization, and cleanup.
- Add negative tests for wrong prefixes, unexpected volumes, unsafe paths,
  malformed archives, excessive payloads, and interrupted transfers.

Execution slices:

1. Add a separately authenticated storage-transfer route and client method
   around the existing typed `StorageHelperRequest`. Stream request and response
   payloads with explicit byte declarations and hard limits, keep payload bytes
   out of JSON/audit/idempotency resources, and expose them to a dedicated
   storage executor boundary. Cover missing, unexpected, truncated, excessive,
   and malformed transfers while leaving the gateway on its current direct
   helper path.
2. Add a broker-owned Docker storage adapter for session-data initialization,
   bounded file writes, browser-context clone/export/import/measure/delete, and
   owned-volume inspect/remove. Derive volume names, helper image, mounts,
   commands, paths, modes, user, network isolation, and resource limits from
   trusted broker configuration. Reject unsafe paths, malformed archives,
   unsupported entries, wrong volume prefixes, unexpected resources, helper
   failures, and interrupted transfers without exposing archive content or raw
   Docker errors.
3. Add a gateway storage-control boundary that preserves workspace-file
   reads, manifest construction, binding state, active-context exclusion,
   profile quotas, retention, and direct `docker_pool` compatibility while
   routing every `broker_pool` helper through the broker. Extend the opt-in
   Compose topology and run clone/export/import, file-binding, storage-usage,
   retention, reconnect, workflow, recording, API, CLI, and admin-new parity
   smokes before removing gateway Docker-control access in checkpoint 6.

Slice 1 acceptance:

- input-bearing actions require an exact payload declaration and reject absent,
  short, long, or over-limit request streams before execution; context imports
  must be non-empty while typed session files may be empty;
- output-bearing actions return only an explicitly declared, bounded binary
  stream and reject invalid content type, missing declarations, excessive
  output, and response/request correlation mismatches;
- non-streaming storage actions continue through the JSON operation route and
  reject payload bodies or declarations that do not match their action;
- authentication, concurrency, deadlines, replay/idempotency, stable errors,
  and sanitized audits apply equally to JSON and streaming operations;
- contract, broker API, client, strict formatting/Clippy/Rustdoc, and full
  workspace tests pass while base Compose and user-visible behavior remain
  unchanged.

Slice 1 example use case:

The gateway imports a reusable browser-context archive by submitting typed
context identity metadata plus a declared binary length. The broker authenticates
the service principal, bounds and hashes the stream, and invokes only the
storage executor. A truncated body, undeclared body, oversized archive, replay
with different bytes, or export response above the configured limit fails with
a stable sanitized error and never reaches Docker.

Slice 1 smoke sequence:

1. Run storage contract tests for all action/field/payload combinations and
   exact wire resources.
2. Exercise authenticated import and export against a deterministic fake
   storage executor and verify byte-for-byte transfer plus request correlation.
3. Deny missing, unexpected, truncated, excessive, wrong-media-type, replayed,
   and executor-failed transfers without payload or backend detail leakage.
4. Verify ordinary browser/worker JSON operations and fail-closed storage
   behavior remain unchanged.
5. Run changed-crate tests, strict Clippy/Rustdoc/formatting, full Rust
   workspace, dependency, repository, and documentation checks.

Slice 1 evidence:

- The authenticated `/v1/storage-transfers` route accepts versioned typed
  metadata plus an optional `application/octet-stream` multipart field. Request
  metadata and binary payloads have independent hard limits; payload length is
  checked against the typed declaration before executor dispatch.
- Binary exports carry explicit API version, request id, byte count, and
  lowercase SHA-256 headers. The client requires `Content-Length`, enforces its
  own payload ceiling while reading chunks, and verifies correlation, exact
  length, and digest before returning bytes to the gateway.
- Idempotency fingerprints include typed metadata and input bytes without
  retaining or logging payload content. Exact mutating retries reuse the
  completed result; conflicting bytes are rejected. Export retries must produce
  the same retained digest before the response is accepted.
- Streaming storage actions cannot use the ordinary JSON route. The broker
  exposes a dedicated fail-closed storage executor method, while existing
  non-streaming JSON operations and the direct gateway helper path remain
  unchanged until slices 2 and 3.
- Negative coverage includes unauthenticated, wrong-media, non-storage,
  malformed/interrupted multipart, missing, unexpected, short, excessive, and
  conflicting input; invalid, empty, oversized, uncorrelated, or digest-mismatched
  output; unsafe local client limits; adapter failure; and redaction behavior.
- Validation passed with 51 broker tests, 13 runtime-client tests, 30 contract
  tests, 5 exact wire tests, all 447 gateway tests, the full Rust workspace,
  strict changed-crate Clippy/Rustdoc/formatting, repository documents, and
  dependency safety. Base Compose and user-visible behavior are unchanged, so
  README and ARCH do not require updates in this transport-only slice.

Slice 2 acceptance:

- the broker derives helper image, container and volume names, fixed mount
  targets, file destinations, modes, commands, labels, and limits from trusted
  configuration and typed BrowserPane identifiers only;
- helper containers run without a network as the unprivileged `bpane` user,
  drop all capabilities, use no-new-privileges and a read-only root filesystem,
  and are force-removed after success, failure, or timeout;
- reusable context volumes preserve the direct helper layout, archive format,
  replacement semantics, ownership, permissions, and absent-volume behavior;
- import validation rejects traversal, non-UTF-8, links, special entries,
  excessive paths, entries, compressed bytes, and uncompressed bytes before
  Docker receives the archive;
- base Compose stays fail closed and the overlay enables the storage adapter
  with the same immutable host image while gateway storage calls remain direct
  until slice 3.

Slice 2 example use case:

An operator imports an inactive reusable context. The broker validates the
bounded gzip/tar archive, derives the target context volume, removes only that
owned target, and launches a short-lived network-disabled helper with the
target mounted at the fixed `/run/bpane/storage-helper/target` path. Any
malformed archive, unapproved path, helper failure, or timeout yields a
sanitized error and removes partial helper state without exposing archive
content or Docker diagnostics.

Slice 2 smoke sequence:

1. Unit-test every storage action, typed file destination, absent source,
   immutable configuration, archive denial, derived volume, and helper security
   invariant through a fake Docker boundary.
2. Run broker API storage tests for authentication, exact transfers,
   idempotency, correlation, malformed bodies, and sanitized adapter failures.
3. Validate the overlay resolves one immutable host image for browser and
   storage helpers and leaves base Compose fail closed.
4. Run contract, client, broker, gateway, full workspace, strict Clippy,
   Rustdoc, formatting, repository-document, and dependency checks.
5. Start the opt-in broker overlay and run
   `./scripts/smoke-runtime-broker-storage.sh`; verify the authenticated live
   import, measure, clone, initialize, typed file write, digest-checked export,
   session/context deletion, helper cleanup, and volume cleanup matrix.

Slice 2 evidence:

- `StorageRuntimeDockerAdapter` implements typed initialization,
  materialization, clone, export, import, measurement, and session/context
  deletion behind a narrow Bollard boundary; it accepts no Docker models or
  paths from the HTTP contract.
- Session writes use `SessionDataFileTarget`, so workspace bindings, manifests,
  proxy authentication, and trusted CA data resolve only below fixed
  broker-owned paths with broker-selected modes.
- The helper policy enforces an immutable image, derived named volumes and
  labels, fixed mounts and command, no network, dropped capabilities,
  no-new-privileges, a read-only root, bounded tmpfs/resources/output, and
  guaranteed container cleanup. Failed clone/import helpers also remove their
  partial target volume.
- Browser-context archives are validated with maintained gzip/tar libraries
  before dispatch and the helper retains extraction-time defense in depth.
- The opt-in overlay supplies the storage helper with the same immutable host
  image as the browser adapter. README and ARCH describe that the adapter is
  installed while gateway call-site migration remains slice 3.
- The reusable live storage smoke passes all storage actions through the
  authenticated broker route, verifies exported bytes and SHA-256 metadata,
  and proves helper-container plus owned-volume cleanup. Input helpers consume
  the exact broker-validated byte count, avoiding Docker attach EOF ambiguity.
- Validation passed with 65 broker tests, 32 contract tests, 13 runtime-client
  tests, 5 exact wire tests, all 447 gateway tests, the full Rust workspace,
  strict changed-crate Clippy/Rustdoc/formatting, repository documents,
  dependency safety, Compose/overlay policy checks, a release-image build, and
  the authenticated live storage matrix.

Slice 3 acceptance:

- `DockerRuntimeManager` selects an explicit direct or broker-backed storage
  control alongside browser control; `docker_pool` retains the existing Docker
  command path and `broker_pool` sends every session-data and browser-context
  storage operation through `RuntimeBrokerClient::execute_storage`;
- gateway callers submit only typed session-data destinations for workspace
  bindings, the binding manifest, proxy authentication, and the trusted CA;
  they cannot send a broker path, mode, image, mount, command, or Docker model;
- session-data initialization preserves optional reusable-context profile
  mounting, and ephemeral-session cleanup removes session storage through the
  selected control without changing runtime release semantics;
- context clone, export, import, measurement, retention deletion, and API
  deletion retain active-writer exclusion, absent-volume behavior, archive
  shape, quota reporting, replacement semantics, and sanitized errors;
- workspace bytes are still read by the gateway artifact-store boundary, and
  binding state changes to `materialized` only after every typed broker write
  plus the manifest write succeeds; failures retain actionable sanitized
  binding state without leaking file, credential, or CA content;
- broker results are action-checked: unexpected states, missing export bytes,
  payload/result mismatches, transport failures, and partial operations fail
  closed while direct-mode compatibility remains covered;
- base Compose remains on `docker_pool`; the opt-in overlay exercises the
  gateway-to-broker path and keeps gateway Docker-control removal as the
  separately validated checkpoint 6 topology switch.

Slice 3 example use case:

An operator starts a broker-backed session that uses a reusable browser
context, an authenticated TLS-intercept egress profile, and two workspace-file
bindings. The gateway resolves the approved credential and CA bytes, reads the
workspace artifacts, and sends typed storage intents plus bounded bytes to the
broker. The broker derives the only permitted volume and destination for each
write before launching the browser. A later context export and storage-usage
read also pass through the broker, while an export attempted during an active
context writer remains rejected by the gateway before any storage call.

Slice 3 smoke sequence:

1. Unit-test direct and broker storage-control selection, every storage action,
   exact typed request construction, result/payload validation, unavailable
   broker mapping, and direct Docker argument parity.
2. Test workspace binding and manifest materialization through a deterministic
   broker client, including read-only/read-write targets, binding-state updates,
   missing artifacts, rejected writes, and content redaction.
3. Test context API clone/export/import/delete, profile usage/quota paths, and
   retention against broker control, including active-writer and absent-profile
   cases.
4. Run gateway, runtime contract/client/broker, full workspace, strict Clippy,
   Rustdoc, formatting, dependency, repository-document, Compose, and overlay
   topology checks.
5. Start the broker overlay; run the authenticated broker storage smoke, then
   create/import/clone/export/delete a browser context and start a session with
   workspace files through gateway APIs. Verify session startup, file content,
   usage evidence, cleanup, and that no gateway-owned storage-helper container
   is launched.

Manual checkpoint: clone/export/import a context, bind a workspace file, run a
session, and verify storage usage and cleanup through the broker. Removing the
gateway from the Docker-control network remains checkpoint 6.

Slice 3 evidence:

- `DockerRuntimeManager` derives storage control from browser control, so
  direct and broker lifecycle/storage modes cannot be mixed accidentally.
  `broker_pool` routes initialization, four typed file destinations,
  clone/export/import/measure, retention deletion, and owned-volume cleanup
  through `RuntimeBrokerClient::execute_storage`.
- Workspace bytes and binding state remain gateway-owned. Deterministic tests
  cover read-only/read-write binding writes, manifest ordering, state
  transitions, rejected manifests, active-context exclusion, malformed broker
  results, transport failures, empty files, and content redaction.
- Input-bearing helpers use a broker-derived request-scoped staging volume and
  Docker's bounded archive upload route rather than attach stdin. The helper
  verifies the declared byte count before consumption; helper and staging
  volumes are removed after success, failure, or timeout. This preserves
  realistic Chromium profile import performance without accepting host paths.
- The host helper image pre-creates the fixed unprivileged session-data layout,
  including the first-use reusable-context path, so Docker volume initialization
  does not introduce root-owned nested directories.
- Live validation passes for a 94 MB browser-context export/import with restored
  profile state, clone, reconnect, quota, malformed/archive-link denial, and
  cleanup. The gateway compose workspace test observes a file binding changing
  from `pending` to `materialized` during broker-backed runtime startup.
- Browser/CLI session files, admin-new session navigation/lifecycle/MCP/popup,
  workflow workspace execution, and recording/playback/export smokes pass on
  the broker overlay. The dedicated broker storage smoke uses a 4 MB
  incompressible archive and verifies every storage action plus helper,
  staging, session, and context volume cleanup.
- README, ARCH, and AGENTS now describe broker-owned storage call sites. The
  gateway intentionally retains its Docker-control network only until
  checkpoint 6 proves and switches the broker-only topology.

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

Checkpoint 6 implementation slice:

- Treat `deploy/compose.runtime-broker.yml` as the production-like Docker-host
  topology while retaining base `deploy/compose.yml` as the explicit local
  direct/proxy compatibility topology.
- Override the base gateway service so broker mode has no `DOCKER_HOST`, no
  `docker-control` network membership, and no startup dependency on
  `docker-proxy`. Preserve only the private broker API path and the application
  networks required by gateway features.
- Tighten the overlay contract so only `runtime-broker` and `docker-proxy` join
  `docker-control`, no other service gains proxy reachability, and the gateway
  cannot carry a Docker socket mount or generic Docker endpoint configuration.
- Add a live isolation smoke that inspects the effective container
  configuration and proves proxy DNS, proxy TCP access, and the Docker daemon
  are unavailable from the gateway while broker readiness remains available.
- Exercise browser lifecycle, reusable-context/session-file storage, workflow,
  recording, restart/reconciliation, and denial paths on the isolated overlay.
  Record the exact test evidence before marking the issue complete.

Checkpoint 6 example use case:

An attacker obtains code execution inside `bpane-gateway` and attempts to call
the Docker API directly. In the production-like broker topology the gateway has
neither a socket, a Docker endpoint variable, nor network reachability to the
proxy. Valid typed operations still reach `runtime-broker`, where BrowserPane
policy validates them before the broker uses the isolated Docker-control path.

Checkpoint 6 focused smoke sequence:

1. Render and validate the merged compose model; assert gateway Docker inputs
   are absent and `docker-control` contains only broker and proxy.
2. Start the isolated broker topology and prove the running gateway cannot
   resolve or connect to `docker-proxy`, has no Docker socket, and can reach
   broker readiness.
3. Run broker auth/policy denial tests and verify no helper, worker, browser, or
   staging residue remains.
4. Run browser lifecycle/reconnect, MCP, context/session-file, workflow,
   recording, admin-new, CLI, and complete compose API regressions.
5. Restart gateway and broker independently and verify persisted assignment,
   queued workflow, recording, and runtime reconciliation behavior.

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
