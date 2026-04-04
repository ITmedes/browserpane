# BrowserPane — Web Remote Desktop Protocol

A browser-native remote desktop system that renders a Linux desktop session inside a web container using WebTransport, WebCodecs, and WebGL 2.

## Project Status

> **Experimental, fully vibe-coded, and not manually reviewed yet**
>
> BrowserPane has been built through fast AI-assisted iteration and has not yet gone through a systematic manual code review, security audit, or production hardening pass. The project is promising, but it is still early.

What BrowserPane already delivers:
- A real remote browser/desktop surface running inside managed Linux containers
- An embeddable browser pane for web products and internal tools
- A tile-first render path that keeps UI and text crisp and bandwidth-efficient
- Selective H.264 video for media-heavy regions instead of turning the whole session into full-frame video
- Shared sessions for small curated groups, plus audio, microphone, camera, clipboard, and file transfer primitives

What BrowserPane does not claim yet:
- Production readiness
- Manual review or security hardening
- Cross-browser production support beyond Chromium desktop
- Broadcast-scale delivery

This repository is an open invitation to help turn BrowserPane from a strong vibe-coded prototype into a reviewed, hardened, production-grade system. If the direction is interesting to you, contributions, audits, testing, and architecture feedback are all welcome.

## Example

![BrowserPane example](example.png)

## Architecture

For a deeper technical walkthrough, see [ARCH.md](ARCH.md).

```
Browser (Chrome)          Gateway (Rust)         Host Agent (Rust)
+--------------+       +--------------+       +------------------+
| bpane-client |<----->| bpane-gateway |<----->|    bpane-host     |
|  (TypeScript) | QUIC |  WebTransport|  Unix |  X11 capture     |
|  WebGL 2     |  TLS  |  relay       | socket|  H.264 encode    |
|  WebCodecs   |       |  HTTP API    |       |  XTest inject    |
+--------------+       +--------------+       +------------------+
                              |                       |
                              | HTTP API (:8932)      | CDP (ws://9222)
                              v                       v
                       +--------------+       +--------------+
                       |  mcp-bridge  |       |  Chromium    |
                       |  (Node.js)   |       |  (headless)  |
                       +--------------+       +--------------+
```

**Core code modules:**

| Crate | Purpose |
|-------|---------|
| `bpane-protocol` | Shared wire protocol types, binary framing (`no_std` compatible) |
| `bpane-host` | Linux host agent — screen capture, tile encoding, video encode, audio, input injection |
| `bpane-gateway` | WebTransport server — relays frames between browsers and host, session management, MCP ownership API |
| `bpane-client` | TypeScript browser client — WebGL 2 rendering, WebCodecs decode, input capture, audio/camera/file transfer |

## Prerequisites

- **Rust** (stable, 1.78+)
- **Docker** (for the full local dev stack)
- **Node.js** (22+, for building the TypeScript client, MCP bridge, and running tests)

## Quick Start — Run Locally

Local runtime is supported in two ways:
- Docker Compose on macOS or Linux
- Native Linux only

macOS is supported for building and running tests, but not for running the host stack natively.

### Option 1: Docker Compose (recommended)

This starts four containers on a bridge network:

| Container | What it does |
|-----------|--------------|
| **host** | Ubuntu 24.04 with Xorg dummy display + OpenBox + Chromium + PipeWire + bpane-host |
| **gateway** | WebTransport relay server on port 4433, HTTP API on 8932 |
| **web** | nginx serving the test UI on port 8080 |
| **mcp-bridge** | SSE bridge to @playwright/mcp on port 8931 |

```bash
# Generate a self-signed dev certificate
./deploy/gen-dev-cert.sh dev/certs

# Build and start the stack
docker compose -f deploy/compose.yml up --build
```

Then open **http://localhost:8080** in Chrome and click **Connect**.

The gateway auto-generates an auth token and shares it with the web frontend
via a shared volume, so the page picks it up automatically. The cert hash is
also served by nginx for WebTransport certificate verification.

