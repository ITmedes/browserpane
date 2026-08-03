# Runtime And Operator Requirements

This document consolidates the still-valid runtime lifecycle, local setup,
certificate, MCP, and operator CLI requirements.

## Local Workflow, MCP, Certificate, And Setup

Goal: local development must be deterministic enough that an operator can start
compose, create workflow versions from the bind-mounted checkout, delegate MCP,
recover from certificate changes, and diagnose setup failures without reading
old plans.

Required behavior:

- the gateway must allow the local `/workspace` bind mount explicitly through
  its workflow source trusted-local-root policy,
- the source resolver must apply short-lived, repository-scoped
  `safe.directory` entries only after a local path passes trusted-root
  validation; custom deployments must not require process-wide Git trust,
- workflow source failures must distinguish invalid input, git resolution,
  repository access, materialization, archive/snapshot, and worker/source
  infrastructure failures,
- admin workflow errors should show setup/repository/source/worker categories
  instead of a generic gateway failure,
- the MCP bridge must use the pinned local `@playwright/mcp` dependency
  installed by `npm ci`,
- first MCP connect must not download `@playwright/mcp@latest`,
- a missing local MCP executable should fail early with clear logs/errors,
- `/cert-hash` and `/cert-fingerprint` must remain available for local
  WebTransport trust diagnostics,
- certificate metadata should not be stale across connection attempts,
- certificate rotation/opening-handshake failures should lead operators to the
  gateway URL, local QUIC trust guidance, and SPKI fingerprint helper.

Local troubleshooting runbook topics to preserve in README/setup docs:

- Docker socket activation checks and recovery commands,
- sudo-safe environment passing for local compose runs,
- compose buildx/Bake warning behavior,
- HTTP API port usage versus WebTransport/TLS endpoints,
- API-only `docker_pool` lazy start behavior,
- database owner-column diagnostic queries where relevant,
- WebTransport dev certificate lifetime, hash, and SPKI expectations,
- camera/v4l2loopback provisioning and unsupported-state diagnosis,
- `control_sessions.runtime_binding` versus `control_session_runtimes`
  lifecycle semantics,
- avoiding root-owned generated files in the checkout.

Validation:

- `cargo test -p bpane-gateway`
- focused workflow source tests,
- `cd code/integrations/mcp-bridge && npm test && npm run build`
- `cd code/web/bpane-client && npx tsc --noEmit`
- `cd code/web/bpane-client && npm run smoke:admin-browserpane-tour -- --headless`
- `cd code/web/bpane-client && npm run smoke:admin-mcp -- --headless`
- bridge logs should not contain `npm warn exec` or `@playwright/mcp@latest`
  after first connect.

## BPM Workflow Endpoint Operations

Issue `#172` introduces a machine-facing long-running activity boundary above
the owner workflow-run API. Before an endpoint is marked active, operators must
be able to verify:

- Postgres, runtime dispatch, worker image, credential provider, artifact store,
  and callback delivery dependencies are ready,
- the bound immutable workflow and endpoint revision pass contract validation,
- project policy, endpoint grants, caller limits, deadlines, and result limits
  are effective,
- process-variable mapping, connector credential storage, target-system
  Credential Bindings, and Human Handoff ownership are separated,
- callback destinations pass the controls owned by `#147`,
- the selected polling, webhook, or callback-token profile has a passing
  conformance result,
- dev/test/prod resources and credentials do not cross environment boundaries.

Operational evidence must distinguish:

- accepted and queued work,
- endpoint/caller throttling,
- dependency unavailability or maintenance,
- running progress and last worker heartbeat,
- requested and acknowledged cancellation,
- terminal outcome and retryability,
- attempt/checkpoint and external-side-effect uncertainty,
- per-run event sequence and delivery replay/reconciliation health,
- callback retry/dead-letter/redelivery,
- artifact expiry or authorization failure.

The integration must not keep the original HTTP invocation open for the full
browser run. Callback tokens and upstream connector credentials are sensitive
references and must not appear in labels, logs, event payloads, diagnostics, or
Admin-New.

Validation:

- dependency-aware readiness and overload tests,
- restart/reconciliation with idempotent invocation,
- poll, signed webhook, and callback-token conformance smokes,
- promotion/rollback and environment-isolation tests,
- structured log/trace correlation from invocation through artifact/callback.

## Session Runtime Lifecycle

Runtime lifecycle states must tell operators whether a session has:

- an exact live runtime,
- a released runtime that can restart from profile-backed state,
- a terminal stopped state,
- a killed/failed cleanup state,
- a queued state that is not connectable yet.

Required release/reconnect semantics:

- `POST /api/v1/sessions/{session_id}/release` releases the live runtime while
  preserving the session resource, labels, files, recordings, and profile
  state,
- release is distinct from stop and kill,
- released sessions remain reconnectable,
- reconnect after release should report `profile_restart` or equivalent,
- API/status payloads should preserve `runtime_resume_mode` or the equivalent
  runtime-resume field when rendered,
