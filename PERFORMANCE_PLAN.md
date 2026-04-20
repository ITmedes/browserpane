# BrowserPane Performance Plan

This file is the current performance plan for the BrowserPane stack:

- `code/apps/bpane-host`
- `code/apps/bpane-gateway`
- `code/shared/bpane-protocol`
- `code/web/bpane-client`
- `code/integrations/mcp-bridge`
- deploy/runtime configuration

It is intentionally based on the current code state after the large `bpane-client`
refactor branch, not on older file layouts.

## Intent

- Focus on changes that are likely to improve end-to-end user-visible latency.
- Prefer measured, high-signal work over speculative micro-tuning.
- Keep safety and correctness ahead of “clever” zero-copy rewrites.
- Separate:
  - already completed work
  - still-valid opportunities
  - ideas that are now stale or wrong for the current architecture

## Priority Legend

- `P0` High impact, low-to-moderate implementation risk. Do first.
- `P1` Good impact or strong infrastructure value. Do next.
- `P2` Moderate impact, higher effort, or needs measurement first.
- `P3` Opportunistic or production-only follow-up.

## Current Status

The branch already removed several obvious client-side hot-path copies and allocations:

- avoided extra WebTransport datagram copies in `session-transport-runtime.ts`
- avoided extra NAL fragment copies in `nal.ts`
- introduced callback-style frame parsing in `protocol.ts`
- reduced zstd tile repair allocations in `zstd-tile-renderer.ts`
- reused tile draw rect storage in `tile-draw-runtime.ts`
- avoided tile batch array copies in `tile-batch-sequencer.ts`
- cached repeated Canvas2D fill styles in `fill-tile-renderer.ts`

Those should not be planned again as pending work.

## Guiding Rules

1. Do not “optimize” by creating lifetime bugs.
2. Do not replace copies with views unless payload ownership is still correct.
3. Prefer subsystem-local changes before protocol-wide redesigns.
4. Before larger refactors, add measurement and regression checks.

## Measurement First

Before any broad redesign, add or improve measurements for:

- heavy-scroll repair behavior on `main` vs current branch
- WebGL vs Canvas2D on the same page/workload
- scroll-copy on/off on the same page/workload
- host capture timings
- gateway lag / queue pressure
- client tile decode / draw / batch timings

### P0.1 Add a repeatable branch-vs-main benchmark workflow

**Problem**

The current performance conversation still depends too much on manual feel.

**What to add**

- a fixed heavy-scroll benchmark page/workflow
- one scriptable local benchmark for:
  - `main`
  - current branch
  - `Render=auto`
  - `Render=canvas2d`
  - `scrollCopy=true/false`
- stable capture of:
  - connect time
  - scroll-repair time
  - dropped-frame / late-repair symptoms

**Why**

This is the only reliable way to justify the larger tile-path redesign work below.

## 1. Host Capture Pipeline

### P0.2 Remove full-frame BGRA->RGBA swap from X11 capture

**Location**

- `code/apps/bpane-host/src/capture/x11.rs`
- `code/apps/bpane-host/src/tiles/emitter.rs`

**Current**

The capture backend still performs a full framebuffer BGRA->RGBA swap for every captured frame:

```rust
for pixel in data.chunks_exact_mut(4) {
    pixel.swap(0, 2);
}
```

But tile emission already has per-tile extraction and per-tile BGRA->RGBA conversion support.

**Plan**

- stop converting the entire captured frame up front
- keep captured frame data in source pixel layout
- convert only the tiles that are actually emitted

**Expected gain**

This is still one of the strongest remaining end-to-end CPU wins in the stack.

### P1.1 Reuse tile emission buffers in host hot paths

**Location**

- `code/apps/bpane-host/src/tiles/emitter.rs`

**Current**

Hot paths still allocate fresh `Vec`s in frame emission paths.

**Plan**

- move per-frame `Vec` allocations onto reusable struct fields
- `clear()` and reuse them across frames

**Expected gain**

Lower allocator churn in one of the hottest host paths.

### P2.1 Reuse tile coordinate accumulation buffers

**Location**

- `code/apps/bpane-host/src/tiles/mod.rs`

**Current**

Tile coordinate helpers still allocate intermediate vectors repeatedly.

**Plan**

- move more coordinate collection to caller-owned scratch buffers

**Expected gain**

Moderate host-side allocation reduction during active damage bursts.

## 2. Host Encoding And Cache Behavior

### P2.2 Tune sent-hash cache size for larger displays

**Location**

- `code/apps/bpane-host/src/tiles/emitter.rs`

**Current**

`MAX_SENT_HASHES = 8192` may be too low for high-resolution and scroll-heavy workloads.

**Plan**

