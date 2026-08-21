# BPANE-00175 BrowserPane Remote Protocol V1 Plan

## Metadata

- Issue: `#175`
- State: Qualified
- Owner: BrowserPane maintainers
- Lane: Production
- Target gate: Production Baseline protocol compatibility checkpoint
- Depends on: accepted ADR 0003 and the existing `#151` validation floor
- Coordinates with: `#20`, `#72`, `#75`, `#168`, and `#169`
- Last verified commit/date: `e21e2206a93a` / 2026-08-21

## Business Outcome

An operator can upgrade a BrowserPane gateway and browser client independently
within one documented compatibility window. Compatible peers deterministically
select the same protocol and capability set; incompatible or malformed peers
receive a bounded typed failure before they can operate a browser session.

The result is the first published BrowserPane remote-protocol contract and its
conformance evidence. It makes the current product-specific transport durable
without describing it as an industry standard or claiming compatibility with
arbitrary third-party clients.

## Example Use Case

A gateway is upgraded before an embedded browser client. The client and
gateway authenticate, negotiate the highest common BrowserPane protocol and
capabilities on the gateway-owned reliable stream, and continue through the
documented legacy overlap when the old client has not learned negotiation yet.
An unsupported future client, an expired old client, or a client that sends a
malformed offer is rejected before session-hub membership, ownership, resize,
file, input, or media activity. Rust and TypeScript consume the same checked-in
wire vectors for both outcomes.

## Current Evidence

- `code/shared/bpane-protocol` implements a five-byte little-endian envelope,
  a 16 MiB payload ceiling, eleven channels, typed control/input/cursor/
  clipboard/file/tile messages, raw media payloads, incremental bounded frame
  decoding, property round trips, and mixed-channel integration tests.
- The gateway decodes browser streams with `FrameDecoder`, filters direction
  and session policy, caches `SessionReady`, grid state, and a keyframe for
  late joiners, and rewrites effective session flags for viewers and project
  policy. It does not negotiate a protocol before joining the session hub.
- The host currently emits `SessionReady.version = 2`. The TypeScript
  `SessionControlRuntime` reads only the flags byte and never validates the
  version. The shared JSON `control_session_ready` fixture carries version
  `1`. These values prove that the byte exists, but not that either value has a
  published compatibility meaning.
- `code/shared/bpane-protocol/tests/fixtures/wire-fixtures.json` already has 15
  language-neutral seed entries across control, input, cursor, clipboard,
  files, tiles, audio, video, and three invalid cases. Rust validates all of
  the relevant entries; the TypeScript protocol suite currently reads only the
  control and oversized-frame entries. Golden vectors are therefore partial,
  not wholly absent.
- Rust property tests cover envelope and typed-message round trips and video
  fragmentation. The TypeScript parser covers chunking and oversized lengths.
  No parser fuzz target, negotiated handshake, published capability registry,
  or gateway/client support matrix exists.
- WebTransport authentication already uses an owner bearer or purpose-scoped
  connect ticket. Client errors already have stable `BpaneError` subclasses,
  and gateway logging already sanitizes request targets. The v1 work extends
  those boundaries rather than creating a second authentication scheme.
- `openapi/bpane-control-v1.yaml` does not cover WebTransport. The control API
  compatibility policy explicitly requires WebTransport to have its own
  contract before compatibility claims apply.

## Scope

1. Publish `docs/REMOTE_PROTOCOL_V1.md` as the normative language-neutral wire
   specification for the browser-facing BrowserPane protocol. It must define
   the envelope, endianness, limits, all channel IDs, directions, reliable
   stream versus datagram use, every message tag and field, media framing,
   ordering, fragmentation, duplicate/replay handling, validation, and close
   behavior implemented by the supported host, gateway, and browser client.
2. Define a gateway-owned, authenticated negotiation phase on the existing
   reliable bidirectional stream. A negotiating client sends `ClientHello` as
   its first frame; the gateway selects the highest common version and the
   bounded capability intersection, then sends a typed selection before the
   effective `SessionReady`. Negotiation messages are consumed by the gateway
   and are never forwarded to the host.
