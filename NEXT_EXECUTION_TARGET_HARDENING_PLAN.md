# Next Execution Target Hardening Plan

Date: 2026-05-04

Status: implementation in progress.

GitHub tracking issue: `#65`
https://github.com/ITmedes/browserpane/issues/65

## Purpose

This plan consolidates the next five BrowserPane execution-target priorities
into one implementation roadmap. The goal is to reduce fragmented issue
context after the docker runtime/session data isolation work and keep the next
slices focused on production hardening, regression visibility, and integration
correctness.

The previous `#62` slice established the first required runtime boundary:
docker-backed browser sessions now use separate socket and session data roots,
and workspace-backed session file bindings can be materialized before runtime
startup. This plan starts from that state.

## Priority Order

### 1. Browser Local File Access Policy

Source issue: `#57`

Implementation status: completed on `feature/BPANE-0065`.

Commits:

- `ecd6703 feat(host): deny local browser file access by default`
- `6748ac3 test(browser): verify local file policy at runtime`

Validation completed:

- `cargo fmt --all -- --check`
- `cargo clippy -p bpane-gateway --all-targets --all-features -- -D warnings`
- `cargo test -p bpane-gateway --test start_host_browser_policy`
- `cargo test -p bpane-gateway --test start_host_runtime_paths`
- `cd code/web/bpane-client && npm run build`
- `cd code/web/bpane-client && npm run smoke:browser-policy -- --headless`
- `cd code/web/bpane-client && npm run smoke:test-embed-lifecycle -- --headless`

Goal: default browser sessions must not be able to read arbitrary local files
through `file:///` navigation or browser file-system APIs.

Implementation targets:

- Extend the managed Chromium policy while preserving extension policy.
- Block `file:///*` by default.
- Deny File System Access API read/write prompts by default.
- Add an explicit development escape hatch only if required for compatibility.
- Expose the effective policy mode in diagnostics or session status.
- Add test harness probes for blocked local-file access.

Acceptance criteria:

- A default docker-backed session cannot display `file:///etc/passwd`.
- File System Access API read/write access is denied by default.
- Existing extension policy behavior still works.
- The test harness can show the effective file access mode and probe result.

### 2. Complete Session File Data Visibility

Source issue: `#56`

Implementation status: completed on `feature/BPANE-0065`, with optional
follow-ups listed below.

Completed scope:

- Session-scoped file metadata/resource model for runtime uploads and downloads.
- Postgres migration and in-memory/Postgres store support for session files.
- Owner-scoped read APIs:
  - `GET /api/v1/sessions/{session_id}/files`
  - `GET /api/v1/sessions/{session_id}/files/{file_id}`
  - `GET /api/v1/sessions/{session_id}/files/{file_id}/content`
- Browser upload recording from completed `CH_FILE_UP` transfers.
- Browser download recording from completed `CH_FILE_DOWN` transfers before
  gateway fan-out.
- Configured session-file retention cleanup for expired runtime file artifacts
  and metadata.
- `dev/test-embed.html` Session Files panel.
- Compose-backed browser smoke for upload visibility and authenticated content
  download.

Commits:

- `69337d7 feat(gateway): track uploaded session files`
- `ed53828 feat(gateway): record downloaded session files`
- `e4789d0 test(client): add session file harness smoke`
- `7b7a451 feat(gateway): clean up retained session files`

Validation completed:

- `cargo fmt --all -- --check`
- `cargo clippy -p bpane-gateway --all-targets --all-features -- -D warnings`
- `cargo test -p bpane-gateway`
- `cargo test -p bpane-gateway session_files::retention`
- `cd code/web/bpane-client && node --check scripts/run-session-files-smoke.mjs`
- `cd code/web/bpane-client && npm run build`
- `cd code/web/bpane-client && npm run smoke:session-files -- --headless`

Follow-up candidates:

- Decide whether runtime session files need a dedicated artifact store instead
  of the current pragmatic workspace-file-store backed implementation.
- Add stronger download e2e coverage if a host-triggered download fixture is
  introduced.
- Keep API-level session A/B isolation covered and add full browser-harness
  isolation only if regression risk justifies the runtime cost.

Goal: browser uploads and downloads should become attributable session state,
not only runtime-local scratch files or immediate transport events.

Implementation targets:

- Keep the current per-session docker data volume boundary.
- Teach browser upload/download finalization to report session file metadata to
  the gateway or a session-scoped API.
- Persist session file metadata for upload/download origins.
- Decide whether `CH_FILE_DOWN` remains an immediate convenience channel or
  becomes a notification that a control-plane file is ready.
- Add digest, byte count, media type, source, session id, owner, and timestamp.
- Add cleanup or retention rules for scratch session files.
- Add a `dev/test-embed.html` Session Files panel for manual and smoke testing.

Acceptance criteria:

- Browser uploads and downloads are visible through session-scoped APIs.
- Session A cannot list or download Session B files.
- Workspace-backed files are materialized only when explicitly bound.
- The harness can exercise bindings, uploads, downloads, and isolation without
  direct Docker inspection.

### 3. MCP Delegation Drift And Session-Bound Control

Source issue: `#54`

Implementation status: in progress on `feature/BPANE-0065`.

Completed scope:

- Switching the local bridge from session A to session B now releases live MCP
  ownership for A before caching automation access for B.
- `dev/test-embed.html` refreshes bridge/session state before delegation and
  clears previous bridge delegates after the bridge adopts the new session.
- The multi-session smoke refetches both session resources and statuses after
  a bridge switch and fails if session A remains backend-delegated or
  MCP-owned.
- `/health` now exposes explicit control-plane/backend delegation visibility
  and a `bridge_alignment` value so stale, split, or endpoint-mismatched bridge
  state is machine-checkable.
- The multi-session smoke asserts the bridge reports `aligned` health after
  switching delegation to session B.
- The multi-session smoke now opens a real MCP streamable HTTP client, calls
  `browser_navigate`, and verifies the side effect lands in the delegated
  session's Chromium runtime instead of the previous session.

Commits:

- `bcd85f9 fix(mcp): clear stale delegation on session switch`
- `586b93e feat(mcp): expose bridge alignment health`
- `5bce3e0 test(client): verify mcp actions target delegated session`

Validation completed:

- `cd code/integrations/mcp-bridge && npm run build`
- `cd code/web/bpane-client && node --check scripts/run-multi-session-smoke.mjs`
- `cd code/web/bpane-client && npm run build`
- `cd code/web/bpane-client && npm run smoke:multisession -- --headless`

Remaining scope:

- Keep the per-connection multi-session MCP model as the longer-term design.

Goal: delegation must not leave the MCP bridge silently controlling the wrong
BrowserPane session.

Completed implementation targets:

- Stabilize existing single-session `/control-session` switching first.
- Clear previous session MCP ownership when switching from session A to B.
- Improve UI messaging so "gateway delegated" and "bridge adopted" are distinct.
- Surface split or stale bridge state through `/health`.
- Add regression coverage that refetches both session statuses after a switch.
- Add at least one real MCP tool-call side effect test to verify it lands on
  the intended session.

Remaining implementation target:

- Keep the per-connection multi-session MCP model as the longer-term design.

Acceptance criteria:

- Delegating session B cannot leave the bridge silently driving session A.
- Stale MCP ownership is cleared from the previous session after a switch.
- The smoke suite catches wrong-session routing through a real action, not only
  through metadata checks.

### 4. Remote Deployment Documentation

Source issue: `#53`

Implementation status: completed on `feature/BPANE-0065`.

Completed scope:

- Added [REMOTE_DEPLOYMENT.md](REMOTE_DEPLOYMENT.md) with remote HTTPS,
  WebTransport certificate, OIDC alignment, compose override, runtime mode, and
  dev-service exposure guidance.
