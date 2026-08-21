# BrowserPane Remote Protocol Version 1

Status: normative BrowserPane wire contract baseline, published 2026-08-21.
Runtime negotiation and strict v1 enforcement are not implemented by this
slice; see [Compatibility and rollout](#compatibility-and-rollout).

## 1. Scope and language

This document is the normative, language-neutral contract for version 1 of the
BrowserPane browser WebTransport protocol. It defines the bytes exchanged
between a BrowserPane gateway/host (the **server**) and the BrowserPane web
client (the **client**). It is not an Internet standard and makes no claim that
arbitrary clients are compatible.

The key words **MUST**, **MUST NOT**, **REQUIRED**, **SHOULD**, **SHOULD NOT**,
and **MAY** describe the version 1 contract. Sections explicitly labelled
"current legacy behavior" describe pre-contract compatibility evidence, not
v1 behavior.

The owner-scoped HTTP API, WebTransport authentication, authorization,
session persistence, and media codec implementations are outside this wire
contract. A negotiated capability means only that both peers understand its
wire representation. It never grants authorization, overrides browser role or
project policy, proves runtime/device availability, or enables a feature by
itself.

## 2. Primitive encoding and global limits

- All integers are unsigned unless prefixed with `i`; every multi-byte integer
  uses little-endian byte order.
- A `bool` is one byte and MUST be exactly `0x00` or `0x01`.
- A `bytes32` value is `length:u32` followed by exactly `length` bytes.
- Text identified as UTF-8 MUST be well formed. Fixed text fields are NUL
  padded; content ends at the first NUL and unused bytes MUST be zero.
- Reserved flag bits and reserved fields MUST be zero when sent and MUST be
  rejected when received.
- A reliable frame payload MUST NOT exceed 16 MiB (`16,777,216` bytes). A
  decoder MUST reject the declared length before allocating or waiting for the
  payload.
- An incremental reliable decoder MUST retain no more than 16 MiB plus the
  five-byte envelope header. Exceeding that pending-byte bound is a protocol
  failure.
- Counts and lengths MUST be checked before multiplication, allocation, or
  indexing. Arithmetic overflow is a protocol failure.
- A known fixed-size message MUST have exactly its documented length. A known
  variable-size message MUST end immediately after its declared bytes. Trailing
  bytes, truncation, unknown channels, unknown message tags, invalid enum
  values, and invalid booleans are protocol failures.

## 3. Transport and envelope

After WebTransport authentication, the server opens one bidirectional reliable
stream. Both peers multiplex reliable protocol messages on that stream with
this envelope:

| Offset | Field | Type | Meaning |
| --- | --- | --- | --- |
| 0 | `channel` | `u8` | One channel ID from the registry below |
| 1 | `payload_length` | `u32` | Number of following payload bytes, at most 16 MiB |
| 5 | `payload` | bytes | Exactly `payload_length` bytes |

The envelope itself is five bytes. A stream read MAY contain part of an
envelope, part of a payload, or multiple envelopes. Receivers MUST buffer an
incomplete suffix within the global pending-byte bound and emit complete frames
in stream order.

H.264 delta fragments are the only current raw WebTransport datagram payload.
They do not carry the five-byte envelope. H.264 keyframe fragments use reliable
`CH_VIDEO` envelopes so a lost datagram does not make recovery depend on
another lost keyframe. All other messages use the reliable stream.

Reliable ordering is global to the single stream. WebTransport datagrams can
be lost, duplicated, or reordered. Neither transport path adds application
retransmission beyond the keyframe rule and the tile cache-miss rule defined
below.

## 4. Channel registry

`C→S` means client to server, `S→C` means server to client, and `Bidi`
means that the message family has valid members in both directions.

| ID | Name | Direction | Transport | Capability |
| --- | --- | --- | --- | --- |
| `0x01` | `CH_VIDEO` | S→C | raw datagram for deltas; reliable envelope for keyframes | `h264_video` |
| `0x02` | `CH_AUDIO_OUT` | S→C | reliable envelope | one selected desktop-audio codec |
| `0x03` | `CH_AUDIO_IN` | C→S | reliable envelope | `microphone_opus` |
| `0x04` | `CH_VIDEO_IN` | C→S | reliable envelope | `camera_h264_annex_b` |
| `0x05` | `CH_INPUT` | C→S | reliable envelope | core, except `KeyEventEx` |
| `0x06` | `CH_CURSOR` | S→C | reliable envelope | core |
| `0x07` | `CH_CLIPBOARD` | Bidi | reliable envelope | `clipboard_text` |
| `0x08` | `CH_FILE_UP` | C→S | reliable envelope | `file_transfer` |
| `0x09` | `CH_FILE_DOWN` | S→C | reliable envelope | `file_transfer` |
| `0x0A` | `CH_CONTROL` | Bidi | reliable envelope | core or message-specific |
| `0x0B` | `CH_TILES` | Bidi | reliable envelope | core or message-specific |

A message on the wrong channel or in the wrong direction is invalid. A server
MUST apply authorization, role, and policy checks in addition to protocol
direction. In particular, restricted viewers cannot send input, clipboard,
microphone, camera, upload, or resize messages and cannot receive clipboard or
download messages even if those wire capabilities were negotiated.

## 5. Negotiation

Version 1 reserves three control tags for negotiation. The later runtime
negotiation slices MUST use these exact assignments and layouts.

| Tag | Message | Direction | Layout and length |
| --- | --- | --- | --- |
| `0x0A` | `ClientHello` | C→S | `version_count:u8`, ascending unique `versions:u16[]`, `required_count:u8`, ascending unique `required_capabilities:u16[]`, `optional_count:u8`, ascending unique `optional_capabilities:u16[]`; 6–148 bytes total including tag |
| `0x0B` | `ServerSelection` | S→C | `selected_version:u16`, `capability_count:u8`, ascending unique `capabilities:u16[]`; 4–132 bytes total including tag |
| `0x0C` | `ProtocolReject` | S→C | `failure:u16`; exactly 3 bytes total |

`ClientHello.version_count` MUST be 1–8. Each capability count MUST be at
most 64, their sum MUST be at most 64, and the required and optional sets MUST
be disjoint. Version `0` is invalid. BrowserPane protocol v1 has numeric version
`1`.

The server selects the highest mutually supported version. For version 1 it
MUST select every known required capability and every mutually supported
optional capability. An unknown required capability rejects the handshake; an
unknown optional capability is ignored. A selection MUST be a subset of the
client offer and MUST contain every required capability.

The client sends `ClientHello` as its first protocol frame. Before a successful
`ServerSelection`, neither peer may send any other protocol message and the
gateway MUST NOT join the session hub, start a runtime, or cause another
session side effect. A duplicate hello or selection is invalid. Negotiation
has a configurable timeout from 100 through 10,000 milliseconds and defaults
to 3,000 milliseconds.

The minimum v1 core is negotiation and framing; resolution/ready/ping/pong;
base input tags `0x01`–`0x04`; cursor messages; and tile `GridConfig`, `Fill`,
`Qoi`, and `BatchEnd`. Everything else requires its capability below.

### 5.1 Capability registry

| ID | Name | Enables | Dependency |
| --- | --- | --- | --- |
| `0x0001` | `tile_zstd` | tile `Zstd` | core tiles |
| `0x0002` | `tile_cache` | tile `CacheHit` and `CacheMiss` | core tiles |
| `0x0003` | `tile_scroll` | `ScrollCopy`, `GridOffset`, `ScrollStats`, `TileDrawMode` | core tiles |
| `0x0004` | `h264_video` | `CH_VIDEO` H.264 | none |
| `0x0005` | `roi_video` | `VideoRegion` and video tile metadata | `h264_video` |
| `0x0006` | `audio_pcm_s16le` | desktop PCM codec | none |
| `0x0007` | `audio_adpcm_ima_stereo` | desktop IMA ADPCM codec | none |
| `0x0008` | `audio_opus` | desktop Opus codec | none |
| `0x0009` | `microphone_opus` | `CH_AUDIO_IN` Opus | none |
| `0x000A` | `camera_h264_annex_b` | `CH_VIDEO_IN` H.264 | none |
| `0x000B` | `clipboard_text` | clipboard text | none |
| `0x000C` | `file_transfer` | upload and download families | none |
| `0x000D` | `extended_keyboard` | `KeyEventEx` and `KeyboardLayoutInfo` | base input |
| `0x000E` | `client_access_state` | `ClientAccessState` | none |

At most one of the three desktop-audio codec capabilities may appear in
`ServerSelection`. A dependent capability without its dependency makes the
selection invalid.

## 6. Control messages

The first byte is the tag.

| Tag | Message | Direction | Fields after tag | Total bytes |
| --- | --- | --- | --- | --- |
| `0x01` | `ResolutionRequest` | C→S | `width:u16`, `height:u16` | 5 |
| `0x02` | `ResolutionAck` | S→C | `width:u16`, `height:u16` | 5 |
| `0x03` | `SessionReady` | S→C | `version:u8`, `session_flags:u8` | 3 |
| `0x04` | `Ping` | Bidi | `sequence:u32`, `timestamp_ms:u64` | 13 |
| `0x05` | `Pong` | Bidi | `sequence:u32`, `timestamp_ms:u64` | 13 |
| `0x06` | `KeyboardLayoutInfo` | C→S | `layout_hash:[u8;32]` | 33 |
| `0x07` | `BitrateHint` | S→C | `bits_per_second:u32` | 5 |
| `0x08` | `ResolutionLocked` | S→C | `width:u16`, `height:u16` | 5 |
| `0x09` | `ClientAccessState` | S→C | `access_flags:u8`, `width:u16`, `height:u16` | 6 |

The negotiation tags `0x0A`–`0x0C` are defined in section 5.

In a negotiated v1 connection, `SessionReady.version` MUST be `1` and MUST
match `ServerSelection.selected_version`. `SessionReady` follows successful
selection and describes runtime availability, not protocol understanding.
An exact selected-version replay MAY update effective runtime or policy flags
after owner promotion, late join, or policy filtering, but it MUST NOT change
the selected version or enable an unselected capability.
`ResolutionAck` confirms an applied request. `ResolutionLocked` is retained as
a legacy compatibility message; v1 servers SHOULD use `ClientAccessState` when
that capability was selected. A `Pong` copies the corresponding ping sequence
and timestamp. Unsolicited or mismatched pongs are ignored, not replayed.

`session_flags` assignments are `0x01 audio`, `0x02 clipboard`, `0x04 file
transfer`, `0x08 microphone`, `0x10 camera`, and `0x20 keyboard layout`.
`access_flags` assignments are `0x01 view only` and `0x02 resize locked`.
Unassigned bits are reserved.

## 7. Input and cursor

All input messages are C→S. Coordinates use the current remote pixel space.
Modifiers are a bit set: `0x01 Ctrl`, `0x02 Alt`, `0x04 Shift`, `0x08 Meta`,
and `0x10 AltGr`; remaining bits are reserved.

| Tag | Input message | Fields after tag | Total bytes |
| --- | --- | --- | --- |
| `0x01` | `MouseMove` | `x:u16`, `y:u16` | 5 |
| `0x02` | `MouseButton` | `button:u8`, `pressed:bool`, `x:u16`, `y:u16` | 7 |
| `0x03` | `MouseScroll` | `dx:i16`, `dy:i16` | 5 |
| `0x04` | `KeyEvent` | `keycode:u32`, `pressed:bool`, `modifiers:u8` | 7 |
| `0x05` | `KeyEventEx` | `keycode:u32`, `pressed:bool`, `modifiers:u8`, `key_char:u32` | 11 |

Mouse button values are `0 left`, `1 middle`, `2 right`, `3 back`, and
`4 forward`. `KeyEventEx` requires `extended_keyboard`; `key_char` is a Unicode
scalar value or zero when no character applies.

Cursor messages are S→C:

| Tag | Cursor message | Fields after tag | Total bytes |
| --- | --- | --- | --- |
| `0x01` | `CursorMove` | `x:u16`, `y:u16` | 5 |
| `0x02` | `CursorShape` | `width:u16`, `height:u16`, `hotspot_x:u8`, `hotspot_y:u8`, `rgba_length:u32`, RGBA bytes | 11 + data |

Cursor shape data MUST contain exactly `width * height * 4` bytes. Hotspots
MUST be inside a non-empty shape. The byte order per pixel is R, G, B, A.

## 8. Clipboard and file transfer

Clipboard payloads use tag `0x01` (`Text`), then `utf8_length:u32`, then that
many UTF-8 bytes. The family is bidirectional and requires `clipboard_text`.
BrowserPane limits clipboard text to 1 MiB and rejects empty, oversized, or
invalid UTF-8 content.

File messages require `file_transfer`. The same payload layouts are used on
`CH_FILE_UP` (C→S) and `CH_FILE_DOWN` (S→C).

| Tag | File message | Fields after tag | Total bytes |
| --- | --- | --- | --- |
| `0x01` | `FileHeader` | `id:u32`, `filename:[u8;256]`, `size:u64`, `mime:[u8;64]` | 333 |
| `0x02` | `FileChunk` | `id:u32`, `sequence:u32`, `data_length:u32`, data | 13 + data |
| `0x03` | `FileComplete` | `id:u32` | 5 |

For each transfer ID, `FileHeader` MUST come first, chunks MUST start at
sequence zero and increase by one without gaps, the sum of chunk data MUST NOT
exceed the advertised size, and `FileComplete` MUST arrive once after exactly
the advertised bytes. IDs cannot be reused while active. A duplicate header,
chunk, or completion is invalid. Current senders use chunks of at most 64 KiB;
v1 receivers MUST accept smaller chunks and MUST reject chunks larger than
64 KiB. Filenames are relative names, not paths: separators, `.`/`..`, control
characters, empty names, and embedded NULs are invalid. MIME is optional UTF-8
metadata and cannot be interpreted as authority to execute content.

## 9. Tile messages

All tile messages are S→C except `CacheMiss`, which is C→S. A connection
starts a grid with `GridConfig`; a resize replaces it. Coordinates MUST be
inside the active grid and rectangles MUST remain within the screen.

| Tag | Message | Fields after tag | Bytes | Capability |
| --- | --- | --- | --- | --- |
| `0x01` | `GridConfig` | `tile_size:u16`, `columns:u16`, `rows:u16`, `screen_width:u16`, `screen_height:u16` | 11 | core |
| `0x02` | `CacheHit` | `column:u16`, `row:u16`, `hash:u64` | 13 | `tile_cache` |
| `0x03` | `Fill` | `column:u16`, `row:u16`, `rgba:u32` | 9 | core |
| `0x04` | `Qoi` | `column:u16`, `row:u16`, `hash:u64`, `data_length:u32`, QOI bytes | 17 + data | core |
| `0x05` | `VideoRegion` | `x:u16`, `y:u16`, `width:u16`, `height:u16` | 9 | `roi_video` |
| `0x06` | `BatchEnd` | `frame_sequence:u32` | 5 | core |
| `0x07` | `ScrollCopy` | `dx:i16`, `dy:i16`, `region_top:u16`, `region_bottom:u16`, `region_right:u16` | 11 | `tile_scroll` |
| `0x08` | `GridOffset` | `offset_x:i16`, `offset_y:i16` | 5 | `tile_scroll` |
| `0x09` | `CacheMiss` | `frame_sequence:u32`, `column:u16`, `row:u16`, `hash:u64` | 17 | `tile_cache` |
| `0x0A` | `ScrollStats` | 23 cumulative `u32` counters | 93 | `tile_scroll` |
| `0x0B` | `TileDrawMode` | `apply_offset:bool` | 2 | `tile_scroll` |
| `0x0C` | `Zstd` | `column:u16`, `row:u16`, `hash:u64`, `data_length:u32`, zstd RGBA bytes | 17 + data | `tile_zstd` |

`tile_size`, screen dimensions, rows, and columns MUST be nonzero. Current
senders use a 32–256, 16-pixel-aligned tile size. `Fill.rgba` is
packed so the least-significant byte is red, followed by green, blue, and
alpha. Decoded QOI or zstd data MUST describe exactly the addressed tile
dimensions in RGBA order. Hashes identify decoded tile pixels, not compressed
bytes.

Tile updates between successive `BatchEnd` messages form one ordered batch.
`TileDrawMode` resets to true at the beginning of a batch. A `CacheHit` is
applied only when the receiver has the announced hash; otherwise the client
sends one `CacheMiss`. The server repairs a miss with tile bytes. Repeated
cache-miss reports do not authorize unbounded retransmission or cache growth.

`ScrollCopy` copies existing pixels inside `[region_top, region_bottom)` and
`[0, region_right)` by `(dx,dy)`. `GridOffset` applies to following content
tiles. `ScrollStats` fields, in wire order, are:

1. `scroll_batches_total`
2. `scroll_full_fallbacks_total`
3. `scroll_potential_tiles_total`
4. `scroll_saved_tiles_total`
5. `scroll_non_quantized_fallbacks_total`
6. `scroll_residual_full_repaints_total`
7. `scroll_residual_interior_limit_fallbacks_total`
8. `scroll_residual_low_saved_ratio_fallbacks_total`
9. `scroll_residual_large_row_shift_fallbacks_total`
10. `scroll_residual_other_fallbacks_total`
11. `scroll_zero_saved_batches_total`
12. `scroll_split_region_batches_total`
13. `scroll_sticky_band_batches_total`
14. `scroll_chrome_tiles_total`
15. `scroll_exposed_strip_tiles_total`
16. `scroll_interior_residual_tiles_total`
17. `scroll_edge_strip_residual_tiles_total`
18. `scroll_small_edge_strip_residual_tiles_total`
19. `scroll_small_edge_strip_residual_rows_total`
20. `scroll_small_edge_strip_residual_area_px_total`
21. `host_sent_hash_entries`
22. `host_sent_hash_evictions_total`
23. `host_cache_miss_reports_total`

These counters are session telemetry, may wrap as `u32`, and MUST NOT be used
as an authorization or integrity signal.

## 10. Audio

An audio channel payload has no message tag:

`sequence:u32 | timestamp_us:u64 | data_length:u32 | data`

The fixed prefix is 16 bytes. Sequence numbers increase modulo `2^32` within a
direction. A receiver drops duplicate or stale audio frames; it does not replay
them. Gaps represent loss and do not block later audio.

Codec-tagged `data` starts with ASCII magic `WRA1` (`57 52 41 31`), then one
codec byte, then codec payload:

| Codec | Byte | Direction and current profile | Capability |
| --- | --- | --- | --- |
| PCM signed 16-bit little endian | `0x00` | S→C, 48 kHz stereo | `audio_pcm_s16le` |
| IMA ADPCM stereo | `0x01` | S→C, 48 kHz stereo | `audio_adpcm_ima_stereo` |
| Opus | `0x02` | S→C 48 kHz stereo, or C→S 48 kHz mono | `audio_opus` or `microphone_opus` by direction |

Current frames represent 20 milliseconds. A v1 `CH_AUDIO_OUT` payload MUST
use the one selected desktop codec. A v1 `CH_AUDIO_IN` payload MUST be tagged
Opus and requires `microphone_opus`. Untagged legacy PCM is not valid v1.

## 11. H.264 video

### 11.1 Server-to-client video

`CH_VIDEO` carries one fragment payload with this layout:

`nal_id:u32 | fragment_sequence:u16 | fragment_total:u16 | keyframe:bool |
pts_us:u64 | data_length:u32 | data | flags:u8 | optional tile fields`

The fixed size through flags is 22 bytes. Flag `0x01` means six following
`u16` values are present: `tile_x`, `tile_y`, `tile_width`, `tile_height`,
`screen_width`, `screen_height`. The total prefix is then 34 bytes. Other flag
bits are reserved. Tile fields require `roi_video` and describe the composition
rectangle for every fragment of that NAL.

BrowserPane senders cap the complete raw datagram payload at 1,100 bytes, so
fragment data is at most 1,078 bytes without tile metadata or 1,066 bytes with
it. `fragment_total` MUST be 1–65,535 and `fragment_sequence` MUST be less than
it. All fragments for a NAL ID MUST agree on total, keyframe, timestamp, and
tile fields. Empty NAL data is represented by one fragment numbered zero.

Delta fragments use raw datagrams and may arrive in any order. A receiver
reassembles a NAL only after exactly one copy of every sequence is present,
drops inconsistent or duplicate fragments, bounds incomplete NAL state to 32
entries, and may evict the oldest incomplete entry. Keyframe fragments use
reliable `CH_VIDEO` envelopes. Because the same NAL can be observed through
both paths during transition or replay protection, receivers retain the most
recent 128 completed NAL IDs and do not decode one twice.

The reassembled bytes are H.264 Annex B. `VideoRegion` and tile metadata must
agree for ROI composition. A delta whose reference state is unavailable is
dropped until a valid keyframe restores decoder state.

### 11.2 Client-to-server camera

`CH_VIDEO_IN` is a reliable-envelope payload containing one H.264 Annex B
access unit produced by browser WebCodecs. An empty payload stops camera
ingress. The channel requires `camera_h264_annex_b`; there is no MJPEG fallback.

## 12. Failure and close rules

The typed failure registry is frozen for v1:

| ID | Name | Meaning |
| --- | --- | --- |
| `0x0001` | `unsupported_protocol_version` | no offered version can be selected |
| `0x0002` | `required_protocol_capability_missing` | a required capability is unknown, unsupported, or dependency-invalid |
| `0x0003` | `malformed_protocol_hello` | hello layout, count, sorting, uniqueness, or value is invalid |
| `0x0004` | `protocol_downgrade_refused` | policy forbids the only available legacy/downgrade path |
| `0x0005` | `protocol_handshake_timeout` | no valid hello/selection arrived before the deadline |
| `0x0006` | `protocol_selection_mismatch` | selection is not a valid subset or omits a required capability |
| `0x0007` | `unexpected_protocol_frame` | state, direction, channel, tag, enum, flag, length, order, or duplicate rule failed |
| `0x0008` | `protocol_frame_too_large` | a declared reliable payload exceeds 16 MiB |
| `0x0009` | `protocol_pending_buffer_limit` | incomplete reliable bytes exceed 16 MiB plus five bytes |

When safe before transport close, the rejecting peer sends `ProtocolReject`
with only the numeric failure. It then closes the WebTransport session with
application close code `0x4250 + failure ID` (`0x4251`–`0x4259`). Normal
session completion uses code `0`. Unknown failure IDs are not sent. A peer MUST
NOT place raw errors, URLs, credentials, paths, browser content, media/file
bytes, identity values, or resource IDs in protocol failures, close reasons,
logs, traces, or metric labels.

After successful negotiation, any global parse violation is fatal. A receiver
MAY locally drop a semantically stale media fragment or cache report where this
document explicitly permits it; that is not permission to accept malformed
bytes. No protocol error is echoed back as unbounded data.

## 13. Compatibility and rollout

Issue #263 publishes the contract and vector baseline only. It does **not**
claim that current gateway or browser binaries perform the v1 handshake. The
codec, state-machine, browser, fuzz, and real-stack rollout work is sequenced in
issues #264–#268 under program #175.

| Gateway | Client | Required result |
| --- | --- | --- |
| current pre-contract | current pre-contract | existing legacy profile; no negotiated-version claim |
| v1-capable | v1-capable | negotiate numeric version 1 and an explicit capability set |
| v1-capable | current pre-contract | legacy profile only when the gateway's explicit legacy policy permits it; otherwise typed downgrade rejection |
| current pre-contract | v1-capable | client legacy fallback only when explicitly enabled; otherwise timeout/rejection |
| v1-capable | unsupported or malformed peer | typed rejection and bounded close, with no runtime/session side effect |

When the negotiation work is implemented, gateway and client legacy fallback
default to enabled for the initial v1 overlap, and deployment/runtime
configuration must make that choice explicit. The client fallback recognizes
only the checked current legacy `SessionReady` shape, not arbitrary bytes.

Current legacy behavior has two incompatible-looking `SessionReady.version`
bytes: the host emits `2`, while the original shared fixture contains `1`; the
browser currently ignores the field. They are pre-contract implementation
markers, not public protocol v2 or proof of v1 negotiation. A later negotiated
v1 path must send `1` consistently. Current legacy decoders also differ in
strictness for non-canonical booleans, reserved/trailing bytes, and pending
buffer enforcement. The current video decoder also accepts the older 21-byte
prefix without a flags byte, and current synthetic cursor seeds validate wire
framing without enforcing full RGBA dimensions. Those differences remain
legacy-only and must not be described as v1 conformance.

Legacy fallback MUST be explicit, observable through bounded aggregate
diagnostics, and disableable. A peer MUST NOT silently reinterpret a malformed
v1 hello as legacy. Downgrade, selection, and rejection telemetry uses fixed
failure names only and contains no resource IDs or payload-derived labels.

## 14. Shared vector catalog

The checked-in catalog at
`code/shared/bpane-protocol/tests/fixtures/wire-fixtures.json` has schema version
`1` and catalog name `browserpane-current-seed`. It contains exactly the 15
original synthetic byte sequences. Each vector records a unique name,
direction, transport, channel, sorted capability list, exact lowercase wire
hex, and one expected outcome:

- `valid` plus a stable message classification; or
- `invalid` plus a stable error classification.

Rust and TypeScript tests enumerate the entire catalog, reject duplicate or
malformed metadata, route every vector through production parsing boundaries,
and compare the same outcome classes. Adding an entry without teaching both
consumers to classify it fails both suites. These 15 seeds are a regression
baseline, not complete protocol coverage or interoperability proof.

## 15. Change and deprecation policy

- Version 1 numeric versions, channel IDs, tags, capability IDs, failure IDs,
  field order, byte order, and frozen vector bytes MUST NOT be reassigned.
- A compatible addition requires a new capability or a previously reserved
  value, bounded parsing rules, fixtures in both languages, and documentation
  in this file before runtime use.
- A change to existing field meaning, required ordering, validation, or bytes
  requires a new negotiated protocol version and an explicit compatibility and
  downgrade review.
- Deprecated capabilities remain reserved and cannot be reused. Removal needs
  measured legacy usage, operator notice, a rollback plan, and overlap for at
  least one release before default disablement.
- Implementations MUST preserve unknown optional capability tolerance but MUST
  reject unknown required capabilities, channels, message tags, and flags.
- A fixture schema revision is separate from a protocol version. Consumers
  MUST reject unsupported fixture schema versions rather than ignore fields.

The source contract for later implementation is this document plus the shared
catalog. Code and executable tests remain the evidence for current legacy
behavior; differences must be recorded in the compatibility section rather
than silently changing this v1 contract.