3. Publish negotiated protocol version `1` as the first supported public
   version. The current unvalidated host value `2` is a legacy implementation
   marker, not a retroactively promised public protocol v2. A v1 gateway
   normalizes the browser-facing `SessionReady.version` to the selected v1
   value while retaining the existing host/gateway compatibility path.
4. Freeze a capability registry. It must distinguish protocol understanding
   from runtime availability and authorization, cover every optional current
   path (tile extensions and compression, ROI video, audio codecs, microphone,
   camera, clipboard, file transfer, extended keyboard events, access-state,
   and scroll messages), and name the minimal v1 core. Unknown optional
   capability IDs are ignored and never selected; unknown required IDs reject
   the connection.
5. Add the initial rolling-upgrade overlap for the current unnegotiated client
   profile. A gateway may enter that profile only when legacy compatibility is
   enabled and no syntactically valid `ClientHello` has been received. A
   malformed or downgrade-attempting hello must never fall back to legacy.
6. Enforce the selected version and capability upper bound in the gateway and
   TypeScript client before normal session operation. The selected version is
   immutable for the connection. An exact `SessionReady` replay may update
   effective runtime/policy flags for owner promotion or policy filtering, but
   it may not change the selected version or enable an unnegotiated protocol
   capability.
7. Expand the existing JSON seed catalog into a schema-versioned shared vector
   corpus consumed completely by Rust and TypeScript. Cover every v1 message
   family, handshake outcome, boundary value, invalid encoding, and expected
   error classification without maintaining separate language-specific truth.
8. Add deterministic malformed-input tests, property/mutation coverage, Rust
   fuzz targets, a real gateway/client compatibility smoke, safe live
   diagnostics, and the change/deprecation/release-note policy required to
   review later protocol revisions.

## Non-Goals

- Replacing WebTransport, H.264, Opus, QOI, zstd, WebCodecs, or WebGL.
- Claiming that the BrowserPane protocol is an industry standard or that
  arbitrary third-party clients are supported.
- Rewriting the tile/video render hot path or optimizing codecs, capture,
  gateway fan-out, copying, locking, or backpressure without profiles.
- Changing owner identity, connect-ticket purpose, viewer authorization,
  project policy, file authority, or other control-plane grants.
- Adding a general session-inspector history store, protocol catalog, or
  standalone Admin-New management route.
- Defining SBOM, signing, provenance, release-channel, package-version, or
  broad component-support policy.
- Treating successful same-version local connection as Production, scale, or
  third-party interoperability evidence.
- Preserving every pre-contract experimental marker indefinitely. Only the
  explicitly tested current legacy profile receives the initial overlap.

## Decisions And Dependencies

### V1 negotiation and downgrade contract

Authentication and owner/session visibility checks run before protocol details
are disclosed. After WebTransport acceptance, the gateway opens the existing
reliable bidirectional stream but does not resolve/start a runtime, join the
session hub, consume a viewer slot, claim ownership, or forward frames until
negotiation succeeds or the explicit legacy path is selected.

`ClientHello` carries a bounded canonical list of supported protocol versions,
required capability IDs, and optional capability IDs. Lists are length-limited,
strictly ordered, and duplicate-free. The gateway computes the highest common
published version; a client cannot select a lower value when a higher common
value exists. Version `1` is the only negotiated version in the first matrix,
so its negotiated minimum and maximum are both `1`.

The gateway replies with a typed selected version and capability set, followed
by a `SessionReady` whose version must match. The client does not fire its
protocol-ready/public connect signal, send normal input, or accept media before
both messages validate. Data before `ClientHello`, a duplicate hello, changed
selection, a mismatched `SessionReady`, or capability use outside the selected
set is a protocol violation.

The normative specification owns the exact unused control tags and binary
field encodings. They must be additive to the current tag space, be represented
in the shared vectors, and remain strictly bounded. Numeric assignment is an
implementation detail only after these semantics are satisfied; query-string
version negotiation and new credentials are outside the contract.

### Initial compatibility window

The first published support matrix has these rows:

| Gateway | Browser client | Result |
| --- | --- | --- |
| v1 negotiating | v1 negotiating | Select v1 and the capability intersection. |
| v1 negotiating | current pre-contract client fixture | Use the explicit legacy profile only while gateway legacy compatibility is enabled. |
| current pre-contract gateway fixture | v1 client | Use the client's explicit legacy fallback only when it receives the recognized current legacy `SessionReady` shape and client legacy compatibility is enabled. |
| v1 or legacy | unsupported old/future or malformed peer | Typed rejection/typed client error; no normal session activity. |

Gateway and client legacy compatibility are enabled for the initial v1 rollout
so the gateway-first upgrade in the issue use case works. The setting, selected
mode, and warning must be visible to operators. Disabling it is reversible.
Removal or a default change requires a later reviewed release decision under
`#75`, an updated matrix and release note, at least one released overlap with
passing previous-client evidence, and a new qualification pass. This plan does
not invent a calendar end-of-support date.

### Capability and authorization separation

Negotiated capabilities mean only that both peers understand a wire behavior.
The effective usable set is the intersection of negotiated support, runtime
availability, session/project policy, browser feature detection, and client
role. `SessionFlags` continue to report effective runtime/session features and
`ClientAccessFlags` continue to express view-only/resize authority. Neither
message may grant a capability or permission absent from the other controlling
layers.

### Ownership boundaries

- `#175` owns the browser-facing wire specification, negotiation, current
  legacy overlap, protocol-specific compatibility matrix, parser/conformance
  evidence, fuzzing, typed protocol failures, and safe live connection
  diagnostics.
- `#20` retains durable session logs, historical inspection, page/tab models,
  and general session-inspector APIs. If negotiated metadata later needs
  persistence or historical query, that is a separate `#20` slice; `#175`
  exposes only bounded live diagnostics.
- `#72` retains the cross-product threat model, ingress/TLS deployment
  hardening, generalized rate limits, incident response, and named-profile
  security acceptance. `#175` may mitigate T-18 but cannot close `#72` or make
  a Production security claim.
- `#75` retains BrowserPane release channels, component version bundles, SBOM,
  signing, provenance, release promotion, published dates, and sustained
  support policy. `#175` supplies the protocol matrix, deprecation input, and
  conformance evidence that `#75` later consumes.
- `#168` retains host/client capture, classification, decode, and render
  optimization. V1 freezes behavior and limits; it does not optimize them.
- `#169` retains gateway fan-out, send locking, copying, backpressure, and
  multi-viewer capacity optimization. V1 must preserve late-join and viewer
  semantics but does not claim a capacity envelope.

## Contract Changes

- API/OpenAPI: N/A. WebTransport remains outside
  `openapi/bpane-control-v1.yaml`; no owner API operation or schema changes.
- Protocol/event schemas: add the normative v1 document, negotiation messages,
  capability registry, typed rejection vocabulary, shared-vector schema, and
  protocol change/deprecation rules. Standard codec bitstreams remain governed
  by their upstream specifications; BrowserPane specifies only their framing
  and use.
- Database/migrations: N/A. Negotiation and diagnostics are connection-local;
  no session-control row or migration is required.
- Admin-New: show safe live version, selected mode, capability names, and typed
  incompatibility feedback in the existing session preview/observability
  experience. No new catalog or management route.
- CLI/SDK: the browser client SDK exposes a read-only protocol diagnostic
  snapshot and stable typed connect failures. The product CLI gains no new
  resource or command because it does not own the WebTransport session codec;
  test tooling may print the same sanitized snapshot.
- Deployment/configuration: add one bounded gateway legacy-compatibility
  setting and its client-SDK counterpart, plus a bounded handshake timeout.
  The initial v1 rollout defaults legacy compatibility on; production-like and
  local Compose manifests must spell out the selected value. No new port,
  credential, secret, database, or runtime backend.
- README/ARCH/AGENTS/operator docs: update README protocol support wording,
  the ARCH wire table/handshake, operator configuration and diagnostics, the
  validation matrix, capability maturity, R-007 evidence, and release notes.
  `AGENTS.md` changes only if code-aligned protocol commands or architecture
  ownership need a durable contributor update.

## Security And Data Impact

- Authenticate before returning supported-version details, and preserve the
  current purpose-scoped connect ticket and owner visibility boundary.
