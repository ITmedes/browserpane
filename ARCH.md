# BrowserPane Architecture Deep Dive

## What It Is

BrowserPane is a browser-native remote desktop protocol. It renders a Linux desktop
inside a browser `<div>` using WebTransport, WebCodecs, and WebGL 2 — no
plugins, no Electron, no VNC viewer. The container size drives the remote
resolution pixel-for-pixel.

The system has five runtime components connected by two transport layers plus a persistent control-plane store:

```
┌─────────────┐   Unix Socket   ┌─────────────┐   WebTransport   ┌─────────────┐
│  bpane-host │ <─────────────> │bpane-gateway│ <──────────────> │   Browser    │
│  (Rust)     │   binary frames │  (Rust)     │   QUIC streams   │  (TS)       │
└─────────────┘                 └─────────────┘                   └─────────────┘
       │                              │
       │ CDP (ws://9222)              │ HTTP API (:8932)
       v                              v
┌─────────────┐                ┌─────────────┐
│  Chromium   │                │ mcp-bridge  │
│  (headless) │                │  (Node.js)  │
└─────────────┘                └─────────────┘
                                      │
                                      v
                               ┌─────────────┐
                               │  Postgres   │
                               │ control API │
                               └─────────────┘
```

---

## Tech Stack

| Layer | Technology | Why |
|-------|-----------|-----|
| Host agent | Rust + Tokio | Low-latency async I/O, zero-copy frame handling, direct X11 access |
| Screen capture | X11 SHM + XDamage + XComposite | Incremental damage, shared-memory pixel access, no full-frame polling |
| Video encode | FFmpeg x11grab -> libx264 | H.264 Baseline for WebCodecs compat; process isolation from host |
| Gateway | Rust + wtransport | QUIC/WebTransport server with reliable streams + datagrams |
| Browser client | TypeScript + fzstd | WebGL 2 compositing (Canvas 2D fallback), WebCodecs H.264 decode, WebTransport API |
| Wire protocol | Custom binary, no serde | Manual `[u8]` encode/decode for minimal overhead and zero alloc on hot path |
| Tile compression | QOI or Zstd (configurable) | QOI: fast decode, good for UI; Zstd: better ratio for complex content |
| Audio | PipeWire -> FFmpeg -> Opus or IMA-ADPCM | 48 kHz stereo, silence-gated; Opus default (64 kbps CBR), ADPCM fallback |
| MCP bridge | Node.js + @playwright/mcp | SSE proxy for browser automation with live supervision |
| Session store | PostgreSQL 16 | Durable owner-scoped `/api/v1/sessions` resources |
| Deployment | Docker Compose, 6 containers | Isolated services on a bridge network |

---

## Deployment Topology

Six containers on a Docker bridge network (`172.28.0.0/24`):

```
              Browser / E2E Test
                     │
                     v
       ┌──────────────────────────┐
       │  nginx (web, :8080)      │
       │  / -> test-embed.html    │
       │  /dist/ -> bpane-client   │
       │  /auth-config.json       │
       │  /cert-hash -> cert.pem  │
       │  /cert-fingerprint ->    │
       │     cert.pem             │
       └────────────┬─────────────┘
                    │
           Docker Bridge Network
                    │
   ┌──────────────┬──────────────┬──────────────┬──────────────┬──────────────┐
   │              │              │              │              │              │
   v              v              v              v              v              v
┌──────────┐ ┌──────────────┐ ┌───────────────┐ ┌──────────────┐ ┌──────────────┐
│bpane-host│ │bpane-gateway │ │  mcp-bridge   │ │   Keycloak   │ │   Postgres   │
│ .0.10    │ │ :4433 (QUIC) │ │ :8931 (SSE)   │ │ :8080/.8091  │ │ :5432/.5433  │
│          │ │ :8932 (HTTP) │ │               │ │ local OIDC   │ │ control-plane│
│ Xorg :99 │ │              │ │ @playwright/  │ │ realm        │ │ state        │
│ OpenBox  │ │ Unix socket  │ │ mcp (STDIO)   │ │              │ │              │
│ Chromium │ │ <-> host IPC │ │               │ │              │ │              │
│ PipeWire │ │              │ │ Supervisor    │ │              │ │              │
│ FFmpeg   │ │ Session store │ │ monitor       │ │              │ │              │
│bpane-host│ │ + auth API    │ │ (polls API)   │ │              │ │              │
│          │ │               │ │               │ │              │ │              │
│ CDP :9222│ │ Max 10        │ │ MCP clients   │ │              │ │              │
│ -> :9223 │ │ viewers       │ │ connect here  │ │              │ │              │
└──────────┘ └──────────────┘ └───────────────┘ └──────────────┘ └──────────────┘
      │               ^
      └───────────────┘
       /run/bpane/agent.sock
```