- make it configurable
- expose eviction telemetry
- tune from measurement instead of guessing

### P3.1 Revisit H.264 encode path after tile path is stable

**Location**

- `code/apps/bpane-host/src/encode/software.rs`
- broader architecture

**Plan**

- profile frame-object reuse opportunities
- defer hardware encode exploration until tile and transport hotspots are better characterized

**Why**

The current user-visible issues have mostly been tile/repair oriented, not obviously H.264 encode bound.

## 3. Gateway Transport And Session Hub

### P0.3 Remove avoidable frame cloning in session-hub pump

**Location**

- `code/apps/bpane-gateway/src/session_hub/pump.rs`

**Current**

The pump still caches `Arc::new(frame.clone())` separately from the `Arc` sent to broadcast consumers.

**Plan**

- create one `Arc<Frame>`
- reuse it for both:
  - cache storage
  - broadcast

**Expected gain**

Removes avoidable frame copies on a very active path.

### P1.2 Consolidate cache lock scopes in the session hub

**Location**

- `code/apps/bpane-gateway/src/session_hub/pump.rs`

**Current**

The pump still takes multiple async mutex locks per frame for different cached state fields.

**Plan**

- combine cached state under fewer locks
- ideally update all relevant cache state in one lock scope

**Expected gain**

Lower per-frame await/lock overhead and lower contention under multi-client load.

### P1.3 Reduce send-stream lock contention in gateway egress

**Location**

- `code/apps/bpane-gateway/src/transport/egress.rs`
- `code/apps/bpane-gateway/src/transport/tasks.rs`

**Current**

Per-frame writes still lock the send stream repeatedly.

**Plan**

- move toward a dedicated writer task per client, or
- batch writes under fewer lock acquisitions

**Expected gain**

Lower latency variance and better scaling with multiple viewers.

### P2.3 Revisit relay channel sizing and memory bounds

**Location**

- `code/apps/bpane-gateway/src/relay.rs`
- `code/apps/bpane-gateway/src/transport.rs`

**Current**

Relay channels and concurrent-session limits are still static and broad.

**Plan**

- tune channel sizes from measurement
- consider making concurrent-session limits configurable

**Expected gain**

Better memory discipline and less worst-case queue growth.

## 4. Client Reliable Tile Path

This is now the biggest remaining client-side performance area.

The major remaining issue is no longer a simple copy; it is object churn in the reliable tile repair path.

### P0.4 Redesign the `TileCommand` parse/queue/apply path around streaming ownership

**Location**

- `code/web/bpane-client/js/render/tile-message-parser.ts`
- `code/web/bpane-client/js/tile-compositor.ts`
- `code/web/bpane-client/js/render/tile-batch-command-applier.ts`
- `code/web/bpane-client/js/session-frame-router-runtime.ts`

**Current**

The client still creates and queues many short-lived `TileCommand` objects on the hottest reliable render path.

This is likely the biggest remaining JavaScript allocation hotspot during heavy scroll repair.

**Important constraint**

Do **not** blindly replace payload `slice()` with `subarray()` in the current parser.
That would be unsafe with the current reusable receive buffer and queued-batch model.

**Plan**

Redesign this area only with one of these safe ownership models:

- parse straight into a batch-owned command arena
- parse/apply in a more streaming-oriented way while preserving frame sequence behavior
- or explicitly transfer batch payload ownership before using views

**Expected gain**

High. This is the next meaningful client-side optimization target after the already-completed low-risk work.

### P1.4 Add direct instrumentation to the tile repair path

**Location**

- `tile-message-parser.ts`
- `tile-compositor.ts`
- `tile-draw-runtime.ts`
- session stats / diagnostics

**Plan**

Measure:

- parsed tile commands per batch
- average queued commands per batch
- QOI vs zstd repair mix
- repair decode counts
- stale decode count
- batch apply time

**Why**

This gives a measurement baseline before changing the command model.

### P2.4 Revisit redundant decode avoidance after the command-path redesign

**Location**

- `qoi-tile-renderer.ts`
- `zstd-tile-renderer.ts`

**Current**

There may still be room to skip decode for known-redundant tiles in some flows, but this should be revisited after the command and batch model is clarified.

**Why**

The current branch already reduced some decode-side overhead. The next bigger gain is likely above this layer.

## 5. Client Video And Audio Follow-Up

### P1.5 Bound `tileInfoByTimestamp`

**Location**

- `code/web/bpane-client/js/session-video-decoder-runtime.ts`

**Current**

`tileInfoByTimestamp` can still grow if decode output/cleanup is disrupted.

**Plan**

- cap size, or
- evict old entries based on timestamp / age

**Expected gain**