- Added a README pointer from the local development section to the remote
  deployment notes.

Commits:

- `b87d7f0 docs: add remote deployment notes`

Validation completed:

- `git diff --cached --check`

Goal: document the deployment assumptions that differ between localhost compose
and remote/self-hosted testing.

Implementation targets:

- Add README documentation for remote HTTPS and secure-context requirements.
- Document OIDC issuer, redirect URI, web origin, gateway URL, and certificate
  metadata alignment.
- Warn against exposing dev Postgres, Vault, Keycloak, gateway API, and
  MCP bridge ports publicly.
- Document `sudo env ... docker compose ...` override behavior.
- Reference docker-pool runtime defaults and current local testing assumptions.

Acceptance criteria:

- A user moving from localhost to a remote host can identify the required HTTPS,
  OIDC, and WebTransport configuration changes.
- The docs clearly state that local compose is not production deployment
  guidance.

### 5. Admin App And Real-Time Operations Slice

Source issue: `#63`

Goal: reduce the broad admin-app issue into implementable slices without
starting a large, unfocused rewrite.

Implementation targets:

- Keep `#63` as the broad product umbrella.
- Start with a narrow reference-admin foundation slice only after the security
  and integration hardening items above are complete.
- Proposed first admin slice:
  - scaffold `code/web/bpane-admin`;
  - preserve local OIDC and cert metadata behavior;
  - add typed REST client boundaries;
  - show session list/detail;
  - embed the live browser client through `bpane-client`;
  - port the lifecycle smoke coverage that currently depends on
    `dev/test-embed.html`.
- Defer the full real-time WebSocket/event-stream work until the first admin
  shell proves the resource model and smoke parity.

Acceptance criteria for the first admin slice:

- The admin app can authenticate in local compose.
- It can list sessions and open a live session detail view.
- It does not duplicate `bpane-client` internals.
- It has initial smoke coverage for session lifecycle actions.
- `dev/test-embed.html` remains available until smoke parity is achieved.

## Issue Consolidation Plan

The new consolidated issue should become the short-term execution tracker for
this file.

Recommended issue actions:

- Close `#57` as superseded by priority 1 in the consolidated issue.
- Keep `#56` open only if the team wants it as a broader file-data epic;
  otherwise close it as superseded by priority 2 and reference `#62` for the
  completed runtime-boundary part.
- Close `#54` as superseded by priority 3 if the consolidated issue will own
  MCP delegation stabilization.
- Close `#53` as superseded by priority 4 if the documentation task is small
  enough to remain in this roadmap.
- Keep `#63` open as the product umbrella and add a comment that this plan only
  covers the first admin-app slice.

## Validation Strategy

Run narrow checks per slice, then finish with the compose e2e and browser smoke
coverage relevant to the changed surface.

Expected validation set:

- `cargo fmt --all -- --check`
- `cargo clippy -p bpane-gateway --all-targets --all-features -- -D warnings`
- `cargo test -p bpane-gateway`
- `cargo test --workspace`
- `scripts/run-gateway-compose-e2e.sh --suite all`
- `cd code/web/bpane-client && npx tsc --noEmit`
- `cd code/web/bpane-client && npm test`
- `cd code/web/bpane-client && npm run build`
- Targeted Playwright smokes for file-policy, session-files, MCP delegation, and
  session lifecycle behavior.

## Open Questions

- Should approved session files ever be opened through `file:///`, or should all
  approved file access use authenticated BrowserPane HTTP URLs?
- Should browser downloads default to session-scoped artifacts or workspace
  files?
- Should the MCP parallel-session endpoint be implemented in the same slice as
  single-session drift stabilization or in a later issue?
- Should the first admin app slice wait for a gateway event stream, or should it
  start with REST reconciliation and add WebSocket synchronization later?