- Bound hello bytes, version and capability counts, pending stream bytes,
  frame payloads, fragment counts, reassembly state, handshake time, and per-
  connection parser work. A bad client is closed without terminating other
  viewers or the underlying runtime.
- Treat a syntactically valid but incompatible or malicious hello as a hard
  rejection. Never turn malformed input, duplicate fields, a downgrade, or a
  missing required capability into legacy fallback.
- Enforce channel direction, negotiation state, message lengths, enum values,
  trailing bytes, file/video fragment constraints, and selected capabilities
  before dispatch. Reliable-stream chunking is allowed; data reordering is not.
- Permit an exact selected-version `SessionReady` replay for late join and
  owner/policy changes. Reject version changes; effective flags may only move
  within the fixed negotiated upper bound and current authorization policy.
- Protocol diagnostics may contain bounded version numbers, mode, capability
  identifiers, and fixed rejection codes. They must not contain connect
  tickets, bearer values, request URLs, session/resource IDs as metric labels,
  browser content, clipboard/file payloads, media bytes, identity claims, raw
  errors, or local paths.
- Metrics use fixed outcome/reason enums only, without version, capability,
  session, URL, or peer-controlled labels. Logs use fixed event names and
  bounded sanitized fields. Full durable session logging remains `#20`.
- Fuzz corpora and failure artifacts must be synthetic. A crashing or hanging
  input becomes a minimal checked-in regression vector without customer data.

## Migration, Compatibility, And Rollback

The rollout order is gateway first, browser client second, then optional legacy
disablement in a separately reviewed release:

1. Ship the normative spec, vector schema, gateway negotiation state machine,
   explicit legacy mode, typed failures, and diagnostics while the existing
   client fixture still passes.
2. Ship the v1 client hello, selection/`SessionReady` enforcement, typed SDK
   errors, and recognized old-gateway fallback.
3. Record the real current/current, gateway-first, client-first/rollback, old,
   and future-version results in the compatibility matrix.
4. Leave legacy compatibility enabled until `#75` owns a published removal or
   default-change decision with the required overlap evidence.

There is no database migration. During the initial overlap, rollback is a
normal gateway or client revert because the opposite side still has the tested
legacy profile. After an operator disables legacy mode, re-enable it and prove
the legacy smoke before reverting either peer. Once a future release removes
legacy code, rollback is a coordinated gateway/client release action; silent
downgrade is never a recovery mechanism.

A partially deployed negotiation implementation must fail closed before normal
session operation, not reinterpret v1 frames as legacy. Existing sessions may
finish on their already selected connection; reconnect negotiates afresh.

## Observability, Failure Behavior, And Operator Feedback

The gateway and SDK use one bounded failure vocabulary at minimum:

- `unsupported_protocol_version`
- `required_protocol_capability_missing`
- `malformed_protocol_hello`
- `protocol_downgrade_refused`
- `protocol_handshake_timeout`
- `protocol_selection_mismatch`
- `unexpected_protocol_frame`
- `protocol_frame_too_large`
- `protocol_pending_buffer_limit`

The gateway sends a bounded rejection frame when it can do so safely, then
closes with a stable application close code. The SDK maps the outcome to a
stable `BpaneError` code, does not fire `onConnect`, tears down partial client
state, and exposes a user-action message such as update client, update gateway,
or enable the temporary legacy profile. Malformed bytes never appear in the
message.

The live diagnostic snapshot reports local supported range, peer offered
range, selected version, negotiated versus legacy mode, selected capability
names, effective session flags, handshake duration, and the last fixed failure
code. Admin-New consumes that snapshot in the existing session live/
observability UI. Gateway counters cover negotiation attempts, successes,
legacy selections, and fixed failure reasons; they carry no resource or
peer-controlled labels.

## Implementation Slices

1. Publish the normative v1 document, vector schema, capability registry,
   error vocabulary, and support matrix; expand the current 15-entry seed set
   without changing runtime behavior.
2. Add Rust and TypeScript negotiation types/codecs plus complete shared-vector,
   boundary, property, and mutation tests.