- stopped sessions remain terminal and cannot mint new connect tickets,
- stale tabs or stale access-token issue flows must not silently reactivate a
  stopped session,
- explicit owner action is required for any profile-backed restart after stop,
- runtime assignment reconciliation must not map a selected session to the wrong
  stale/missing Docker runtime,
- stop and release are blocked while eligible owner clients or blockers are
  connected,
- queued sessions must be visible but non-connectable until promoted.

Admin requirements:

- show release, stop, kill, cancel queued, disconnect one connection, and
  disconnect-all with eligibility and blocker reasons,
- distinguish live runtime, released runtime, stopped session, queued session,
  and profile-backed restart in list/detail views,
- avoid letting stopped sessions dominate the inspector list,
- keep cleanup explicit; do not auto-delete resources with retained artifacts
  or recordings,
- reconnect visual bootstrap must not leave persistent black tile/canvas state.

Manual release/reconnect smoke:

1. Start compose with Postgres, Keycloak, gateway, web, and Docker runtime pool.
2. Open the admin app and sign in with `demo / demo-demo`.
3. Create and connect a session.
4. Confirm state is active/running and resume mode is exact live.
5. Disconnect all browser clients.
6. Release runtime.
7. Confirm state/runtime is released and a release timestamp is visible.
8. Reconnect intentionally.
9. Confirm the browser renders visible content and resume mode is
   profile-backed restart.
10. Disconnect and stop the session.
11. Confirm stopped sessions cannot be joined or issued connect tickets.
12. Switch between active, released, queued, and stopped sessions and confirm
    browser runtime state never crosses selected-session boundaries.

API release/reconnect smoke:

1. Create a session through `POST /api/v1/sessions`.
2. Release it through `POST /api/v1/sessions/{id}/release`.
3. Verify the resource reports released state, released runtime status, and no
   `stopped_at`.
4. Mint a connect ticket and verify `token_type=session_connect_ticket`.
5. Fetch the session and confirm `runtime_resume_mode` is `profile_restart` or
   equivalent.
6. Stop the session.
7. Attempt another access-token mint and verify `409 Conflict`.

Validation:

- `cargo test -p bpane-gateway`
- `cd code/web/bpane-admin && npm test && npm run check && npm run build`
- `cd code/web/bpane-client && npx tsc --noEmit`
- `cd code/web/bpane-client && npm run smoke:admin-session -- --headless`
- Docker-pool compose API suite when runtime behavior changes.

## Operator CLI

The supported local `bpane` CLI is the automation-safe operator surface for
session inspection, session cleanup, MCP delegation, profiles, and identity
checks.

Required behavior:

- use `BPANE_BASE_URL` or `BPANE_API_URL`, defaulting to local web/gateway,
- use `BPANE_ACCESS_TOKEN` plus `--token` and `--access-token` aliases,
- use `BPANE_MCP_CONTROL_URL` where direct bridge control is still relevant,
- print JSON success responses and structured JSON errors,
- return stable non-zero exit codes for usage errors, auth failures, HTTP
  failures, and strict preflight issues,
- reject unsupported options so typos do not silently change behavior,
- preserve inline option values containing `=` for labels and JSON payloads,
- keep cleanup dry-run by default and require explicit confirmation for
  destructive actions,
- profile config precedence is flags, environment, profile values, defaults,
- profile token persistence is explicit and config writes enforce `0600`.

Required command families:

- sessions: list, get, status, create, stop, release where supported, kill,
  access-token, automation-access, disconnect-all, cleanup, cancel queued,
- MCP: health, authorize, revoke, set-default, clear-default, doctor,
  preflight, repair,
- projects, browser contexts, egress profiles, file workspaces, session
  templates, identity, service principals, and other resource commands where
  API support exists,
- profiles: list, show, init.

MCP doctor/preflight/repair requirements:

- check bridge health and control-session reachability,
- with a session id, check visibility, session state, automation delegate,
  MCP owner state, and default bridge alignment,
- `mcp doctor` is interactive-friendly and only fails with `--fail-on-issues`,
- `mcp preflight` is strict for automation and exits non-zero on issues,
- `mcp repair` must refuse mutation when the session is not visible to the
  current owner token,
- repair output remains machine-readable and includes skipped actions.

CLI smoke:

1. Start local compose.
2. Run `cd code/web/bpane-client && npm run smoke:bpane-cli -- --headless`.
3. Manually check `npm run bpane:cli -- --help`.
4. Export an admin bearer as `BPANE_ACCESS_TOKEN`.
5. Run `bpane session list`, `bpane session status <session-id>`, MCP
   health/authorize/set-default/clear-default/revoke, and stop/kill against a
   disposable session.

Acceptance:

- commands send the expected v1 owner-scoped API requests,
- MCP commands use the configured gateway/bridge control path,
- missing auth, usage errors, and HTTP failures are machine-readable,
- CLI unit tests and local smoke pass.