**Ports exposed to host machine:**
- `8080/tcp` — nginx (web UI, auth config, cert hash, SDK dist)
- `4433/tcp+udp` — WebTransport (QUIC)
- `8932/tcp` — gateway HTTP API
- `8091/tcp` — local Keycloak realm for dev/testing
- `8931/tcp` — MCP bridge (SSE)
- `5433/tcp` — local Postgres for the session control plane

**Host container internals:** Xorg with dummy video driver (3840x2160 virtual
framebuffer, runtime resizable via xrandr), OpenBox WM (locked down, no
keybinds), Chromium from xtradeb PPA (software rendering via SwiftShader,
CDP on :9222 proxied to :9223 via socat), PipeWire + PulseAudio compat
(null sink `bpane-desktop` at 48 kHz/stereo/S16LE), FFmpeg for both screen
capture and audio capture, and the `bpane-host` Rust binary.

---

## Components

### bpane-host (~15,200 lines Rust)

The host agent runs inside the Linux environment being shared. It is the most
complex component — roughly 70% of the Rust codebase.

**Capture pipeline:**
- Connects to X11 display via `x11rb` (RustConnection)
- XDamage tracks incremental screen changes (8ms coalesce window)
- XComposite redirects subwindows for reliable capture
- MIT-SHM shared memory for zero-copy pixel access (XRGB8888)
- XFixes cursor shape/position tracking (adaptive 10-60Hz polling)

**Tile system (~2,200 lines, 2 modules — `tiles/mod.rs` + `tiles/emitter.rs`):**
This is the architectural centerpiece. The screen is divided into a grid of
64x64 pixel tiles. Each frame, the system:

1. **Damages** — XDamage bounding box narrows which tiles to examine
2. **Captures** — SHM GetImage pulls pixels for damaged tiles only
3. **Hashes** — xxHash3 per-tile for change detection
4. **Deduplicates** — two levels:
   - Per-position: skip if hash unchanged at same (col, row)
   - Content-addressable: detect hash reuse across positions (scroll optimization)
5. **Encodes** — multi-codec strategy per tile:
   - Solid color -> `Fill` (~9 bytes)
   - Hash match -> `CacheHit` (~13 bytes)
   - UI content -> `Qoi` or `Zstd` (1-10 KB)
   - Video region -> deferred to H.264 on separate channel
6. **Detects scrolls** — compares tile hash columns to find vertical/horizontal
   displacement, emits `ScrollCopy` + `GridOffset` instead of re-encoding

**H.264 pipeline:**
- FFmpeg x11grab subprocess captures screen -> libx264 encode
- NAL unit extraction from Annex B byte stream
- Keyframe every ~250ms (GOP 15 at 60fps) for fast artifact recovery
- Configurable bitrate, preset, profile via environment variables
- Three modes via `BPANE_H264_MODE`: `always` (full screen), `video_tiles`
  (only CDP-detected video regions), `off` (tiles only)

**CDP integration (~2,000 lines, `cdp_video.rs`):**
- Connects to Chromium DevTools Protocol (WebSocket)
- Extracts `<video>` element bounds for targeted H.264 encoding
- Reads scroll position for scroll detection tuning
- Pauses page videos during scroll (reduces capture noise)