> **Chrome TLS requirement:** WebTransport requires trusted TLS even on localhost.
> Launch Chrome with the SPKI fingerprint from the generated cert:
>
> ```bash
> # The fingerprint is written by gen-dev-cert.sh
> cat dev/certs/cert-fingerprint.txt
>
> # Launch Chrome with that fingerprint
> chrome --ignore-certificate-errors-spki-list=<fingerprint>
> ```

### Option 2: Build + Test From Source (macOS / Linux)

```bash
# Build all Rust crates
cargo build --workspace

# Run Rust tests (409 tests across all crates)
cargo test --workspace

# Build and test the TypeScript client (261 tests)
cd code/web/bpane-client
npm ci
npm run build
npm test
```

This option is for building and validating the codebase. It does not start a supported local BrowserPane runtime on macOS.

If you want to actually run BrowserPane locally:
- use Docker Compose on macOS or Linux
- use the native manual stack below on Linux only

### Option 3: Full Manual Local Stack (Linux only)

**Terminal 1 — Virtual display + window manager + app:**

```bash
Xvfb :99 -screen 0 1280x720x24 &
DISPLAY=:99 openbox &
DISPLAY=:99 chromium --no-first-run http://example.com &
```

**Terminal 2 — Host agent:**

```bash
DISPLAY=:99 cargo run -p bpane-host -- --socket /tmp/bpane.sock --fps 60
```

**Terminal 3 — Gateway server:**

```bash
# Generate a cert first
./deploy/gen-dev-cert.sh dev/certs

cargo run -p bpane-gateway -- \
  --agent-socket /tmp/bpane.sock \
  --port 4433 \
  --cert dev/certs/cert.pem \
  --key dev/certs/cert.key
```

The gateway prints a dev token on startup. Open `dev/test-embed.html` in Chrome
(launched with the SPKI fingerprint flag) and paste the token to connect.

## Project Layout

