# `bpane-protocol` Production Hardening Plan

Updated: April 14, 2026

## Goal

Make `code/shared/bpane-protocol` maintainable as a real production protocol crate, with:

- explicit API evolution rules
- stronger wire-compat protection
- better human developer experience
- clean validation under normal Rust library quality gates

This plan is based on a crate review against current Rust API Guidelines, the Cargo Book, and the Rustdoc Book, plus local validation of tests, rustdoc, `no_std`, and Clippy.

## Current Baseline

Passing now:

- `cargo test -p bpane-protocol`
- `cargo test -p bpane-protocol --no-default-features`
- `RUSTDOCFLAGS='-D warnings' cargo doc -p bpane-protocol --no-deps`
- `cargo clippy -p bpane-protocol --all-targets --all-features -- -D warnings`

## Execution Status

1. Done: Restore library quality gates.
   - `FileMessage` no longer fails strict Clippy due to a large inline header variant.
   - Wire format is unchanged.
2. Done: Add crate docs and compatibility policy.
   - crate-level rustdoc now documents feature flags, channel model, and wire/API compatibility expectations
   - public-facing crate metadata now exists in `Cargo.toml`
3. Done: Add exact wire fixtures and cross-language protocol tests.
   - shared fixture bytes now live in `tests/fixtures/wire-fixtures.json`
   - Rust exact-byte tests validate representative messages plus malformed cases
   - browser-client tests validate the same fixture bytes for frame parsing, file transfer, tiles, input, clipboard, and video NAL handling
4. Done: Centralize incremental framing.
   - `bpane-protocol` now provides a reusable `FrameDecoder` for partial reliable-stream parsing
   - host IPC, gateway relay, and browser reliable-stream ingress use the shared decoder instead of open-coded pending-buffer loops
   - unit coverage now includes protocol-level partial/incomplete cases plus split-read adapter tests in host and gateway
5. Done: Improve documentation and discoverability.
   - crate rustdoc now documents MSRV alongside compatibility and feature policy
   - public fallible/panicking frame APIs now state their error and panic contracts
   - workspace and crate metadata now declare an explicit `rust-version`
6. Next: Broader type-safety and API ergonomics cleanup.

## Decision Gate

Before broader API work, decide which of these is true:

1. `bpane-protocol` is workspace-internal only.
2. `bpane-protocol` is a reusable/stable crate boundary and wire contract.

This is the main sequencing dependency.

- If internal-only: optimize for maintainability and correctness first.
- If reusable/stable: treat public enums/fields as long-term API commitments and harden for semver now.

## Priority 0: Restore Library Quality Gates

### Objective

Make the crate pass the normal library validation set without caveats.

### Tasks

- Resolve `FileMessage` size imbalance.
  - Preferred options:
    - introduce indirection for the large `FileHeader` payload
    - or split header/chunk/complete into smaller dedicated types behind a dispatch enum
  - Avoid blanket lint suppression unless the memory tradeoff is explicitly documented and accepted.
- Keep `cargo clippy -D warnings` green for the crate.
- Add the crate validation commands to CI if they are not already covered.

### Exit Criteria

- `cargo clippy -p bpane-protocol --all-targets --all-features -- -D warnings` passes.

### Manual App Checkpoint

Run the app after this step and test file transfer only.

- Upload a file from the browser client to the host session.
- Trigger a browser download in Chromium and confirm it reaches the client-side download flow.
- Use at least:
  - one small text file
  - one binary file larger than a single protocol chunk

Reason:

- This step changed the in-memory shape of `FileMessage` and touched host file-transfer call sites.
- The wire format stayed stable, so other channels do not need a full browser sanity pass yet.

## Priority 1: Lock Down API Evolution Strategy

### Objective

Prevent accidental semver traps in the protocol surface.

### Risks Today

- Public enums are exhaustive.
- Public enum variants expose fully public fields.
- Adding a new variant or field will be breaking for downstream matching and construction.

### Tasks

- Decide whether protocol message enums should become `#[non_exhaustive]`.
- Decide whether data-bearing protocol types should keep public fields or move to constructors/accessors.
- Where representation is intentionally public, document that this is a compatibility commitment.
- Add a short compatibility policy to crate docs:
  - wire format compatibility expectations
  - public API compatibility expectations
  - process for introducing new protocol messages

### Exit Criteria

- There is one documented policy for evolving message enums and data types.
- New protocol additions have a non-breaking path, or the crate explicitly documents that it is not a stable public API.

### Manual App Checkpoint

- No running-app test required if this step is documentation and API policy only.
- If the step also changes public constructors or call sites, run a light browser smoke test:
  - connect
  - move mouse
  - type in the browser

## Priority 2: Add Real Wire-Compatibility Tests

### Objective

Catch protocol drift that encode/decode round-trip tests cannot detect.

### Tasks

- Add golden byte fixtures for representative messages on every typed channel.
- Add malformed fixture coverage:
  - truncated payloads
  - unknown tags
  - trailing bytes
  - oversized declared lengths
  - invalid field combinations where applicable
