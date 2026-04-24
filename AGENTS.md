# BrowserPane Agent Guide

This file is the shared project memory for BrowserPane. Keep it short, code-aligned, and current.

Project-wide Rust coding standards live in `RUST_STANDARDS.md`.
- Apply them to all Rust crates in this repo.
- Update that file instead of expanding this one with detailed Rust style rules.

Project-wide TypeScript and Node.js coding standards live in `NODEJS_STANDARDS.md`.
- Apply them to `code/web/bpane-client`, `code/integrations/mcp-bridge`, `code/integrations/recording-worker`, `code/integrations/workflow-worker`, and future TS/Node packages.
- Update that file instead of expanding this one with detailed TS/Node style rules.

When docs disagree, prefer:
1. The code
2. Runtime manifests and package scripts
3. This file
4. `README.md`

For the frozen owner-scoped session-control contract, use `openapi/bpane-control-v1.yaml`.

## What BrowserPane is

BrowserPane is a browser-native remote browser/desktop stack for a Linux host container.

Current product shape:
- A Linux container runs Xorg dummy + Openbox + Chromium.
- `bpane-host` captures and classifies the surface.
- `bpane-gateway` exposes WebTransport plus legacy and versioned HTTP APIs.
- Phase 0 session resources are persisted in Postgres behind the gateway.
- The browser client renders a tile-first stream with optional ROI H.264 video.
- Shared sessions are collaborative by default; optional exclusive-owner mode can lock later browser clients into read-only viewers.

## Current support matrix

- Host runtime: Linux only. Ubuntu 24.04 container is the primary target.
- Browser runtime: Chromium desktop only. Firefox and Safari are not production targets.
- Shared sessions: supported for small curated groups, not broadcast-scale delivery.
- Exclusive browser-owner mode: optional in `bpane-gateway` via `--exclusive-browser-owner`; default is disabled.
- Viewer cap: configurable in `bpane-gateway` via `--max-viewers`, default `10` when exclusive-owner mode or MCP ownership is active.
- MCP automation: supported via `mcp-bridge` and gateway ownership APIs.
- Browser extensions: owner-approved unpacked extensions are supported for docker-backed sessions and workflow runs; `static_single` does not support session extension sets.
- Camera ingress: disabled by default in compose; requires browser H.264 encode support and a mapped `v4l2loopback` device on the host.
- In exclusive-owner or MCP-owned sessions, restricted browser viewers are view-only: no input, clipboard, microphone, camera, upload, download, or resize.

## Architecture map

- `code/apps/bpane-host`
  - Linux host agent. Main orchestration lives in `src/main.rs`.
  - `capture/`: X11 capture and ROI video capture support.
  - `tiles/`: tile classification and Fill/QOI/zstd emission.
  - `audio/`: desktop audio out and microphone ingest.
  - `camera.rs`: H.264 browser camera ingress to virtual camera.
  - `clipboard.rs`, `filetransfer.rs`, `input/`, `resize.rs`: host-side interaction plumbing.