```
pane/
+-- Cargo.toml                    # Workspace root
+-- code/
|   +-- apps/
|   |   +-- bpane-host/           # Host agent daemon
|   |   |   +-- src/
|   |   |       +-- main.rs       # Session orchestration, frame bridge
|   |   |       +-- ipc.rs        # Unix socket IPC framing
|   |   |       +-- capture/      # Screen capture (FFmpeg x11grab, X11 SHM)
|   |   |       +-- encode/       # Video encode (libx264, JPEG, test)
|   |   |       +-- tiles/        # Tile grid, emitter, scroll detection
|   |   |       +-- input/        # Input injection (XTest mouse + keyboard)
|   |   |       +-- audio/        # PipeWire/PulseAudio capture + mic input
|   |   |       +-- cdp_video.rs  # CDP video region detection
|   |   |       +-- camera.rs     # H.264 browser camera -> v4l2loopback
|   |   |       +-- clipboard.rs  # X11 clipboard sync
|   |   |       +-- filetransfer.rs # File upload/download
|   |   |       +-- cursor.rs     # XFixes cursor tracking
|   |   |       +-- resize.rs     # Resolution change handler
|   |   |       +-- display.rs    # Display mode detection
|   |   |
|   |   +-- bpane-gateway/        # WebTransport gateway
|   |       +-- src/
|   |           +-- main.rs       # CLI args, startup
|   |           +-- transport.rs  # WebTransport accept + per-client handler
|   |           +-- session_hub.rs # Broadcast fan-out, late-join bootstrap
|   |           +-- session_registry.rs # Hub lifecycle, concurrent join
|   |           +-- relay.rs      # Unix socket <-> async bridge
|   |           +-- session.rs    # Session heartbeat
|   |           +-- api.rs        # HTTP API (session status, MCP ownership)
|   |           +-- auth.rs       # HMAC-SHA256 token auth
|   |           +-- config.rs     # CLI config
|   |
|   +-- shared/
|   |   +-- bpane-protocol/       # Shared types, binary framing (no_std)
|   |       +-- src/
|   |           +-- channel.rs    # 11 channel IDs
|   |           +-- frame.rs      # Binary encode/decode, message dispatch
|   |           +-- types.rs      # All message structs and enums
|   |           +-- lib.rs
|   |
|   +-- web/
|   |   +-- bpane-client/         # TypeScript browser client
|   |       +-- js/
|   |       |   +-- bpane.ts      # Public API, session lifecycle, video decode
|   |       |   +-- protocol.ts   # Wire protocol constants, frame encode/decode
|   |       |   +-- webgl-compositor.ts # WebGL 2 GPU-accelerated tile renderer
|   |       |   +-- tile-compositor.ts  # Tile batch processing, scroll, cache dispatch
|   |       |   +-- tile-cache.ts # LRU tile cache (8192 entries, 50 MB cap)
|   |       |   +-- input-controller.ts # Keyboard, mouse, scroll, clipboard, dead keys
|   |       |   +-- input-map.ts  # KeyboardEvent.code -> evdev mapping
|   |       |   +-- audio-controller.ts # Opus/ADPCM/PCM decode, mic capture
|   |       |   +-- audio-worklet.ts # AudioWorklet ring buffer playback
|   |       |   +-- camera-controller.ts # WebCodecs H.264 encode, adaptive bitrate
|   |       |   +-- file-transfer.ts # Upload/download, drag-drop
|   |       |   +-- session-stats.ts # Telemetry counters
|   |       |   +-- nal.ts        # H.264 NAL reassembly from datagrams
|   |       |   +-- qoi.ts        # QOI image decoder
|   |       |   +-- hash.ts       # FNV-1a (clipboard echo prevention)
|   |       |   +-- __tests__/    # 261 Vitest tests (16 test files)
|   |       +-- dist/             # Vite build output (ES module)
|   |       +-- package.json
|   |       +-- tsconfig.json
|   |
|   +-- integrations/
|   |   +-- mcp-bridge/           # MCP automation bridge
|   |       +-- src/
|   |       |   +-- index.ts      # SSE server, Playwright MCP proxy
|   |       |   +-- supervisor-monitor.ts # Polls gateway for viewer count
|   |       +-- package.json
|   |
|   +-- tests/
|       +-- e2e/                  # End-to-end Playwright tests
|           +-- bpane.spec.ts     # Connection, rendering, reconnect
|           +-- playwright.config.ts
|
+-- deploy/
|   +-- compose.yml               # Local dev stack (4 containers)
|   +-- Dockerfile.host           # Ubuntu 24.04 host agent image
|   +-- Dockerfile.gateway        # Gateway server image
|   +-- Dockerfile.web            # nginx frontend image
|   +-- Dockerfile.mcp-bridge     # MCP bridge image
|   +-- start-host.sh             # Host container orchestration
|   +-- gen-dev-cert.sh           # Self-signed TLS cert generator
|   +-- xorg-dummy.conf           # Xorg headless display config
|   +-- nginx.conf                # nginx reverse proxy config
|   +-- bpane-ext/                # Chromium extension (scroll override)
|   +-- chromium-policies/        # Chromium managed policies (AdBlock)
|   +-- ansible/                  # Production deployment playbook
+-- dev/
    +-- test-embed.html           # Browser test harness UI
    +-- certs/                    # Generated dev certificates
```

## Wire Protocol

All messages use a 5-byte envelope: `channel(u8) + length(u32 LE) + payload`. No JSON, no protobuf — manual binary serialization for minimal overhead and zero allocation on the hot path.