**Input injection:**
- XTest extension for keyboard and mouse events
- evdev keycodes from browser `KeyboardEvent.code`
- Absolute mouse positioning (client coords -> screen coords)
- Keyboard layout passthrough via CDP `InsertText` for Unicode codepoints

**Audio capture (~1,100 lines, `audio/`):**
- PipeWire/PulseAudio -> FFmpeg -> configurable codec
- Three codecs: Opus (64 kbps CBR, default), IMA-ADPCM (4:1), raw PCM
- Codec-tagged frame format: `[magic: "WRA1"][codec: u8][reserved][payload]`
- Silence gate: -50 dBFS threshold, 220ms hangover
- Graceful degradation if PipeWire unavailable

**Microphone input (`audio/input.rs`):**
- Receives Opus audio from browser via CH_AUDIO_IN
- Pipes decoded audio to host application via loopback

**Camera ingress (`camera.rs`):**
- Receives H.264 access units from browser via CH_VIDEO_IN
- Spawns FFmpeg to decode H.264 -> v4l2loopback virtual camera
- Chromium's `getUserMedia()` sees it as a real webcam
- Disabled by default in compose; requires `v4l2loopback` on host

**File transfer (`filetransfer.rs`, ~600 lines):**
- Upload: browser sends FileHeader -> FileChunk (64 KB) -> FileComplete
- Download: monitors directory for new files, sends chunked
- Session-specific upload directories, size validation

**Clipboard sync (`clipboard.rs`):**
- X11 CLIPBOARD selection via XFixes SelectionNotify
- FNV-1a hash for echo prevention (avoids round-tripping own writes)
- Max 1 MiB text payloads

### bpane-gateway (~2,800 lines Rust)

Stateless relay between host agent and browser clients.

- **WebTransport server** (wtransport crate, QUIC + TLS on port 4433)
- **Session hub** (`session_hub.rs`, ~850 lines): one host agent, N browser clients
  - Broadcast: host -> all clients (tokio broadcast channel, capacity 1024)
  - Merge: all clients -> host (mpsc channel)
  - Caches SessionReady + last keyframe + grid config for late joiners
  - Browser sessions are collaborative by default
  - If `--exclusive-browser-owner` is enabled, the first browser is owner and later browser clients are viewers
  - Viewer cap: configurable via `--max-viewers` (default 10)
- **Session registry** (`session_registry.rs`): manages hub lifecycle with
  TOCTOU-safe concurrent join (two-phase lock pattern)
- **Session control** (`session_control.rs`): owner-scoped versioned session resources with:
  - Postgres-backed persistence in normal runtime
  - in-memory backend fallback for tests and dev fallback mode
  - session-scoped connect metadata and routing keyed by public `session_id`
  - `legacy_single_runtime` compatibility gating so Phase 0 can expose session resources before true multi-session workers land
- **Runtime manager** (`runtime_manager.rs`): resolves `session_id -> runtime endpoint`
  - current backends are:
    - `static_single`: one shared host socket, with idle release semantics in the gateway
    - `docker_single`: opt-in Docker-backed worker startup/shutdown for the active session, with idle timeout and one active runtime at a time
  - this is still the seam where true multi-session worker pooling and runtime caps will land
- **MCP ownership**: atomic flag that locks resolution for browser clients
  when an MCP agent owns the session
- **Auth** (`auth.rs`): OIDC/JWT validation for browser and API clients, plus legacy HMAC token compatibility for migration and tests
- **Heartbeat**: disconnects after 15s without CONTROL ping
- **HTTP API** (`api.rs`, :8932):
  - `POST /api/v1/sessions` — create a persistent session resource
  - `GET /api/v1/sessions` — list owner-scoped sessions
  - `GET /api/v1/sessions/{id}` — fetch one owner-scoped session resource
  - `DELETE /api/v1/sessions/{id}` — stop one owner-scoped session resource
  - `POST /api/v1/sessions/{id}/access-tokens` — mint a short-lived session-scoped connect ticket
  - `POST /api/v1/sessions/{id}/automation-owner` — delegate one session to an automation principal
  - `DELETE /api/v1/sessions/{id}/automation-owner` — clear automation delegation
  - `GET /api/v1/sessions/{id}/status` — session-scoped runtime telemetry for compatibility mode
  - `POST /api/v1/sessions/{id}/mcp-owner` — session-scoped MCP ownership claim
  - `DELETE /api/v1/sessions/{id}/mcp-owner` — session-scoped MCP ownership release
  - `GET /api/session/status` — client counts, resolution, telemetry
  - `POST /api/session/mcp-owner` — claim session, lock resolution
  - `DELETE /api/session/mcp-owner` — release ownership
  - all current endpoints require `Authorization: Bearer <token>`