- `code/apps/bpane-gateway`
  - WebTransport gateway and shared-session coordinator.
  - `transport.rs`: browser connection loop, per-client policy, relay behavior.
  - `session_hub.rs`: fan-out, late-join bootstrap, viewer cap, telemetry.
  - `session_control.rs`: versioned session-control store and Postgres integration, including workflows, credential bindings, file workspaces, and approved extension metadata.
  - `session_manager.rs`: internal gateway boundary for session runtime lifecycle. The rest of the gateway should depend on this façade instead of backend details.
  - `credential_provider.rs`: credential binding secret-provider boundary. Local compose uses HashiCorp Vault dev mode and the current implementation targets Vault KV v2.
  - `workflow_source.rs`: workflow source contract and git ref resolution. Workflow definition versions can pin git-backed source metadata to an immutable commit at publish time without embedding source blobs into the control plane.
  - `file_workspace.rs`: owner-scoped file workspace and workspace-file resource shapes persisted by the control plane.
  - `workspace_file_store.rs`: workspace file content storage boundary. `local_fs` is the current implementation; workspace files carry opaque artifact refs plus optional provenance metadata instead of raw filesystem paths.
  - `recording_artifact_store.rs`: recording artifact storage boundary. `local_fs` is the current implementation; the gateway persists opaque artifact refs instead of raw filesystem paths.
  - `recording_lifecycle.rs`: recorder-worker launch, persisted assignment tracking, and restart reconciliation for session-scoped recording, including `recording.mode=always`. Recording resources are contiguous segments; restart recovery fails the stale in-flight segment and starts a linked fresh one instead of pretending the artifact is continuous.
  - `recording_playback.rs`: derives session-level playback/export resources from retained recording segments and packages a zipped playback bundle with manifest + player + included media files.
  - `recording_observability.rs`: gateway-local counters/timestamps for recording finalization, playback export generation, and retention passes.
  - `recording_retention.rs`: periodic cleanup of completed recording artifacts after the session-scoped retention window expires; it clears artifact refs but preserves recording segment metadata.
  - `workflow_lifecycle.rs`: control-plane launch/supervision for workflow workers. The gateway can auto-start Playwright workflow workers as short-lived Docker jobs, persist run-worker assignments, and fail stale active runs after restart instead of leaving them orphaned.
  - `workflow_observability.rs`: gateway-local counters/timestamps for workflow-produced file uploads and workflow retention passes.
  - `workflow_retention.rs`: periodic cleanup of retained workflow logs and structured outputs after the configured workflow retention windows expire.
  - `runtime_manager.rs`: current `SessionManager` backend implementation; supports `static_single`, `docker_single`, and `docker_pool`. The default stack still uses the single-runtime path; `docker_pool` adds explicit runtime caps for parallel session workers and can now be exercised from local compose for browser sessions. Docker-backed workers carry a session id into their Chromium profile path so stopped sessions can restart against the same persisted browser profile, and Docker runtime assignments are persisted/reconciled through Postgres on gateway restart.
  - `api.rs`: legacy compatibility endpoints plus the frozen owner-scoped `/api/v1/sessions` surface and session-scoped `access-tokens`, `automation-owner`, `status`, and `mcp-owner` routes.
- `code/shared/bpane-protocol`
  - Shared wire protocol, frame envelope, channel IDs, and message types.
- `code/web/bpane-client/js`
  - Real browser client implementation.
  - `bpane.ts`: public API and session orchestration.
  - `tile-compositor.ts` / `webgl-compositor.ts`: render path.
  - `audio-controller.ts`: desktop audio decode and microphone Opus encode.
  - `camera-controller.ts`: WebCodecs H.264 camera ingress.
  - `file-transfer.ts`, `input-controller.ts`, `session-stats.ts`: browser interaction and telemetry.
- `code/web/bpane-client`
  - TypeScript package. There is no meaningful Rust browser client crate in the current repo.
- `code/integrations/mcp-bridge`
  - SSE bridge to `@playwright/mcp`; owns session registration and MCP supervision behavior.
  - Can resolve an explicit control-plane session via `/api/v1/sessions`, accepts delegated-session assignment through its local `/control-session` API, resolves the managed session's runtime CDP endpoint from the session resource, and uses session-scoped `status` / `mcp-owner` APIs when a managed session is configured, including in `docker_pool` mode.
- `code/integrations/recording-worker`
  - Playwright-driven recorder worker that attaches as a `recorder` browser client through the control plane.
  - Creates or adopts session recording resources via `/api/v1/sessions/{id}/recordings`, waits for stop/finalize signals, then hands a temporary local file path back to the gateway for artifact-store finalization.
- `code/integrations/workflow-worker`
  - One-off workflow executor worker for owner-scoped workflow runs with git-backed source snapshots.
  - Loads the workflow run through the gateway using an owner bearer token, mints session automation access, downloads the run source snapshot and workspace inputs, materializes them locally, uploads produced files back through run-scoped artifact APIs, and executes the pinned Playwright entrypoint against the bound BrowserPane session.
- `deploy/compose.yml`
  - Source of truth for local dev runtime defaults.
  - Local auth in compose is OIDC via Keycloak on `:8091`.
  - Local session-control persistence in compose is Postgres on `:5433`.
  - Local workflow credential binding dev/testing uses HashiCorp Vault dev mode on `:8200`.
  - Local compose can now be switched to `docker_pool` for browser-session workers via gateway env overrides; `mcp-bridge` resolves the delegated session's runtime endpoint dynamically in that mode.
  - The gateway is configured to auto-launch workflow workers against the `deploy-workflow-worker` image on the compose network. Build that image before workflow-run smoke tests or local workflow execution.
  - The gateway mounts the repo at `/workspace:ro` so local git-backed workflow sources can be resolved and materialized during development smokes.