3. Add the gateway pre-hub handshake state machine, highest-common selection,
   legacy profile, host `SessionReady` normalization, typed close behavior,
   bounded diagnostics, and policy/late-join regressions.
4. Add client hello/selection enforcement, delayed ready signal, capability
   gating, recognized legacy fallback, typed errors, and SDK diagnostics.
5. Add malformed-state integration tests and Rust fuzz targets for envelope,
   typed dispatch, negotiation, and video/file fragmentation/reassembly.
6. Add the compatibility smoke, Admin-New live feedback, deployment/operator
   documentation, validation-stage wiring, release note, maturity/risk update,
   and final rollback evidence.

Each slice must keep current same-version behavior green. No slice may claim
v1 support until gateway and client enforcement plus shared conformance and the
compatibility smoke are present together.

## Acceptance Criteria

- `docs/REMOTE_PROTOCOL_V1.md` specifies every current browser-facing envelope,
  channel, message, media framing, limit, direction, ordering, fragmentation,
  replay/duplicate, negotiation, capability, error, and close rule, and matches
  the reviewed Rust/TypeScript implementations.
- Protocol v1 is selected only as the highest common supported version;
  unsupported old/future, downgrade, missing-required-capability, malformed,
  duplicate, and timed-out peers fail with the documented typed outcome before
  session-hub membership or normal session activity.
- The browser validates the selected reply and `SessionReady.version`, delays
  its ready signal until validation succeeds, and never enables a wire
  capability or user permission outside the negotiated/runtime/policy/role
  intersection.
- The current pre-contract client and gateway fixtures pass only through the
  documented legacy profile. Its default, warning, diagnostics, disablement,
  rollback, and later-removal gate are explicit.
- The existing 15 seed vectors are retained or deliberately versioned, and the
  expanded canonical corpus is consumed completely by both Rust and
  TypeScript for every valid and invalid v1 family with matching outcomes.
- Negative tests cover malformed, oversized, truncated, trailing, unknown,
  wrong-direction, wrong-order, duplicate, fragmented, replayed, resource-
  exhausting, unsupported-version, and capability-violation inputs.
- Four focused Rust fuzz surfaces replay their checked corpus on every normal
  test run, complete a local 60-second run per target, and complete a recorded
  Linux sanitizer run of at least ten minutes per target before issue closure,
  with zero crash, panic, out-of-bounds access, unbounded allocation, or hang.
- Matching current, gateway-first legacy, old-gateway/new-client rollback,
  unsupported old/future, and malformed handshake fixtures pass the real
  compatibility smoke. The current feature smoke exercises tiles, ROI video,
  input, resize, clipboard, desktop audio, and file transfer where enabled.
- Safe live protocol diagnostics and typed user feedback are visible through
  the SDK and existing Admin-New session experience without credentials,
  resource IDs in metric labels, browser content, media/file bytes, URLs, or
  raw errors.
- README, ARCH, validation, operator, risk/maturity, compatibility matrix,
  deprecation/release-note, and claim wording agree. No standardization,
  arbitrary-client, Production, scale, or release-governance claim is inferred.

## Test Strategy

### Unit

- Rust: exact encode/decode for every channel/message/negotiation variant;
  lowest/highest numeric values; strict trailing data; unknown tags/enums;
  bounded `FrameDecoder`; canonical version/capability lists; highest-common
  selection; legacy eligibility; downgrade and duplicate rejection.
- TypeScript: mirror the Rust outcomes for envelope and every message family;
  enforce channel IDs, encode/decode size limits, pending-buffer limits,
  handshake state/order, delayed ready callback, selected-version mismatch,
  optional/required capability behavior, legacy fallback, typed errors, and
  cleanup.
- Preserve and extend Rust `proptest` round trips. Add deterministic TypeScript
  property/mutation tests for arbitrary stream chunk boundaries and corpus
  mutations, with fixed seeds recorded on failure.

### Shared Vectors

- Give every vector a schema version, stable name, direction/transport,
  capability precondition, exact hex, expected decoded fields or fixed error,
  and whether it is a complete reliable frame or raw datagram payload.
