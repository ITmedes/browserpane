# BrowserPane Agent Guide

This file is the shared project memory for BrowserPane. Keep it short, code-aligned, and current.

Project-wide Rust coding standards live in `RUST_STANDARDS.md`.
- Apply them to all Rust crates in this repo.
- Update that file instead of expanding this one with detailed Rust style rules.

Project-wide TypeScript and Node.js coding standards live in `NODEJS_STANDARDS.md`.
- Apply them to `code/web/bpane-client`, `code/integrations/mcp-bridge`, and future TS/Node packages.
- Update that file instead of expanding this one with detailed TS/Node style rules.

When docs disagree, prefer:
1. The code
2. Runtime manifests and package scripts
3. This file
4. `README.md`

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
  - `session_control.rs`: Phase 0 versioned session-resource store and Postgres integration.
  - `runtime_manager.rs`: session-id-to-runtime resolution seam; currently supports `static_single` and opt-in `docker_single` backends, but both still allow only one active runtime at a time.
  - `api.rs`: legacy compatibility endpoints plus `POST/GET/DELETE /api/v1/sessions` and session-scoped `access-tokens`, `automation-owner`, `status`, and `mcp-owner` routes.
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
  - Can resolve an explicit control-plane session via `/api/v1/sessions`, accepts delegated-session assignment through its local `/control-session` API, and uses session-scoped `status` / `mcp-owner` APIs when a managed session is configured.
- `deploy/compose.yml`
  - Source of truth for local dev runtime defaults.
  - Local auth in compose is OIDC via Keycloak on `:8091`.
  - Local session-control persistence in compose is Postgres on `:5433`.

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
- Gateway session status reports:
  - browser and viewer counts
  - `max_viewers` and remaining slots
  - join latency telemetry
  - full-refresh burst telemetry

## Commands that matter

- Full Rust test suite: `cargo test --workspace`
- Gateway tests: `cargo test -p bpane-gateway`
- Host tests: `cargo test -p bpane-host`
- Protocol tests: `cargo test -p bpane-protocol`

Run these in `code/web/bpane-client`:
- `npx tsc --noEmit`
- `npm test`
- `npm run build`
- `npm run test:coverage`

Run these where applicable:
- `cd code/integrations/mcp-bridge && npm run build`
- `cd code/tests/e2e && npm test`
  - Chromium only
  - expects the dev stack and local cert setup

## Local development flow

1. `./deploy/gen-dev-cert.sh dev/certs`
2. `docker compose -f deploy/compose.yml up --build`
3. Open `http://localhost:8080` in Chromium.
4. Log in through the local Keycloak realm if prompted.
5. The test page will resolve or create an owner-scoped `/api/v1/sessions` resource before transport connect.
6. The test page will mint a short-lived session-scoped connect ticket before WebTransport connect.
7. Use `Delegate MCP` if you want the local `mcp-bridge` to adopt that same session.
8. If needed, use the SPKI fingerprint from `http://localhost:8080/cert-fingerprint` so Chromium trusts the local gateway cert. `./deploy/gen-dev-cert.sh dev/certs` also refreshes `dev/certs/cert-fingerprint.txt` from the same `cert.pem`.
9. `keycloak` listens on `:8091`, `postgres` on `:5433`, `mcp-bridge` on `:8931`, and the gateway HTTP API on `:8932`.

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