## Protocol and media facts

- `CH_VIDEO` is server-to-client datagram H.264 ROI video.
- `CH_TILES` is reliable tile rendering and is the primary visual path for UI/text.
- Desktop audio out uses codec-tagged frames; the compose stack currently defaults to Opus.
- Microphone ingress is Opus, not raw PCM.
- Camera ingress is H.264 via WebCodecs only. There is no MJPEG fallback.
- Tiles are QOI or zstd depending on emitter settings and heuristics.
- Viewers receive a filtered capability set and are enforced as read-only in both gateway and client.

## Shared-session behavior

- Browser sessions are collaborative by default.
- If `--exclusive-browser-owner` is enabled, one owner drives the session and additional browser clients join as viewers.
- MCP ownership still locks browser clients into viewer behavior.
- Late joiners are bootstrapped from cached session state and late-join refreshes are tracked in gateway telemetry.
- If a worker is still alive, reconnect returns to the exact live runtime. After idle-stop, reconnect restarts from the persisted Chromium profile instead of a true suspended process image.
- Gateway session status reports:
  - browser and viewer counts
  - `max_viewers` and remaining slots
  - session-scoped recording playback/export summary derived from retained segments
  - join latency telemetry
  - full-refresh burst telemetry

## Commands that matter

- Full Rust test suite: `cargo test --workspace`
- Gateway tests: `cargo test -p bpane-gateway`
- Host tests: `cargo test -p bpane-host`
- Protocol tests: `cargo test -p bpane-protocol`

Run these in `code/web/bpane-client`:
- `npx tsc --noEmit`
- `npm run smoke:automation-tasks -- --headless`
- `npm run smoke:file-workspaces -- --headless`
- `npm test`
- `npm run build`
- `npm run smoke:recording -- --headless`
- `npm run smoke:workflow-embed -- --headless`
- `npm run smoke:workflow-credentials -- --headless`
- `npm run smoke:workflow-workspace -- --headless`
- `npm run smoke:workflows -- --headless`
- `npm run smoke:workflow-extension -- --headless`
- `npm run smoke:multisession -- --headless`
- `npm run test:coverage`

Run these where applicable:
- `cd code/integrations/mcp-bridge && npm run build`
- `cd code/integrations/recording-worker && npm run build`
- `cd code/integrations/workflow-worker && npm run build`

## Local development flow

1. `./deploy/gen-dev-cert.sh dev/certs`
2. For the multi-session control-plane path, prefer:
   `BPANE_GATEWAY_RUNTIME_BACKEND=docker_pool BPANE_GATEWAY_MAX_ACTIVE_RUNTIMES=2 docker compose -f deploy/compose.yml up --build`
3. Open `http://localhost:8080` in Chromium.
4. Log in through the local Keycloak realm with `demo / demo-demo`.
5. The test page will resolve or create an owner-scoped `/api/v1/sessions` resource before transport connect.
6. The test page will mint a short-lived session-scoped connect ticket before WebTransport connect.
7. Use `Delegate MCP` if you want the local `mcp-bridge` to adopt that same session.
8. If needed, use the SPKI fingerprint from `http://localhost:8080/cert-fingerprint` so Chromium trusts the local gateway cert. `./deploy/gen-dev-cert.sh dev/certs` also refreshes `dev/certs/cert-fingerprint.txt` from the same `cert.pem`.
9. `vault` listens on `:8200`, `keycloak` on `:8091`, `postgres` on `:5433`, `mcp-bridge` on `:8931`, and the gateway HTTP API on `:8932`.

## Guardrails for contributors and agents

- Trust code and runtime manifests over stale prose. `README.md` may lag behind implementation.
- For Rust work, follow `RUST_STANDARDS.md` in addition to this file.
- Do not edit generated or vendored output:
  - `code/web/bpane-client/dist/`
  - `node_modules/`
  - `test-results/`
- Keep this file aligned with the live code when browser support, session-sharing behavior, media codecs, or runtime topology changes.
- Prefer narrow, subsystem-specific validation plus any impacted cross-cutting checks.

## When adding or changing features

- Update the support matrix if the change affects:
  - browser support
  - host platform support
  - session-sharing limits
  - default media behavior
- Update the architecture map if subsystem ownership moves.
- Only document commands that are actually runnable in this repo.