- Cover the envelope, all control/input/cursor/clipboard/file/tile variants,
  audio/video framing, negotiation accept/reject, min/max values, concatenated
  frames, every valid split point, and invalid/truncated/oversized forms.
- Rust and TypeScript must enumerate the same catalog and fail if an entry is
  skipped, renamed without versioning, has an unknown outcome, or produces
  different bytes/error classes.

### Integration And Validation Errors

- Gateway tests prove auth precedes negotiation; rejected peers do not resolve
  a runtime, join a hub, consume viewer capacity, claim owner, resize, forward
  input/media/files, or disturb an existing client.
- Exercise host legacy-ready normalization, policy/viewer flag filtering,
  exact ready replay, changed-version rejection, late join, reconnect, and two
  clients negotiating different optional subsets against one live runtime.
- Exercise arbitrary reliable-stream fragmentation, concatenation, premature
  EOF, unknown channel/tag, trailing bytes, wrong direction/state, duplicate
  hello/fragments, impossible fragment totals, oversized declared/actual
  values, and bounded cleanup after failure.
- Keep the in-memory/Postgres session-store contract N/A: no persisted model
  changes. Preserve gateway transport/policy/session-hub and host dispatch
  regressions because negotiation changes their ordering.

### Fuzz

- Add `cargo-fuzz` targets for frame envelope/incremental decoding, typed
  message dispatch across all typed channels, handshake/state selection, and
  video/file fragment parsing/reassembly.
- Seed them from the shared vectors and prior regression inputs. Bound input,
  allocation, fragment count, and execution time inside the harness so a
  resource-exhaustion case is observable rather than exhausting the runner.
- Replay minimized corpora in deterministic Rust tests. TypeScript replays the
  same malformed corpus and runs seeded mutation/property tests; it does not
  claim memory-safety fuzz evidence from Vitest alone.

### Smoke And E2E

- Add one package-owned `smoke:protocol-compatibility` command using the real
  local Compose gateway/browser path plus checked current and previous client/
  gateway fixtures.
- Assert matching v1 feature operation, both rolling-upgrade directions,
  legacy disablement, unsupported old/future rejection, malformed hello
  rejection, visible SDK/Admin-New failure, sanitized diagnostics, reconnect,
  late join, and cleanup.
- Run the existing session/file/audio/multisession smokes needed to exercise
  tiles, ROI video, input, resize, clipboard, audio, file transfer, viewer
  policy, and late-join behavior. Camera/microphone device success is required
  only when the supported test host exposes those devices/codecs; negotiation
  and denial remain deterministic without them.

### Coverage And Quality

- Required focused commands include `cargo test -p bpane-protocol`, targeted
  gateway transport/session-hub tests, targeted host dispatch tests, client
  protocol/control/stream tests, TypeScript check/build, the browser-client
  coverage ratchet, the new compatibility smoke, repository documents, and
  `git diff --check`.
- The shared-vector and fuzz-corpus replay tests run in the normal fast floor;
  bounded live fuzz and Compose compatibility evidence run in their explicit
  stages. A broad workspace or product build does not replace focused parser,
  negative, sanitizer, or rolling-compatibility evidence.
- Changed negotiation, parser, and failure branches must be represented in
  coverage. Existing Rust and browser-client floors may not be lowered to land
  the slice.

## Manual Test Sequence

1. Start the supported local Compose profile with gateway and client legacy
   compatibility explicitly enabled; confirm readiness without printing any
   connect credential.
2. Connect the v1 client to the v1 gateway. Verify v1/highest-common selection,
   expected capability intersection, delayed ready state, and safe diagnostics.
3. Exercise tiles, ROI video, input, resize, clipboard, desktop audio, and a
   bounded upload/download where enabled; confirm viewer policy still removes
   interactive capabilities for a restricted viewer.
4. Connect the checked current pre-contract client to the v1 gateway and the v1
   client to the checked old-gateway fixture. Verify the documented legacy
   warning and feature path in each direction.
5. Disable gateway legacy compatibility and retry the old client. Verify typed
   visible rejection before hub membership; re-enable it and confirm recovery.
6. Offer only unsupported old and future versions, a missing required
   capability, a downgrade selection, a malformed/duplicate hello, and a frame
   before hello. Verify each fixed error, connection-only cleanup, and no
   runtime/viewer/input/file side effect.