- **Relay** (`relay.rs`): bidirectional Unix socket <-> async bridge, 64 KB read
  buffer, zero-copy frame slicing with `Bytes`

### bpane-client (~6,500 lines TypeScript)

Runs in Chromium desktop. No WASM runtime — pure TypeScript with fzstd for Zstd
decompression.

- **Session lifecycle** (`bpane.ts`, ~1,300 lines):
  - `BpaneSession.connect(options)` factory — probes mic/camera support first
  - `accessToken`-based connect path (legacy `token` still accepted for compatibility)
  - ResizeObserver -> 150ms debounce -> ResolutionRequest
  - Canvas pixel dimensions = container pixel dimensions (optional HiDPI scaling)
  - WebTransport connection with cert hash fetching for self-signed certs
  - Channel multiplexing across reliable streams + datagrams
  - Ping/pong heartbeat (5s interval)
  - Resolution lock awareness for viewer clients
  - Feature callbacks: `onConnect`, `onDisconnect`, `onError`,
    `onCapabilitiesChange`, `onResolutionChange`

- **WebGL tile renderer** (`webgl-compositor.ts`, ~760 lines):
  - WebGL 2 GPU-accelerated rendering (preferred path)
  - Vertex/fragment shader program for tile rects and solid fills
  - Persistent GPU textures for video frames (zero-copy upload on Chrome)
  - Framebuffer-based scroll blit (GPU copy for ScrollCopy)
  - Software renderer detection (SwiftShader, llvmpipe, Lavapipe)
  - Automatic fallback to Canvas 2D if WebGL 2 unavailable or software-only
  - Diagnostics: renderer name, vendor, backend selection reason

- **Tile compositor** (`tile-compositor.ts`, ~490 lines):
  - Processes batched tile commands in frame-sequence order
  - Promise-chain serialization for async decode/draw
  - Epoch-based staleness (ignores completions after grid reset)
  - Dispatches to WebGL renderer or Canvas 2D context
  - Redundant QOI/Zstd detection for scroll batch optimization

- **Tile cache** (`tile-cache.ts`, ~300 lines):
  - LRU hashmap: 8,192 entries, 50 MB cap (dual-limit eviction)
  - Stores `ImageBitmap` (Chrome GPU path) or `ImageData` (fallback)
  - Hit/miss/eviction counters for telemetry

- **NAL reassembly** (`nal.ts`, ~160 lines):
  - Reassembles fragmented H.264 NALs from datagrams (QUIC MTU ~1200 bytes)
  - Ring-buffer deduplication (128-entry window)
  - VideoTileInfo parsing for partial-screen video compositing

- **Input** (`input-controller.ts`, ~1,100 lines + `input-map.ts`, ~130 lines):
  - KeyboardEvent.code -> evdev keycode mapping (comprehensive table)
  - Dead-key composition with 16ms timeout for accent generation
  - macOS-specific remapping: Command -> Ctrl, Option -> Alt/AltGr
  - Synthetic dead-accent generation for macOS backtick/circumflex
  - Scroll normalization with fractional accumulation (60px = 1 step)
  - 16ms mouse move throttle
  - Clipboard sync with FNV-1a echo prevention

