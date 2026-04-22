# BrowserPane

BrowserPane is a browser-native remote browser/desktop stack for a Linux host container.

It runs a real Chromium session inside a Linux environment, captures that surface on the host, transports it over WebTransport, and renders it in a browser client with a tile-first pipeline plus optional ROI H.264 video for media-heavy regions.

![BrowserPane example](example.png)

## Current Status

BrowserPane is still experimental.

Current support and scope:

- Host runtime: Linux only. Ubuntu 24.04 container is the primary target.
- Browser runtime: Chromium desktop only. Firefox and Safari are not production targets.
- Shared sessions: collaborative by default, intended for small curated groups rather than broadcast-scale delivery.
- Owner/viewer mode: optional exclusive-owner mode is supported in the gateway; restricted viewers are read-only.
- Camera: disabled by default in the compose stack and requires browser H.264 encode support plus a mapped `v4l2loopback` device.

## How The System Is Shaped

At a high level, BrowserPane has five responsibilities:

1. Run a real browser session in a Linux host environment.
2. Capture and classify that surface efficiently.
3. Transport state, input, and media between host and browser.
4. Render the remote session in a regular web page.
5. Coordinate shared-session policy and automation ownership.

The default local runtime looks like this:

```text
browser client <-> bpane-gateway <-> bpane-host <-> Chromium inside Linux container
                       |
                       +-> postgres
                       |
                       +-> mcp-bridge
```

## Projects And Responsibilities

| Project | Responsibility |
| --- | --- |
| `code/apps/bpane-host` | Linux host agent. Captures the desktop surface, classifies tiles, drives ROI H.264 video, emits audio, injects input, and handles clipboard, file transfer, resize, and camera ingress plumbing. |
| `code/apps/bpane-gateway` | WebTransport entry point and shared-session coordinator. Relays frames between browser clients and the host, applies owner/viewer policy, and exposes the HTTP session/ownership API. |
| `code/shared/bpane-protocol` | Shared binary wire contract. Defines channels, frame envelopes, typed protocol messages, and incremental frame decoding used by the Rust services and validated against the browser client. |
| `code/web/bpane-client` | Real browser client. Renders tiles/video, decodes media, captures keyboard/mouse/clipboard input, and manages browser-side audio, camera, and file-transfer flows. |
| `code/integrations/mcp-bridge` | Automation bridge for MCP/Playwright-style control flows. Integrates with gateway ownership APIs so automation can drive a session while humans observe. |
| `deploy/` | Local runtime manifests and container images. This is the practical source of truth for how the dev stack is assembled and started. |

## Rendering Model

BrowserPane is not a simple full-frame video streamer.

- UI and text travel primarily over the reliable tile path.
- Media-heavy regions can move to ROI H.264 on the video path.
- Desktop audio travels separately from visual updates.
- Input, clipboard, file transfer, microphone, and camera each have dedicated protocol flows.

That split is what lets the system keep static UI sharp while still handling moving video efficiently.

## Protocol Model

The shared protocol is a compact binary protocol implemented in `bpane-protocol`.

- Reliable typed channels are used for control, input, cursor, clipboard, file transfer, and tiles.
- Raw media channels are used for video, desktop audio, microphone, and camera payloads.
- The protocol crate is the source of truth for frame/message definitions; the README stays intentionally high-level.

## Local Development

### Recommended: Docker Compose

Generate a dev certificate once:

```bash
./deploy/gen-dev-cert.sh dev/certs
```

Start the stack:

```bash
docker compose -f deploy/compose.yml up --build
```

Then open `http://localhost:8080` in Chromium.

The compose stack starts:

- `host`: Linux host runtime with Xorg dummy, Openbox, Chromium, and `bpane-host`
- `gateway`: WebTransport relay on `:4433` and HTTP APIs on `:8932`
- `postgres`: session-control database on `:5433`
- `keycloak`: local OIDC provider on `:8091`
- `web`: local frontend on `:8080`
- `mcp-bridge`: MCP bridge on `:8931`

The default local runtime backend is still a single shared host worker. The gateway now also has opt-in Docker-backed runtime backends:

- `docker_single`: one start-on-demand runtime container with idle shutdown
- `docker_pool`: multiple start-on-demand runtime containers with explicit `max_active_runtimes` and `max_starting_runtimes`

Neither Docker-backed mode is enabled by default in `deploy/compose.yml`, but the local stack is now wired so you can switch to the pool backend with:

```bash
BPANE_GATEWAY_RUNTIME_BACKEND=docker_pool \
BPANE_GATEWAY_MAX_ACTIVE_RUNTIMES=2 \
docker compose -f deploy/compose.yml up --build
```

`deploy/compose.yml` now mounts Docker access into the gateway and forwards a shared host-worker env profile automatically. If your compose project name is not the default `deploy`, override these defaults too:

- `BPANE_GATEWAY_DOCKER_RUNTIME_IMAGE`
- `BPANE_GATEWAY_DOCKER_RUNTIME_NETWORK`
- `BPANE_GATEWAY_DOCKER_RUNTIME_VOLUME`

The default local auth flow is now OIDC-based:

- open `http://localhost:8080`
- click `Login`
- authenticate against the local Keycloak realm
- use the demo account `demo / demo-demo`
- return to the page and either select an existing session or click `Start New Session`
- the page joins the selected owner-scoped `/api/v1/sessions` resource, or creates a new one before opening WebTransport
- sessions created from the test page use a 5 minute idle timeout and are stopped automatically if they remain unused or become idle without any browser viewers or MCP owner
- reconnecting a stopped session now restarts the same session resource instead of creating a new one
- in Docker-backed runtime modes, BrowserPane reuses a session-specific Chromium profile so cookies, cache, downloads, and Chromium session-restore state survive worker restarts
- Docker-backed runtime assignments are now persisted in Postgres and recovered on gateway restart, so an existing pool-mode worker can be rebound without launching a duplicate container
- exact in-memory browser process state is only preserved while the worker is still alive; once idle-stop shuts a worker down, reconnect restores the browser from its persisted profile rather than from a true container checkpoint
- if you want the local `mcp-bridge` to follow that same session, click `Delegate MCP`

`test-embed.html` fetches `/auth-config.json` and performs an Authorization Code + PKCE login. The browser client then connects to the gateway with an OIDC access token.
Before WebTransport connect, the page now mints a short-lived session-scoped connect ticket from the session API and uses that ticket on the transport URL instead of the long-lived bearer token.

For Chromium, WebTransport still needs trusted TLS on localhost. The current runtime SPKI fingerprint is served at:

```text
http://localhost:8080/cert-fingerprint
```

`./deploy/gen-dev-cert.sh dev/certs` also refreshes `dev/certs/cert-fingerprint.txt` from the same `cert.pem` for CLI use.

### Session Control Plane

The local stack now includes a Phase 0 session control plane in `bpane-gateway`.

- `POST /api/v1/sessions`
- `GET /api/v1/sessions`
- `GET /api/v1/sessions/{id}`
- `DELETE /api/v1/sessions/{id}`

These endpoints are bearer-protected, owner-scoped, and stored in Postgres.

The same API surface now also includes session-scoped runtime compatibility routes:

- `POST /api/v1/sessions/{id}/access-tokens`
- `GET /api/v1/sessions/{id}/status`
- `POST /api/v1/sessions/{id}/mcp-owner`
- `DELETE /api/v1/sessions/{id}/mcp-owner`
- `POST /api/v1/sessions/{id}/automation-owner`
- `DELETE /api/v1/sessions/{id}/automation-owner`

The local dev flow now uses those routes to bridge browser-owned and automation-owned control:

- `test-embed.html` resolves or creates an owner-scoped session before connect
- it then mints a short-lived `session_connect_ticket` from `POST /api/v1/sessions/{id}/access-tokens`
- the gateway routes the WebTransport connect through that explicit session id instead of one global token path
- `Delegate MCP` assigns that session to the local `bpane-mcp-bridge` service principal
- the page then calls `mcp-bridge` on `:8931/control-session` so the bridge adopts that same session for later ownership/status calls
- the local `mcp-bridge` now resolves the managed session's runtime CDP endpoint from the session resource, so delegated control also works in `docker_pool` mode

Current limitation:

- the public session resource model is now versioned and persistent
- gateway transport and runtime compatibility APIs are now session-scoped
- the default runtime backend is still `legacy_single_runtime` compatibility mode
- the optional `docker_single` backend can now start and stop one runtime container for the active session
- the optional `docker_pool` backend can start multiple runtime containers in parallel, but only up to its configured runtime caps
- Docker-backed runtime assignment metadata is now persisted and reconciled on gateway startup so pool-mode workers can survive a gateway restart cleanly
- `mcp-bridge` now follows the selected delegated session's runtime endpoint, but each bridge instance still manages only one control session at a time
- the default compose stack still only runs one active BrowserPane session at a time because it uses the single-runtime backend
- global compatibility routes like `/api/session/status` and `/api/session/mcp-owner` are only valid in legacy single-runtime mode; multi-runtime backends should use session-scoped `/api/v1/sessions/{id}/...` routes

### Build And Test Without Running The Full Stack

Rust:

```bash
cargo build --workspace
cargo test --workspace
```

Browser client:

```bash
cd code/web/bpane-client
npm ci
npx tsc --noEmit
npm test
npm run build
```

Other useful checks:

```bash
cargo test -p bpane-protocol
cargo test -p bpane-host
cargo test -p bpane-gateway
cd code/integrations/mcp-bridge && npm run build
cd code/web/bpane-client && npm run smoke:multisession -- --headless
```

## Shared Session Behavior

- Sessions are collaborative by default.
- If the gateway runs with exclusive browser ownership, one browser client is interactive and later clients become viewers.
- MCP ownership also forces browser clients into viewer behavior.
- Viewers are read-only and do not get interactive capabilities like input, clipboard, upload, download, microphone, camera, or resize.

## Authentication Model

- Browser clients authenticate to `bpane-gateway` with bearer access tokens.
- In the local compose stack, those tokens come from the Keycloak realm on `:8091`.
- The gateway supports OIDC/JWT validation with issuer, audience, and JWKS configuration.
- `mcp-bridge` uses OIDC client-credentials to call the gateway HTTP API.
- The versioned session API is owner-scoped off those bearer-token identities.
- Session-scoped browser transport now uses short-lived signed connect tickets minted from the session API.
- The old shared dev-token file flow is no longer the default local path.

## Documentation Policy

This README is intentionally responsibility-oriented and high level.

It should explain:

- what BrowserPane is
- what each project is responsible for
- what is currently supported
- how to run and validate the system

It should not try to mirror the exact file layout or every implementation detail. Those move too quickly and become stale.

When documentation disagrees with reality, prefer:

1. the code
2. runtime manifests and package scripts
3. `AGENTS.md`
4. this `README.md`
