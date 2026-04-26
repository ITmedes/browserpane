# BrowserPane

BrowserPane is planned to be a self-hostable remote browser and workflow execution platform for humans and agents.

Most browser automation products stop at managed browsers, CDP endpoints, or live debug links. BrowserPane treats the live browser session itself as the product surface: a real Chromium session that browser users, supervisors, and automation can all attach to with shared-session policy, owner/viewer controls, and persistent session resources.

BrowserPane is intended to be integrated into larger automation and workflow systems. Its workflow layer is primarily about browser-run execution, supervision, artifacts, and human intervention around a live browser session, not about replacing a general scheduler or DAG orchestrator.

It runs a real Chromium session inside a Linux environment, captures that surface on the host, transports it over WebTransport, and renders it in a browser client with a tile-first pipeline plus optional ROI H.264 video for media-heavy regions.

![BrowserPane example](example.png)

The frozen v1 session-control contract now lives in [openapi/bpane-control-v1.yaml](openapi/bpane-control-v1.yaml).

## Why BrowserPane

BrowserPane will be worth considering if you need more than "a browser for an agent."

- Shared sessions are a first-class feature, not an afterthought. Multiple browser clients can join the same session with collaborative or restricted viewer behavior.
- Automation attaches to governed sessions instead of bypassing session policy. MCP and other automation flows operate through explicit ownership and session-control APIs.
- The remoting stack is browser-native. BrowserPane uses WebTransport plus a tile-first render path with optional ROI H.264 instead of relying only on full-frame streaming or vendor-hosted live debug UIs.
- The session behaves like a real remote workspace. Clipboard, file transfer, audio out, microphone in, camera ingress, resize, and input policy are part of the system design.
- The platform is self-hostable. Teams can run BrowserPane in their own environment instead of treating browser control as a SaaS-only dependency.

## Where It Fits

BrowserPane is a strong fit for:

- human-in-the-loop browser automation
- collaborative investigation, support, or review sessions
- regulated or private deployments that need self-hosted browser access
- workflow systems that need durable session identity, artifacts, logs, and audit history
- platforms that need a governed browser execution target inside a larger orchestration stack

## Current Status

BrowserPane is still experimental.

Current support and scope:

- Host runtime: Linux only. Ubuntu 24.04 container is the primary target.
- Browser runtime: Chromium desktop only. Firefox and Safari are not production targets.
- Shared sessions: collaborative by default, intended for small curated groups rather than broadcast-scale delivery.
- Owner/viewer mode: optional exclusive-owner mode is supported in the gateway; restricted viewers are read-only.
- Camera: disabled by default in the compose stack and requires browser H.264 encode support plus a mapped `v4l2loopback` device.
- Control plane: owner-scoped v1 APIs now cover sessions, session recordings, workflow definitions/runs, file workspaces, credential bindings, and approved extensions.
- Workflow execution: Git-backed workflow versions run through a gateway-managed `workflow-worker`; the current executor model is Playwright.
- Workflow boundary: BrowserPane currently focuses on executing and supervising browser workflows. Broader scheduling, DAG orchestration, and cross-system coordination are expected to sit above BrowserPane rather than inside it.

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
| `code/integrations/workflow-worker` | On-demand workflow executor. Downloads pinned workflow source snapshots, attaches with session automation access, runs Playwright workflow entrypoints, resolves credential/workspace inputs, and writes logs, outputs, and produced files back to the gateway. |
| `code/integrations/recording-worker` | On-demand recording executor. Attaches as a passive recorder client, captures WebM output, and finalizes recording metadata into gateway-managed artifact storage. |
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

If you want the full multi-session flow from the local session console, start the stack in `docker_pool` mode:

Generate a dev certificate once:

```bash
./deploy/gen-dev-cert.sh dev/certs
```

Start the stack:

```bash
BPANE_GATEWAY_RUNTIME_BACKEND=docker_pool \
BPANE_GATEWAY_MAX_ACTIVE_RUNTIMES=2 \
docker compose -f deploy/compose.yml up --build
```

If you rebuild `gateway` later without those env overrides, Compose will fall back to the default `static_single` backend.

Then open `http://localhost:8080` in Chromium.

Use these local dev credentials on the login screen:

- username: `demo`
- password: `demo-demo`

Then:

1. Click `Login`
2. Click `Start New Session` to create a fresh browser, or select an older session and click `Join / Reconnect`
3. Open the same selected session in another signed-in browser window if you want to share it live with another user
4. Click `Delegate MCP` if you want the local `mcp-bridge` to drive that exact session

If you only want the older single-runtime dev stack, the default command still works:

```bash
docker compose -f deploy/compose.yml up --build
```

The compose stack starts:

- `host`: Linux host runtime with Xorg dummy, Openbox, Chromium, and `bpane-host`
- `gateway`: WebTransport relay on `:4433` and HTTP APIs on `:8932`
- `postgres`: session-control database on `:5433`
- `vault`: local HashiCorp Vault dev server on `:8200` for workflow credential bindings
- `keycloak`: local OIDC provider on `:8091`
- `web`: local frontend on `:8080`
- `mcp-bridge`: MCP bridge on `:8931`

The local compose file also defines a `workflow-worker` image profile. The gateway launches workflow-worker containers on demand; you normally do not start that container as a long-lived service yourself.

The gateway supports three runtime backends:

- `static_single`: one shared host worker
- `docker_single`: one start-on-demand runtime container with idle shutdown
- `docker_pool`: multiple start-on-demand runtime containers with explicit `max_active_runtimes` and `max_starting_runtimes`

`deploy/compose.yml` still defaults to `static_single`, but the local stack is wired so you can switch to the pool backend with:

```bash
BPANE_GATEWAY_RUNTIME_BACKEND=docker_pool \
BPANE_GATEWAY_MAX_ACTIVE_RUNTIMES=2 \
docker compose -f deploy/compose.yml up --build
```

`deploy/compose.yml` now mounts Docker access into the gateway and forwards a shared host-worker env profile automatically. If your compose project name is not the default `deploy`, override these defaults too:

- `BPANE_GATEWAY_DOCKER_RUNTIME_IMAGE`
- `BPANE_GATEWAY_DOCKER_RUNTIME_NETWORK`
- `BPANE_GATEWAY_DOCKER_RUNTIME_VOLUME`

The default local auth flow is OIDC-based:

- open `http://localhost:8080`
- click `Login`
- authenticate against the local Keycloak realm
- use the demo account `demo / demo-demo`
- return to the page and either select an existing session or click `Start New Session`
- the page joins the selected owner-scoped `/api/v1/sessions` resource, or creates a new one before opening WebTransport
- sessions created from the test page use a 5 minute idle timeout and are stopped automatically if they remain unused or become idle without any browser viewers or MCP owner
- reconnecting a stopped session now restarts the same session resource instead of creating a new one
- the console UI now shows whether the currently selected session is the exact session delegated to the local MCP bridge
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

The local stack now includes a frozen v1 session control plane in `bpane-gateway`.

Canonical contract:

- [openapi/bpane-control-v1.yaml](openapi/bpane-control-v1.yaml)

- `POST /api/v1/sessions`
- `GET /api/v1/sessions`
- `GET /api/v1/sessions/{id}`
- `DELETE /api/v1/sessions/{id}`

These endpoints are bearer-protected, owner-scoped, and stored in Postgres.

The same frozen API surface also includes session-scoped runtime routes:

- `POST /api/v1/sessions/{id}/access-tokens`
- `GET /api/v1/sessions/{id}/status`
- `POST /api/v1/sessions/{id}/mcp-owner`
- `DELETE /api/v1/sessions/{id}/mcp-owner`
- `POST /api/v1/sessions/{id}/automation-owner`
- `DELETE /api/v1/sessions/{id}/automation-owner`

The local dev flow uses those routes to bridge browser-owned and automation-owned control:

- `test-embed.html` resolves or creates an owner-scoped session before connect
- it then mints a short-lived `session_connect_ticket` from `POST /api/v1/sessions/{id}/access-tokens`
- the gateway routes the WebTransport connect through that explicit session id instead of one global token path
- `Delegate MCP` assigns that session to the local `bpane-mcp-bridge` service principal
- the page then calls `mcp-bridge` on `:8931/control-session` so the bridge adopts that same session for later ownership/status calls
- the local `mcp-bridge` now resolves the managed session's runtime CDP endpoint from the session resource, so delegated control also works in `docker_pool` mode

Current limitation:

- the public session resource model is now versioned and persistent
- gateway transport and runtime compatibility APIs are now session-scoped
- gateway runtime orchestration now goes through an internal `SessionManager` boundary; the current runtime backend implementation still lives in `runtime_manager.rs`
- the default runtime backend is still `legacy_single_runtime` compatibility mode
- the optional `docker_single` backend can now start and stop one runtime container for the active session
- the optional `docker_pool` backend can start multiple runtime containers in parallel, but only up to its configured runtime caps
- Docker-backed runtime assignment metadata is now persisted and reconciled on gateway startup so pool-mode workers can survive a gateway restart cleanly
- `mcp-bridge` now follows the selected delegated session's runtime endpoint, but each bridge instance still manages only one control session at a time
- the default compose stack still runs the single-runtime backend unless you opt into `docker_pool`
- global compatibility routes like `/api/session/status` and `/api/session/mcp-owner` are compatibility-only and are not part of the frozen v1 contract; multi-runtime backends should use session-scoped `/api/v1/sessions/{id}/...` routes