- **Audio** (`audio-controller.ts`, ~520 lines + `audio-worklet.ts`, ~100 lines):
  - Decodes Opus (via WebCodecs AudioDecoder), IMA-ADPCM, raw PCM
  - AudioWorklet playback with ring buffer (1s capacity)
  - Jitter management: 110ms pre-buffer, 200ms overflow drop
  - Microphone capture: `getUserMedia()` -> Opus encode (32 kbps) -> CH_AUDIO_IN

- **Camera** (`camera-controller.ts`, ~500 lines):
  - WebCodecs H.264 encode from `getUserMedia()` video
  - Three profiles: 720p/30fps/1.6Mbps, 540p/24fps/950kbps, 360p/18fps/450kbps
  - Adaptive bitrate: monitors `qualityLimitationReason`, downgrades on
    bandwidth/CPU pressure, upgrades after stability window
  - Frame replacement on queue overflow (latest-wins)

- **File transfer** (`file-transfer.ts`, ~370 lines):
  - Upload via drag-drop or file picker, 64 KB chunks
  - Download auto-saves via Blob URL
  - Wire format: FileHeader (metadata) -> FileChunk (data) -> FileComplete

- **Session stats** (`session-stats.ts`, ~500 lines):
  - Per-channel transfer counters (bytes, frames)
  - Tile command type counters, cache hit rates
  - Scroll health tracking (fallback rate, saved tiles, rolling windows)
  - Video frame decode/drop counts
  - Camera encode telemetry

### mcp-bridge (~340 lines TypeScript)

Optional. Bridges external MCP clients (e.g., Claude Code) to Playwright MCP
running against the Chromium instance inside the host container.

- SSE server for MCP client connections (per-connection `Server` instances)
- Proxies tool calls to @playwright/mcp subprocess (STDIO mode)
- Supervisor-aware: adds configurable delay (default 1500ms) when browser
  viewers are watching (polls gateway status every 2s)
- Lazy registration: only claims MCP ownership on first SSE client connect
- Registers/clears MCP ownership with gateway (resolution lock)
- Uses OIDC client-credentials for gateway API access in the local compose stack
- Exposes a local control-session API on `:8931` so the browser test page can point
  the bridge at an explicitly delegated session without restarting the service
- Graceful shutdown: always releases ownership on SIGINT/SIGTERM

---

## Local Auth Flow

The default dev stack no longer uses a shared token file.

- `web` serves `/auth-config.json`
- `test-embed.html` discovers the OIDC provider and performs Authorization Code + PKCE
- local browser users authenticate against Keycloak on `http://localhost:8091`
- after login, `test-embed.html` resolves or creates an owner-scoped `/api/v1/sessions` resource and uses its returned connect metadata
- the page then mints a short-lived `session_connect_ticket` through `POST /api/v1/sessions/{id}/access-tokens`
- `Delegate MCP` calls `POST /api/v1/sessions/{id}/automation-owner` for the local `bpane-mcp-bridge` principal and then assigns that same session to `mcp-bridge` via `PUT /control-session`
- the resulting access token is sent to `bpane-gateway` as:
  - HTTP API bearer token for authenticated control calls
- the browser transport then uses the minted ticket as:
  - WebTransport query param: `session_ticket=...`
- `bpane-gateway` resolves that ticket back to the delegated or owner-visible `session_id` before admitting the transport
- `mcp-bridge` obtains its own bearer token with client credentials
- the versioned session API is also bearer-protected and owner-scoped
- the current session resource connect contract advertises `auth_type: session_connect_ticket` and still carries `compatibility_mode: legacy_single_runtime`
- the default compose stack still runs the `static_single` runtime backend, so that control-plane flow still lands on one active host worker
- an opt-in `docker_single` runtime backend now exists for start/stop-on-idle worker lifecycle, but it still enforces a single active runtime
- `mcp-bridge` has an optional session-control bootstrap (`BPANE_SESSION_ID` / `BPANE_SESSION_BOOTSTRAP_MODE`) and now also supports explicit delegated-session assignment through its local `/control-session` API

The default imported local realm contains:

- browser client: `bpane-web`
- gateway audience client: `bpane-gateway`
- service-account client: `bpane-mcp-bridge`
- example user: `demo / demo-demo`

