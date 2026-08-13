# BPANE-00167 Docker Runtime Boundary Plan

Issue: [#167 Define Docker runtime launch boundary for production hardening](https://github.com/ITmedes/browserpane/issues/167)

Status: implemented and validated; awaiting merge

## Business Case

The gateway currently receives the host Docker socket directly so it can launch
browser runtimes, workflow workers, recording workers, and short-lived helpers
for browser-context and session-file storage. A compromise of the public gateway
therefore also grants direct access to the host Docker daemon. That is acceptable
as an explicit local-development shortcut, but it is not a suitable production
trust boundary.

This slice must reduce the gateway's direct daemon exposure without breaking the
existing Docker-backed product path. It must also avoid overstating what an API
allowlist can guarantee: permission to create containers remains powerful because
Docker endpoint filtering does not validate image, mount, network, capability, or
privileged-mode fields inside a container-create request.

## Example Use Case

An operator runs BrowserPane on a dedicated Docker host for a controlled pilot.
The gateway can create and stop BrowserPane browser, workflow, and recording
containers through an internal-only Docker API proxy, but it cannot call unrelated
daemon APIs such as system administration, builds, swarm, secrets, services, or
plugins. The raw socket is mounted only into the proxy. The deployment
documentation makes clear that a purpose-specific launch broker or orchestrator
adapter is still required when the gateway itself is inside the production threat
boundary. That stronger boundary must also constrain resource visibility and
validate the content of permitted container and volume requests.

## Decision

1. Keep raw socket access documented as a local-development-only option.
2. Add an isolated, pinned Docker socket proxy as the default compose boundary:
   - no host port,
   - a private network shared only with the gateway,
   - explicit endpoint and method allowlists,
   - the gateway receives `DOCKER_HOST` instead of the socket mount.
3. Treat this proxy as defense-in-depth and a production-like validation topology,
   not as a complete authorization boundary.
4. Stage a purpose-specific runtime launch broker or non-Docker orchestrator
   adapter as the production target. That boundary must validate BrowserPane-owned
   images, names, networks, mounts, resource limits, and lifecycle operations.
   Follow-up implementation is owned by
   [#214](https://github.com/ITmedes/browserpane/issues/214).

## Considered Alternatives

### Raw Docker socket

Retains current behavior with no isolation. It remains useful for local debugging
but is rejected as production guidance.

### Generic Docker socket proxy

Provides a practical migration step and blocks unrelated API families. It is
selected for this slice because it can preserve all current runtime behavior, but
it cannot constrain dangerous fields inside otherwise permitted create requests.

### Purpose-specific launch broker

Provides the strongest Docker-host boundary because BrowserPane can expose only
typed lifecycle operations and validate every launch field. It is the production
target, but extracting all browser, worker, browser-context, and session-file
operations is too large for this bounded slice.

### Kubernetes or cloud runtime adapter

Removes Docker-daemon coupling and maps lifecycle behavior to an orchestrator. It
remains the long-term deployment direction tracked by the runtime portability
work; it does not replace the immediate compose hardening needed here.

## Implementation Steps

### 1. Inventory and contract

- Inventory every Docker API family required by browser runtimes, workflow workers,
  recording workers, readiness, teardown, and volume-backed helper containers.
- Pin the proxy image by immutable multi-platform digest.
- Keep the runtime code dependent on the existing session-manager facade.

### 2. Compose boundary

- Add an internal Docker API proxy service with the raw socket mounted read-only.
- Place it on a dedicated internal network shared only with the gateway.
- Remove the raw socket mount from the gateway.
- Configure the gateway Docker CLI through `DOCKER_HOST`.
- Allow only the HTTP methods and API families required by the inventoried product
  operations.

### 3. Boundary validation

- Add a repeatable script that discovers the daemon API version through the proxy.
- Prove required readiness and scoped lifecycle endpoints remain reachable.
- Prove unrelated build, image mutation, system, swarm, secret, service, node,
  plugin, and broad host administration endpoints are denied.
- Record that the required container and volume API families remain broad enough
  to list unrelated resources and accept caller-supplied launch fields.
- Fail validation if the gateway regains a direct socket mount or the proxy gains a
  published host port.

### 4. Documentation and roadmap

- Update the architecture and operator documentation with the local raw-socket,
  proxy-hardened compose, and production broker/orchestrator boundaries.
- Document the proxy's request-body authorization limitation explicitly.
- Update the capability and delivery roadmap state without claiming that generic
  endpoint filtering makes Docker a production-safe multi-tenant boundary.
- Check whether `README.md` needs a concise local-runtime note.

### 5. Validation

- Run compose config validation and the negative boundary script.
- Run gateway unit tests for affected configuration/runtime behavior.
- Start the compose stack through the proxy boundary.
- Exercise browser session create/start/connect/reconnect/stop/delete.
- Exercise workflow worker launch and completion.
- Exercise recording worker launch, finalization, playback, and download.
- Exercise browser-context/session-file volume helpers where covered by canonical
  compose smokes.

## Acceptance Criteria

- The gateway has no direct Docker socket mount in the canonical compose topology.
- The proxy is digest-pinned, internal-only, and deny-by-default.
- Required BrowserPane Docker operations work through `DOCKER_HOST`.
- Unrelated Docker API families are demonstrably denied.
- Sessions, workflows, recordings, and storage helpers retain their current local
  behavior.
- Documentation accurately separates defense-in-depth from a production trust
  boundary.
- The follow-up broker/orchestrator extraction is explicit and independently
  actionable.

## Post-Implementation Smoke Sequence

1. Run `docker compose -f deploy/compose.yml config` and the runtime-boundary
   static checks.
2. Start the canonical stack and confirm gateway readiness through the proxy.
3. Run the Docker API negative test and confirm unrelated endpoints return denial.
4. Create, connect, reconnect, stop, restart, and delete a Docker-backed session.
5. Execute a workflow run and verify worker cleanup.
6. Enable recording, connect to the session, stop it, and download the finalized
   artifact.
7. Run browser-context and session-file compose coverage for helper-container and
   volume operations.
8. Run the targeted gateway unit/integration tests and impacted browser smokes.
9. Review `README.md`, `ARCH.md`, runtime requirements, and security roadmap claims
   against the final manifest.

## Validation Evidence

- `node scripts/validate-docker-runtime-boundary.mjs`: static compose contract and
  live required/denied Docker API checks passed.
- `node --test scripts/validate-docker-runtime-boundary.test.mjs`: 9/9 negative
  boundary contract tests passed.
- `cargo test -p bpane-gateway`: 435 passed, 1 environment-gated test ignored;
  all integration-test binaries passed their active tests.
- `cargo llvm-cov -p bpane-gateway --summary-only`: 56.13% line, 57.91%
  function, and 60.69% region coverage across the gateway target.
- `cargo fmt --all -- --check` and strict gateway clippy passed.
- `scripts/run-gateway-compose-e2e.sh --suite all`: Postgres store contract,
  17/17 default compose API surfaces, and 4/4 docker-pool lifecycle surfaces
  passed through the proxy boundary.
- Browser smokes passed for multi-session connect/join/MCP routing, unified-admin
  browser-context CRUD/clone/import/binding, recording finalization/playback/
  download, workflow execution, workspace-input policy, and the operator CLI.
- Browser client unit coverage passed with 674 tests and 92.88% line, 93.19%
  function, and 87.57% branch coverage; the production client build passed.
- `node scripts/check-repository-documents.mjs`: 71 Markdown, 8 YAML, and 3
  workflow files passed repository documentation validation.

The multi-session smoke exposed and now covers a pre-existing fixture regression:
the authenticated MCP bridge proxy was called without the owner bearer and its
health URL discarded the `/api/v1/mcp-bridge` prefix. Both calls now retain the
proxy path and owner authentication.

## Out Of Scope

- Claiming the generic proxy is a secure multi-tenant production sandbox.
- A complete runtime-launch broker implementation.
- Kubernetes, Fargate, Cloud Run, or other cloud runtime adapters.
- Replacing the Docker CLI with a new SDK solely for this boundary.
- Unrelated admin UI changes.