- Add cross-language fixtures shared with the TypeScript client.
  - Rust encodes -> TypeScript decodes
  - TypeScript encodes -> Rust decodes
- Add at least one backwards-compat fixture set for previously accepted wire payloads when the protocol changes.

### Exit Criteria

- The crate has fixture-driven tests that validate exact wire bytes, not just round-trips.
- Rust and browser-client protocol behavior is checked against the same fixtures.

### Manual App Checkpoint

- Usually no running-app test required if this step only adds fixtures and tests.
- If browser-client protocol code is touched, run:
  - connect to session
  - verify first paint
  - send input
  - copy/paste once

## Priority 3: Improve Type Safety and Call-Site Ergonomics

### Objective

Reduce invalid states and make the public API feel idiomatic to Rust users.

### Tasks

- Implement standard conversion traits where appropriate:
  - `TryFrom<u8>` for `ChannelId`
  - `From<ChannelId> for u8`
  - `TryFrom<u8>` for `MouseButton`
  - `From<MouseButton> for u8`
- Revisit raw `u8` fields in message types.
  - Prefer typed wrappers or enums when the value space is constrained.
- Improve bitflag ergonomics for `SessionFlags` and `Modifiers`.
  - Consider `bitflags` or equivalent typed helpers if dependency cost is acceptable.
  - At minimum add `contains`, named constructors, and common trait impls where useful.
- Revisit ambiguous APIs such as `FileMessage::decode`.
  - Either make the channel explicit in the public API
  - or make the method/channel contract clearer in docs.

### Exit Criteria

- Common conversions use idiomatic Rust traits.
- Public APIs expose fewer raw protocol primitives without context.

### Manual App Checkpoint

Run a browser smoke test if message constructors, decoders, or browser-client protocol code changes.

- connect
- move mouse and click
- type in a text field
- copy/paste
- upload one file

## Priority 4: Centralize Incremental Framing

### Objective

Remove duplicated frame-boundary logic from host and gateway call sites.

### Tasks

- Introduce a reusable incremental decoder in the protocol crate.
  - candidate shape: `FrameDecoder` or `FrameReader`
  - support partial buffers
  - support zero-copy where practical
  - enforce payload size limits in one place
- Migrate duplicated callers in host/gateway to the shared decoder.
- Add tests for partial delivery, concatenated frames, and oversized frame rejection through the shared decoder.

### Exit Criteria

- Host and gateway no longer open-code frame header parsing.
- Frame-boundary enforcement lives in one tested implementation.

### Manual App Checkpoint

This is the highest regression-risk step so far. Run a broader session test:

- connect a fresh browser session
- verify initial paint
- resize the browser window
- move mouse, click, and type
- copy/paste
- upload and download a file
- confirm audio path still initializes if enabled locally

Reason:

- This step affects reliable-channel framing used by host/gateway transport paths.

## Priority 5: Improve Documentation and Discoverability

### Objective

Make the crate easier to use correctly without reading implementation code.

### Tasks

- Add crate-level rustdoc to `src/lib.rs` covering:
  - channel model
  - typed vs raw channels
  - `std` / `no_std` behavior
  - compatibility policy
  - basic encode/decode examples
- Add `# Errors` and `# Panics` sections to public fallible/panicking APIs.
- Document the feature surface in rustdoc and `Cargo.toml`.
- Add package metadata:
  - `description`
  - `repository`
  - `keywords`
  - `categories`
  - `readme` if appropriate
- Add `rust-version` and adopt an explicit MSRV policy.

### Exit Criteria

- A new contributor can understand the crate’s intended usage from rustdoc alone.
- Cargo metadata is sufficient for production-grade crate discovery and maintenance.

### Manual App Checkpoint

- No running-app test required if this is doc/metadata-only.

## Priority 6: Optional Production Enhancements

These are useful, but should follow the earlier items.

### Candidates

- Optional `serde` support for protocol data structures behind a `serde` feature.
- Static trait checks for `Send` / `Sync` on key public types.
- Docs.rs metadata and feature rendering improvements.
- Release notes / protocol changelog for wire-format changes.

### Manual App Checkpoint

- Depends on the selected enhancement.
- If serialization or protocol code changes, run the same browser smoke test as Priority 3.

## Suggested Execution Order

1. Decide internal-only vs stable public crate.
2. Fix Clippy failure and keep validation green.
3. Add crate docs and compatibility policy.
4. Add wire-fixture and cross-language tests.
5. Improve type safety and API ergonomics.
6. Introduce shared incremental frame decoding.
7. Add optional production extras like `serde`.

## Success Criteria

The crate is in a good production state when all of the following are true:

- validation is green across tests, rustdoc, `no_std`, and Clippy
- API evolution rules are explicit
- exact wire compatibility is protected by fixtures
- downstream call sites do not duplicate framing logic
- public APIs are documented enough to use without source-diving