| Channel | ID | Transport | Direction | Purpose |
|---------|----|-----------|-----------|---------|
| VIDEO | 0x01 | Datagrams | S->C | H.264 NAL units (fragmented for QUIC MTU) |
| AUDIO_OUT | 0x02 | Stream | S->C | Desktop audio (Opus, IMA-ADPCM, or PCM) |
| AUDIO_IN | 0x03 | Stream | C->S | Microphone (Opus) |
| VIDEO_IN | 0x04 | Stream | C->S | Webcam (H.264) |
| INPUT | 0x05 | Stream | C->S | Mouse, keyboard, scroll |
| CURSOR | 0x06 | Stream | S->C | Cursor shape + position |
| CLIPBOARD | 0x07 | Stream | Bidir | Clipboard text sync |
| FILE_UP | 0x08 | Stream | C->S | File upload chunks |
| FILE_DOWN | 0x09 | Stream | S->C | File download chunks |
| CONTROL | 0x0A | Stream | Bidir | Resize, ping/pong, session, bitrate hints |
| TILES | 0x0B | Stream | S->C | Tile rendering commands (12 message types) |

## Testing

```bash
# Rust tests (409 total: protocol + host + gateway)
cargo test --workspace

# Per-crate Rust tests
cargo test -p bpane-protocol
cargo test -p bpane-host
cargo test -p bpane-gateway

# TypeScript client tests (261 total, Vitest)
cd code/web/bpane-client
npx tsc --noEmit      # type check
npm test              # unit tests
npm run test:coverage # with coverage

# MCP bridge build
cd code/integrations/mcp-bridge && npm run build

# E2E tests (requires running dev stack + local certs)
cd code/tests/e2e && npm test
```

## Embedding

```typescript
import { BpaneSession } from 'bpane-client';

const session = await BpaneSession.connect({
  container: document.getElementById('desktop'),
  gatewayUrl: 'https://your-gateway:4433',
  token: 'hmac-auth-token',
  certHashUrl: '/cert-hash',   // for self-signed certs
  hiDpi: false,                // scale for high-DPI displays
  audio: true,
  clipboard: true,
  fileTransfer: true,
  onConnect: () => console.log('connected'),
  onDisconnect: (reason) => console.log('disconnected:', reason),
  onError: (err) => console.error(err),
  onResolutionChange: (w, h) => console.log(`${w}x${h}`),
  onCapabilitiesChange: (caps) => console.log(caps),
});

// The container drives the remote resolution.
// No zoom, no scroll, no letterbox.

// Render diagnostics (WebGL 2 or Canvas 2D fallback)
const render = session.getRenderDiagnostics();
console.log(render.backend, render.reason, render.renderer);

// Telemetry
console.log(session.getSessionStats());
console.log(session.getTileCacheStats());

// Bidirectional media
await session.startMicrophone();
await session.startCamera();

// File transfer
session.promptFileUpload();

session.disconnect();
```

## Key Design Decisions

- **Container-driven sizing** — The `<div>` size _is_ the remote resolution. A 500x500 container means the remote runs at 500x500. Optional `hiDpi` mode applies `devicePixelRatio` scaling.
- **WebGL 2 rendering** — GPU-accelerated tile compositing with persistent video textures and framebuffer scroll blits. Automatic Canvas 2D fallback if WebGL 2 is unavailable or only software-rendered.
- **Per-tile codec selection** — The screen is a 64x64 tile grid. Each tile is independently encoded as Fill (solid color, 9 bytes), CacheHit (hash match, 13 bytes), QOI/Zstd (lossless, 1-10 KB), or deferred to H.264 for video regions.
- **Scroll optimization** — Hash-column comparison detects scroll displacement. Existing pixels are shifted client-side via `ScrollCopy`; only newly exposed edge tiles are encoded.
- **`no_std` protocol crate** — `bpane-protocol` compiles to both native and WASM with no runtime allocation on the hot path.
- **Manual binary serialization** — No serde, no protobuf. All integers little-endian.
- **NAL fragmentation** — Video datagrams exceeding the QUIC MTU (~1200 bytes) are fragmented with sequence numbers and reassembled on the client.
- **Shared sessions** — Browser sessions are collaborative by default. Optional exclusive-owner mode enables one interactive browser client plus up to 10 read-only viewers, and MCP agents can claim ownership and lock resolution for automation while humans observe.
- **Graceful degradation** — Audio, microphone, camera, and file transfer degrade cleanly if the host lacks PipeWire, v4l2loopback, or other optional dependencies.

## License

MIT