### Wire Protocol (bpane-protocol, ~2,800 lines Rust)

Shared crate compiled to both native and WASM. Defines all message types and
binary framing. `no_std` compatible.

**Frame envelope:**
```
+-------------+--------------+---------------------+
| channel: u8 | length: u32  | payload: [u8; length]|
+-------------+--------------+---------------------+
```
All integers little-endian. Max payload 16 MiB. No JSON, no protobuf, no serde.

**11 channels:**

| ID | Channel | Transport | Direction | Purpose |
|----|---------|-----------|-----------|---------|
| 0x01 | Video | Datagrams | S->C | H.264 NAL fragments |
| 0x02 | AudioOut | Stream | S->C | Desktop audio (Opus/ADPCM/PCM) |
| 0x03 | AudioIn | Stream | C->S | Microphone (Opus) |
| 0x04 | VideoIn | Stream | C->S | Webcam (H.264) |
| 0x05 | Input | Stream | C->S | Mouse, keyboard, scroll |
| 0x06 | Cursor | Stream | S->C | Shape + position |
| 0x07 | Clipboard | Stream | Bidi | Text sync |
| 0x08 | FileUp | Stream | C->S | Upload chunks |
| 0x09 | FileDown | Stream | S->C | Download chunks |
| 0x0A | Control | Stream | Bidi | Resize, ping, session, bitrate hints |
| 0x0B | Tiles | Stream | S->C | Tile rendering commands |

**Control message types (8 variants):**
ResolutionRequest, ResolutionAck, SessionReady, Ping, Pong,
KeyboardLayoutInfo, BitrateHint, ResolutionLocked.

**Tile message types (12 variants):**
GridConfig, CacheHit, CacheMiss, Fill, Qoi, Zstd, VideoRegion, BatchEnd,
ScrollCopy, GridOffset, TileDrawMode, ScrollStats.

**Video datagram format:**
```
nal_id(u32) + fragment_seq(u16) + fragment_total(u16) + is_keyframe(u8)
+ pts_us(u64) + data_len(u32) + data + [flags(u8) + tile_info(12 bytes)]
```

---

## Data Flow

### Static Content (text, UI)
```
XDamage event
  -> SHM GetImage (damaged tiles only)
  -> xxHash3 per tile
  -> hash unchanged? -> skip (zero bytes)
  -> hash seen before at different position? -> CacheHit (13 bytes)
  -> solid color? -> Fill (9 bytes)
  -> encode Qoi/Zstd -> Qoi/Zstd message (1-10 KB)
  -> BatchEnd (frame sequence number)
```

### Scroll
```
Scroll input event
  -> CDP scroll position delta (or tile hash column comparison)
  -> ScrollCopy(dx, dy, region bounds)
  -> GridOffset(offset_x, offset_y)
  -> encode only newly exposed edge tiles
```

### Video Content
```
CDP detects <video> element bounds
  -> SetRegion narrows FFmpeg x11grab to video area
  -> H.264 encode (only the video rectangle)
  -> NAL fragment -> VideoDatagram with VideoTileInfo
  -> client composites at video region bounds over tile layer
```

### Input (browser -> host)
```
KeyboardEvent.code -> evdev keycode + modifiers
  -> InputMessage::KeyEvent (or KeyEventEx with key_char for Unicode)
  -> reliable stream -> gateway -> Unix socket -> XTest injection
  -> (KeyEventEx with codepoint -> CDP InsertText for layout passthrough)
```

### Audio (host -> browser)
```
Desktop app -> PipeWire null sink (bpane-desktop)
  -> FFmpeg captures monitor source
  -> Encode: S16LE -> Opus (default) or IMA-ADPCM
  -> Codec-tagged frame [WRA1 magic + codec byte + payload]
  -> CH_AUDIO_OUT stream -> gateway -> browser
  -> AudioDecoder (Opus) or JS decode (ADPCM)
  -> AudioWorklet ring buffer -> speakers
```

