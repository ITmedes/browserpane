# BPANE-00150 Gateway Lifecycle And Readiness Plan

## Metadata

- Issue: [#150](https://github.com/ITmedes/browserpane/issues/150)
- State: Review
- Owner: `thebackplane`
- Lane: Foundation
- Target gate: Foundation Gate
- Branch: `feature/BPANE-00150`
- Depends on: #147 workflow webhook SSRF controls
- Last verified: 2026-08-04 on `feature/BPANE-00150` through the full validation profile

## Business Outcome

Give operators and schedulers a trustworthy distinction between a gateway
process that is alive and one that can safely accept new sessions or workflow
work. During a deployment restart, BrowserPane must stop accepting new work,
allow active HTTP and WebTransport work a bounded drain window, and terminate
predictably instead of relying on abrupt process destruction.

## Example Use Case

A deployment controller sends SIGTERM while operators have live browser
connections and workflow workers are polling the gateway. `/readyz` becomes
unavailable immediately, new API and WebTransport work is rejected, and active
connections receive the configured drain interval. The process exits when the
connections complete or when the interval expires. After restart, the
controller only sends traffic once Postgres, the configured runtime backend,
Vault, and local artifact stores report ready.

## Current Evidence

- `GatewayApp::run` selects between two unbounded server futures; when one
  exits, the other is dropped without coordinated shutdown.
- The API uses `axum::serve` without `with_graceful_shutdown`.
- The WebTransport server accepts forever and detaches every session task.
- SIGINT and SIGTERM have no application-level lifecycle handling.
- `/healthz` and `/readyz` do not exist.
- Compose and its e2e wrapper infer readiness from an authenticated catalog
  request, which cannot explain which dependency is unavailable.

## Lifecycle Contract

Use one shared gateway lifecycle coordinator with explicit `starting`,
`running`, and `draining` states.

- `/healthz` is unauthenticated and returns HTTP 200 while the process can
  serve HTTP, including during bounded drain.
- `/readyz` is unauthenticated, returns HTTP 200 only while lifecycle state is
  `running` and every required dependency passes, and otherwise returns 503.
- Readiness responses expose bounded dependency names, status, and sanitized
  reasons only. They never expose connection strings, tokens, paths containing
  secrets, or backend response bodies.
- The first SIGINT/SIGTERM transition is idempotent and starts drain. A second
  signal forces immediate termination.
- API middleware rejects new non-health requests with 503 after drain starts,
  while health probes remain available for a short configurable readiness
  withdrawal grace and Axum then drains requests already in flight.
- WebTransport stops accepting handshakes when drain starts. Existing session
  tasks may finish naturally until the configured deadline, after which the
  endpoint and remaining tasks are closed.
- Background reconciliation, retention, recording, workflow, and event
  delivery task cancellation is recorded as a follow-up unless the current
  manager already exposes a bounded shutdown boundary. This slice must not
  imply that external worker jobs are drained by the gateway.

## Readiness Dependencies

Required checks are derived from configured components rather than a static
list:

- `session_store`: in-memory is immediately available; Postgres executes a
  bounded `SELECT 1` through the existing pool.
- `runtime_manager`: `static_single` verifies its configured agent socket;
  Docker-backed modes execute a bounded daemon availability probe through the
  configured Docker CLI.
- `credential_provider`: omitted when Vault is not configured; Vault uses its
  standard health endpoint without reading a secret.
- `recording_artifact_store` and `workspace_file_store`: the local filesystem
  implementations verify that their roots can be created and written.

All checks run concurrently behind one configured per-check timeout. A timeout,
task failure, or failed check makes readiness fail closed while liveness stays
available.

## Implementation Slices

### Slice 1: Lifecycle Coordinator And Configuration

- Add a focused lifecycle module with typed state, signal handling, transition
  notification, and bounded drain configuration.
- Add CLI options for readiness timeout and shutdown drain timeout with
  conservative defaults, plus a shorter readiness-withdrawal grace before the
  HTTP listener closes.
- Add state-machine and configuration tests.

### Slice 2: Dependency Readiness Boundary

- Add narrow readiness methods to the session store, session manager,
  credential provider, recording artifact store, and workspace file store.
- Keep backend-specific I/O behind those existing facades.
- Aggregate checks concurrently with sanitized results and timeout handling.

### Slice 3: HTTP And WebTransport Integration

- Add unauthenticated `/healthz` and `/readyz` routes.
- Reject new API work after drain begins and use Axum graceful shutdown.
- Make the WebTransport accept loop lifecycle-aware and retain task ownership
  through bounded drain instead of detaching sessions permanently.
- Add lifecycle logs for signal, readiness failure, drain start, completed
  sessions, timed-out sessions, and process exit.

### Slice 4: Compose, Validation, And Documentation

- Add a gateway compose healthcheck based on `/readyz`.
- Replace the authenticated API readiness workaround in the compose e2e
  wrapper.
- Add focused API, dependency, shutdown, and compose tests.
- Update README, ARCH, AGENTS, roadmap, capability, risk, and validation docs
  where the operational contract changes.

## Test Strategy

### Unit

- Lifecycle transitions are monotonic and repeated drain requests are
  idempotent.
- Readiness is false during `starting` and `draining`.
- Empty optional dependencies do not create phantom failures.
- Individual failure, timeout, and task failure make the aggregate not ready.
- Response reasons remain bounded and sanitized.

### Integration

- `/healthz` returns 200 without authentication.
- `/readyz` returns 200 for healthy configured dependencies and 503 with a
  stable JSON body for a failed dependency or draining lifecycle.
- Non-health API requests receive 503 after drain begins.
- In-flight Axum requests receive the configured graceful window.
- The WebTransport accept loop stops admitting new handshakes and waits for
  tracked session tasks only until the configured deadline.

### Smoke And E2E

- Compose reports the gateway healthy through `/readyz`.
- Stopping Postgres makes `/readyz` return 503 while `/healthz` remains 200;
  restoring Postgres returns readiness to 200.
- Making Vault or Docker unavailable produces the corresponding sanitized
  failed check.
- SIGTERM makes readiness fail before process exit and the service restarts
  cleanly.
- Existing authenticated compose APIs, admin auth/session reconnect, workflow,
  recording, and MCP smoke paths remain functional after restart.

## Rollout And Compatibility

- The new routes are additive and unauthenticated by design; they expose no
  owner or resource data.
- Existing API paths and WebTransport credentials remain unchanged while the
  process is running.
- Deployment probes should use `/readyz` for traffic admission and `/healthz`
  only for process liveness.
- Default drain and readiness timeouts can be overridden through CLI flags.
- No database migration or persisted-resource rollback is required.

## Definition Of Done

- SIGINT and SIGTERM trigger one coordinated, bounded gateway drain.
- API and WebTransport listeners stop admitting new work during drain.
- Active HTTP and WebTransport work is tracked until completion or timeout.
- `/healthz` and dependency-aware `/readyz` have stable, sanitized responses.
- Postgres, runtime manager, configured Vault, and both local artifact stores
  participate in readiness.
- Compose and its e2e wrapper consume the new readiness contract.
- Focused unit/integration tests, gateway tests, compose smoke, and affected
  browser/worker regression smokes pass.
- Issue #150 and the canonical roadmap match implementation evidence.

## Post-Implementation Smoke Sequence

1. Run formatting, strict changed-code Clippy, gateway tests, workspace tests,
   dependency safety, and Rust coverage.
2. Rebuild and start local compose; wait for `GET /readyz` instead of an
   authenticated resource request.
3. Verify unauthenticated `GET /healthz` and `GET /readyz` response schemas and
   status codes.
4. Stop Postgres, Vault, and Docker access one at a time; verify the named
   readiness check fails without leaking backend credentials or response data,
   while `/healthz` remains live.
5. Restore each dependency and verify readiness recovers without restarting the
   gateway where the backend permits it.
6. Open an admin browser session and start a workflow/recording path, then send
   SIGTERM to the gateway container.
7. Verify readiness turns 503, new API and WebTransport attempts are rejected,
   active work receives the bounded drain window, and lifecycle logs identify
   completed versus timed-out work.
8. Restart the gateway and run session reconnect, MCP delegation, workflow,
   recording, and compose API regression smokes.

## Implementation Evidence

Implemented on `feature/BPANE-00150`:

- shared monotonic lifecycle state and coordinated SIGINT/SIGTERM drain,
- public `/healthz` and dependency-aware `/readyz`,
- bounded Postgres, runtime-manager, configured Vault, and local artifact-store
  checks,
- HTTP admission rejection and graceful shutdown during drain,
- WebTransport accept withdrawal with owned connection-task drain,
- compose readiness health check and readiness-based e2e preflight,
- focused tests plus compose Postgres loss/recovery and real SIGTERM/restart
  smoke evidence.

Final validation on 2026-08-04:

- `node scripts/validate.mjs --profile full` passed all 46 stages,
- Rust formatting, workspace Clippy, workspace tests, dependency policy, and
  the Rust coverage ratchet passed,
- 393 gateway tests passed within the workspace suite,
- all maintained Node packages passed their configured checks, tests,
  coverage ratchets, and builds,
- 17 default compose API surfaces and 4 docker-pool lifecycle/capacity surfaces
  passed,
- auth/security, admin-new dashboard/projects/sessions, compatibility admin,
  CLI, MCP multi-session, recording artifact/playback, and workflow-admission
  browser smokes passed,
- the dedicated admin-event reconnect smoke passed across an actual gateway
  restart with a fresh scoped token, query-free WebSocket URL, realtime session
  synchronization, and credential-free gateway logs,
- the restored local stack reports `/healthz` 200, `/readyz` 200, and a healthy
  gateway container.