### Recordings

BrowserPane session recording is now a control-plane feature rather than only a browser-local blob download.

- Session recording policy supports `disabled`, `manual`, and `always`.
- Recording resources are session-scoped and persist segment metadata, runtime state, termination reason, and artifact linkage.
- Recordings can be downloaded from the dev page recording library or through the v1 API.
- Playback/export is modeled separately from raw recording segments, so multi-segment sessions stay explicit.

Primary routes:

- `POST /api/v1/sessions/{id}/recordings`
- `GET /api/v1/sessions/{id}/recordings`
- `GET /api/v1/sessions/{id}/recordings/{recording_id}`
- `POST /api/v1/sessions/{id}/recordings/{recording_id}/stop`
- `GET /api/v1/sessions/{id}/recordings/{recording_id}/content`

Local manual flow:

1. Open `http://localhost:8080`
2. Start or reconnect a session
3. Use the recording controls in `test-embed.html`
4. Download individual segments or the playback export bundle from the recording library

### Workflow Platform

BrowserPane now exposes a first-class workflow execution layer on top of session automation access.

Current workflow capabilities:

- owner-scoped workflow definitions and immutable versions
- workflow runs with logs, events, outputs, recordings, and produced files
- git-backed workflow sources pinned to resolved commits
- source snapshot materialization per run
- file workspaces for reusable inputs and durable outputs
- Vault-backed credential bindings
- approved extension references on workflow versions and sessions
- local workflow CLI for owner-token-driven testing and automation

Primary routes:

- `POST /api/v1/workflows`
- `GET /api/v1/workflows`
- `POST /api/v1/workflows/{id}/versions`
- `POST /api/v1/workflow-runs`
- `GET /api/v1/workflow-runs/{id}`
- `GET /api/v1/workflow-runs/{id}/logs`
- `GET /api/v1/workflow-runs/{id}/events`
- `GET /api/v1/workflow-runs/{id}/produced-files`

Reusable workflow inputs:

- `POST /api/v1/file-workspaces`
- `POST /api/v1/credential-bindings`
- `POST /api/v1/extensions`

Workflow boundary:

- BrowserPane owns browser-run execution, run state, recordings/artifacts, reusable runtime inputs, and human intervention around the run.
- External workflow systems should usually own schedules, DAGs, broad retry policy, and cross-system orchestration.

Local usage options:

- UI: use the workflow panel in `test-embed.html`
- CLI: use `code/web/bpane-client/scripts/workflow-cli.mjs`
- raw API: use the OpenAPI contract in `openapi/bpane-control-v1.yaml`

Typical local workflow path:

1. Start the local compose stack and log in at `http://localhost:8080`
2. Create or reconnect a browser session from `test-embed.html`
3. Create reusable inputs as needed:
   - file workspace for reusable input/output files
   - credential binding for Vault-backed secrets
   - approved extension if the workflow needs a Chromium extension
4. Create a workflow definition and a pinned version that points at a git-backed Playwright entrypoint
5. Start a workflow run from the workflow panel, the CLI, or the raw v1 API
6. Inspect logs, events, outputs, recordings, and produced files from the run resource

Minimal CLI flow with an owner bearer token:

```bash
cd code/web/bpane-client
export BPANE_API_URL=http://localhost:8932
export BPANE_ACCESS_TOKEN=<owner bearer token>
npm run workflow:cli -- workflow list
npm run workflow:cli -- workflow run get <run-id>
```

The CLI is intentionally thin. It wraps the existing owner-scoped v1 workflow routes rather than introducing a second control-plane contract.

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
npm run workflow:cli -- --help
npm run smoke:recording -- --headless
npm run smoke:workflow-cli -- --headless
npm run smoke:workflow-credential-injection -- --headless
```

Other useful checks:

```bash
cargo test -p bpane-protocol
cargo test -p bpane-host
cargo test -p bpane-gateway
cd code/integrations/mcp-bridge && npm run build
cd code/integrations/workflow-worker && npm run build
cd code/web/bpane-client && npm run smoke:recording -- --headless
cd code/web/bpane-client && npm run smoke:workflow-cli -- --headless
cd code/web/bpane-client && npm run smoke:workflow-credential-injection -- --headless
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