Mostly memory safety and runaway-protection, not headline latency.

### P2.5 Reduce remaining Opus playback copies

**Location**

- `code/web/bpane-client/js/audio/opus-playback-runtime.ts`

**Current**

The runtime already reuses internal buffers, but it still copies:

- input packet via `packet.slice(0)`
- final sample window via `output.slice(...)`

**Plan**

- validate whether the input copy is required by `EncodedAudioChunk`
- see whether the final sample handoff can use transfer ownership instead of slicing

**Expected gain**

Moderate. Useful after tile-path work, not before.

### P3.2 Review audio worklet telemetry

**Location**

- `code/web/bpane-client/js/audio/audio-worklet.js`

**Plan**

- surface overflow/drop signals back to session stats

**Why**

This improves diagnosis more than raw performance.

## 6. Deploy / Static Delivery

### P0.5 Enable gzip in nginx

**Location**

- `deploy/nginx.conf`

**Current**

No compression is configured for static assets.

**Plan**

- enable gzip for JS/CSS/text assets

**Expected gain**

Immediate reduction in frontend transfer size with low implementation risk.

### P1.6 Rework cache headers only after versioned asset naming is in place

**Location**

- `deploy/nginx.conf`
- `code/web/bpane-client/vite.config.ts`

**Current**

The frontend currently ships stable names like `/dist/bpane.js`, not hashed assets.

**Plan**

- do **not** add immutable one-year cache headers yet
- first decide whether the client should emit versioned or hashed filenames
- then add long-lived caching for those assets only

**Expected gain**

Good deploy/runtime behavior, but only once the asset naming model is correct.

### P2.6 Revisit local compose memory limits separately from production runtime guidance

**Location**

- `deploy/compose.yml`

**Current**

The local compose stack does not define memory limits, but the right configuration mechanism depends on whether the target is local `docker compose` or a production orchestrator.

**Plan**

- document local-dev expectations separately
- add limits only with the right runtime semantics

## 7. MCP Bridge

### P1.7 Add fetch timeout and backoff to supervisor polling

**Location**

- `code/integrations/mcp-bridge/src/supervisor-monitor.ts`

**Current**

Polling still has no timeout and no backoff.

**Plan**

- use `AbortController`
- add exponential backoff on repeated failures
- reset on success

**Expected gain**

Better resilience and lower noisy load during failure modes.

### P2.7 Add explicit client-count and idle limits for SSE connections

**Location**

- `code/integrations/mcp-bridge/src/index.ts`

**Current**

Connections are cleaned up on close, so this is not a leak, but there is still no explicit upper bound or idle discipline.

**Plan**

- add a client cap
- add idle or liveness policy if needed

**Expected gain**

Predictable memory/use behavior under abuse or accidental misuse.

## 8. Plan Items That Are Already Done

These should remain recorded for history, but they are no longer pending:

- client datagram copy reduction
- NAL copy reduction
- callback-style frame parsing in `protocol.ts`
- zstd tile repair allocation reduction
- tile draw rect reuse
- tile batch array copy removal
- repeated Canvas2D fill-style caching

## 9. Plan Items That Should Be Dropped Or Rewritten

### Do not execute as originally written

- “Use `subarray()` instead of `slice()` in `tile-message-parser.ts`”
  - unsafe with current batch lifetime rules

- “Add immutable cache headers for `/dist/assets/`”
  - current frontend does not emit hashed `/dist/assets/...` files

- “Pool input message buffers” in a naive way
  - easy to introduce send-buffer aliasing bugs

### Rewrite before use

- audio buffer reuse items
  - align them with current `opus-playback-runtime.ts`

- MCP bridge “memory leak” wording
  - rewrite as bounded-client and timeout/backoff work

## Recommended Execution Order

### Phase 1 — Highest-value remaining wins

1. Add/standardize measurement harness for branch vs `main`
2. Remove full-frame BGRA->RGBA swap in host capture
3. Remove avoidable frame clones in gateway session-hub pump
4. Consolidate gateway cache locks
5. Enable nginx gzip

### Phase 2 — Structural hot-path work

6. Reduce gateway egress lock contention
7. Add tile-path instrumentation
8. Redesign `TileCommand` parse/queue/apply ownership model safely

### Phase 3 — Follow-up and protection

9. Bound `tileInfoByTimestamp`
10. Add MCP bridge timeout/backoff and client limits
11. Revisit audio copies
12. Revisit asset caching after asset naming is corrected

## Bottom Line

The current top performance priorities are:

- host full-frame color-swap removal
- gateway cache/relay copy and lock cleanup
- client reliable tile command-path redesign

That is the shortest path to meaningful next wins without reopening large correctness risk in areas that were already stabilized on this branch.