7. Join a second client, promote/reconnect it, and force cached `SessionReady`,
   grid, and keyframe bootstrap. Verify the selected version stays fixed while
   policy-authorized effective flags and display state recover.
8. Run the complete Rust/TypeScript shared-vector suites, deterministic
   malformed corpus, and the recorded bounded fuzz commands.
9. Inspect SDK and Admin-New diagnostics plus gateway logs/metrics. Verify only
   bounded versions, modes, capability names, durations, and fixed outcomes are
   present, then clean up the disposable session and uploaded/downloaded file.
10. Exercise rollback: with legacy enabled, revert one peer to its checked old
    fixture and reconnect successfully; restore v1, rerun the matching smoke,
    and record exact commits/commands/results.

## Documentation And Claim Impact

- Before implementation, the supported claim remains: the Rust and TypeScript
  implementations interoperate in the current local profile, but no published
  compatibility promise exists.
- After all Definition of Done evidence, documentation may claim BrowserPane
  remote protocol v1 compatibility for the exact matrix and named local/
  qualified deployment profiles. It may not claim third-party interoperability,
  standardization, broad deployment support, capacity, or Production readiness.
- Update ADR 0003 only if implementation changes its accepted direction.
  Update capability maturity and R-007 from merged evidence, not from this
  specification PR. `#72`, `#75`, `#168`, and `#169` remain open for their
  independent gates.

## Definition Of Done

- The normative v1 wire/negotiation/capability/failure contract and exact
  compatibility matrix are reviewed and code-aligned.
- Gateway and client enforce highest-common selection, fail-closed ordering,
  capability/authorization separation, typed rejection, bounded resources,
  and the explicit legacy overlap before normal session operation.
- Rust and TypeScript consume the complete shared vector catalog and agree on
  every valid byte sequence and invalid outcome.
- Focused unit, property, malformed, integration, sanitizer/fuzz, real
  compatibility, feature, viewer, reconnect, and late-join evidence passes
  without lowering coverage or security limits.
- Deployment defaults, rollback, legacy disablement/removal gate, diagnostics,
  Admin-New feedback, SDK errors, operator steps, and release notes are
  documented and tested.
- README, ARCH, validation matrix, capability maturity, R-007, issue, plan, and
  protocol compatibility/release documentation match the merged evidence.
- No OpenAPI, persistence, standalone Admin-New route, product CLI command,
  render/fan-out optimization, release-governance completion, standardization,
  arbitrary-client, scale, or Production claim is inferred.

## Post-Implementation Smoke Sequence

1. Run Rust and TypeScript shared-vector/conformance suites and prove every
   catalog entry is consumed by both implementations.
2. Connect matching v1 gateway/client and exercise tiles, ROI video, input,
   resize, clipboard, audio, and file transfer where enabled.
3. Run the gateway-first and old-gateway/new-client fixtures; verify the exact
   documented legacy path and diagnostics.
4. Disable legacy mode and send unsupported old/future, missing-required-
   capability, downgrade, malformed, duplicate, premature, truncated, and
   oversized inputs; verify typed visible rejection before normal activity.
5. Exercise restricted viewer, late join, owner promotion, reconnect, cached
   ready/grid/keyframe replay, and two optional-capability subsets.
6. Run deterministic malformed-corpus tests and the recorded bounded Rust fuzz
   targets; require zero crash, panic, unsafe access, unbounded allocation, or
   hang.
7. Verify SDK/Admin-New diagnostics and gateway logs/metrics are useful and
   redacted, re-enable the documented initial rollout defaults, and clean all
   disposable session/file artifacts.

## Evidence Record

Record the implementation PR/commit, normative spec and vector-schema
versions, selected capability registry, exact compatibility fixtures, test and
coverage summaries, minimized fuzz corpus/duration/sanitizer result, Compose
feature and failure smoke, diagnostics/redaction review, rollout/rollback
result, release-note/matrix link, residual issue links, and R-007/maturity
disposition. Do not attach credentials, private run logs, browser content,
customer data, media/file payloads, or absolute local paths.