### Camera (browser -> host)
```
getUserMedia() -> canvas draw -> VideoFrame
  -> VideoEncoder (H.264 baseline, adaptive profile)
  -> CH_VIDEO_IN stream -> gateway -> Unix socket
  -> FFmpeg H.264 -> v4l2loopback virtual camera
  -> Chromium getUserMedia() sees webcam
```

---

## Rendering Pipeline (Client)

The client uses a two-layer compositing model:

```
+-------------------------------------------+
|          Cursor overlay canvas             |  (top, separate z-index)
+-------------------------------------------+
|   Video texture   |                        |  (WebGL persistent texture,
|   (H.264 decode)  |    Tile layer          |   composited over tiles every
|                    |    (QOI/Zstd/Fill/     |   rAF frame)
|                    |     CacheHit draws)    |
+-------------------------------------------+
```

**Display loop** (requestAnimationFrame):
1. If pending VideoFrame -> upload to GPU texture (WebGL) or draw to buffer (Canvas 2D)
2. Composite video over tiles at video region bounds
3. Only render if `displayDirty` flag set (tile batch arrived or video frame decoded)

**WebGL path (preferred):**
- `fillRect()` — uniform color, no texture bind
- `drawTile()` — texture upload from ImageBitmap/ImageData
- `uploadVideoFrame()` — zero-copy VideoFrame -> GPU texture on Chrome
- `scrollCopy()` — FBO blit: read from scroll texture, write shifted to canvas

**Canvas 2D path (fallback):**
- `drawImage()` / `fillRect()` for tiles
- Scratch canvas for scroll copy operations
- Used when WebGL 2 unavailable or only software renderer detected

---

## What We Understood From Adjacent Systems

The notes below are directional takeaways from studying classic VNC/RFB stacks,
noVNC, Apache Guacamole, and Kasm/KasmVNC. They are not formal benchmarks and
should not be read as exhaustive claims about those projects.

### Compared With Classic VNC / RFB

| Aspect | Classic VNC / RFB | BrowserPane |
|--------|--------------------|-------------|
| Transport | TCP | QUIC via WebTransport |
| Update model | Framebuffer updates using standard encodings like Raw, CopyRect, Hextile, and ZRLE, plus many implementation-specific extensions | Explicit multi-channel protocol with tiles, ROI video, audio, clipboard, and file transfer |
| Resize | Standard `DesktopSize` plus common extensions such as `ExtendedDesktopSize` depending on implementation | Built-in `ResolutionRequest` / `ResolutionAck`, container-driven sizing |
| Browser client story | Usually via a web client such as noVNC, typically over WebSocket | Browser-native client designed around WebTransport, WebCodecs, and WebGL 2 |

### Where BrowserPane Seems Strong

**Embed-first sizing.** BrowserPane is designed around the idea that a remote
desktop should behave like a web component. Container-driven sizing is central
to the protocol instead of an afterthought.

**Mixed-content rendering.** BrowserPane treats text/UI, scroll movement, and
video as different classes of content. The tile/cache pipeline plus targeted
video overlay can reduce redundant work on UI-heavy pages.

**Transport separation.** QUIC/WebTransport lets this design separate reliable
state from lossy media, which is a better fit for a browser-native client than
a single ordered TCP stream.

**Built for supervised interaction.** Clipboard, file transfer, mic, camera,
and MCP-driven supervision are first-class parts of this codebase rather than
external add-ons.

### Where Mature VNC-Style Systems Still Win

**Maturity and ecosystem.** RFB-based systems have decades of operational
history, multiple mature server/client implementations, and far broader field
testing.

**Compatibility.** Traditional VNC-style stacks run across more operating
systems, more deployment shapes, and more browsers today than BrowserPane.

**Operational simplicity.** A minimal VNC stack can be simpler to deploy and
debug than BrowserPane's host + gateway + browser client + optional MCP bridge.

**Less bespoke logic.** BrowserPane's tile pipeline, scroll reuse, CDP
integration, and browser media plumbing are powerful, but they also introduce
more custom code and therefore more surface area for bugs.

---

## Honest Assessment

### What Works Well

- **Tile deduplication is effective.** On static or slowly-changing content
  (dashboards, documents, IDEs), bandwidth usage is dramatically lower than
  full-frame encoding. CacheHit at 13 bytes vs re-encoding an unchanged tile
  at 2-8 KB is a real win.

- **Scroll detection is clever.** Hash-column comparison for displacement
  detection is fast and avoids the complexity of optical flow. It works
  reliably for the common case (vertical page scroll) and degrades gracefully
  to full re-encode when it can't find a match.

- **Container-driven sizing is the right call.** Embedding a remote desktop
  as a responsive web component (not a fixed-size viewer) is a genuine UX
  improvement over VNC/RDP viewers.

- **WebTransport is a strong fit for this architecture.** Independent streams,
  datagram support for lossy video, and built-in TLS line up well with the way
  BrowserPane separates state, media, and control.

- **The MCP supervision model is distinctive.** Allowing an AI agent to control
  the desktop via Playwright while a human watches through the same session is
  a meaningful differentiator for this codebase.

- **WebGL 2 rendering.** GPU-accelerated tile compositing with persistent video
  textures and framebuffer scroll blits reduces client CPU usage and enables
  smooth 60fps compositing.

- **Opus audio.** Full Opus codec support (64 kbps CBR) provides good quality
  desktop audio at low bandwidth. Silence gating eliminates idle audio traffic.

- **Camera/microphone bidirectional media.** H.264 camera ingress via WebCodecs
  with adaptive bitrate profiles and Opus microphone capture make BrowserPane a
  bidirectional media channel, not just a screen viewer.

### What Needs Work

- **Idle CPU is acceptable but not great.** At ~5-6% idle with the current
  optimizations (down from ~50%), it's usable but not invisible. The main
  cost is Chromium's software rendering (SwiftShader) and XDamage-driven
  tile capture. A GPU-accelerated environment would change the picture
  significantly.

- **Single-browser, single-OS.** Chromium-only in practice today and Linux-only
  on the host side limits the addressable market. The underlying web standards
  are broader now, but this project does not target Firefox or Safari for
  production.

- **No hardware encode path yet.** The H.264 pipeline uses libx264 (CPU).
  VAAPI hardware encoding is specced but not implemented. For video-heavy
  workloads in production, this matters.

- **Gateway is simple but limited.** Single-agent sessions, in-memory state,
  no persistence. Fine for dev/demo but not production-grade. No reconnection
  support — if the WebTransport connection drops, the session is gone.

- **No end-to-end testing of the visual pipeline.** The protocol has unit tests
  and integration tests for framing. The tile compositor has unit tests. But
  there's no automated test that captures a known screen, sends it through the
  full pipeline, and verifies the client rendered it correctly. Visual
  correctness is currently validated manually.

### What We Took Away From noVNC, Guacamole, and Kasm/KasmVNC

- **noVNC** prioritizes compatibility and reuse of existing VNC infrastructure.
  It fits well when the goal is "make a VNC session available in the browser"
  with minimal invention.

- **Apache Guacamole** prioritizes broad protocol coverage and deployability.
  It is a mature HTML5 remote-access gateway with a wider compatibility story
  than BrowserPane.

- **Kasm/KasmVNC** already pushes beyond classic VNC assumptions. It uses more
  modern browser-side technology than a plain "Canvas over WebSocket" mental
  model suggests, and it should not be caricatured as a basic legacy VNC stack.

- **BrowserPane** is narrower and more opinionated. The interesting part is not
  "remote desktop in a browser" by itself, but the combination of embed-first
  sizing, tile-first rendering, targeted video, and supervised human/AI control.

BrowserPane occupies a niche: embeddable remote browser surfaces for controlled
Linux container sessions, especially when human supervision and AI-driven
operation need to coexist.

That means the project is not trying to win on breadth. It is trying to be very
good at one specific shape of problem. For general remote desktop needs, mature
VNC-style systems or Kasm are still the more pragmatic default today.
