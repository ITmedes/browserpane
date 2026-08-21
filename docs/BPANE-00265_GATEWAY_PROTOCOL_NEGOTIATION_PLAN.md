# BPANE-00265 Gateway Protocol Negotiation Plan

## Metadata

- Issue: `#265`
- State: Review
- Owner: BrowserPane maintainers
- Lane: Production
- Target gate: Production Baseline gateway enforcement checkpoint
- Depends on: closed `#264`
- Parent program: `#175`; protocol slice 3 of 6
- Last verified commit/date: `93804b7c` / 2026-08-22

## Business Outcome

The gateway rejects incompatible or malformed browser peers before they affect
runtime or shared-session state, while preserving a bounded gateway-first
upgrade path for the checked current pre-contract client.

## Example Use Case

An unsupported future client targets a busy shared session. The gateway returns
a typed error and closes only that connection; runtime, owner, viewer capacity,
and healthy clients are unchanged. A checked old client can use explicit legacy
mode during rollout.

## Current Evidence

The gateway authenticates WebTransport and decodes bounded frames, but resolves
runtime/session-hub behavior without protocol negotiation. It caches and
filters `SessionReady`; the host emits the pre-contract version marker `2`.

## Scope

- Add authenticated pre-session negotiation before runtime resolution, hub
  join, viewer-slot use, ownership, resize, or forwarding.
- Enforce timeout, pending bytes, list limits, ordering/direction, highest-common
  selection, immutable capability upper bound, and typed rejection.
- Normalize browser-facing `SessionReady.version` to v1 while preserving host
  compatibility and late-join/session-policy behavior.
- Add the checked old-client legacy profile with explicit initial enablement,
  warnings, diagnostics, reversible disablement, and no malformed-hello fallback.
- Add no-side-effect, isolation, policy, replay, reconnect, and viewer tests.

## Non-Goals

- New browser negotiation/fallback, Admin-New, fuzz campaign, final Compose
  matrix, persistent history, legacy removal, auth/policy redesign, or API work.

## Decisions And Dependencies

- Authentication and session visibility precede protocol detail disclosure.
- Negotiation frames are gateway-owned and never forwarded to the host.
- Successful selection is immutable. Effective features remain the intersection
  of negotiation, runtime, policy, browser support, and role.
- #266 owns the browser side; #268 owns final compatibility qualification.

## Contract Changes

- API/OpenAPI: N/A; WebTransport contract only.
- Protocol/event schemas: consume #264 messages at a pre-session state boundary.
- Database/migrations: N/A; diagnostics are connection-local.
- Admin-new: N/A in this slice.
- CLI/SDK: N/A.
- Deployment/configuration: bounded handshake timeout and gateway legacy flag;
  spell out the initial enabled value in local/production-like manifests.
- README/ARCH/AGENTS/operator docs: gateway ordering, flags, safe diagnostics,
  and rollback notes.

## Security And Data Impact

Bound bytes, counts, pending state, parser work, and time. Malformed/downgrade
attempts fail closed and never fall back. Close only the offender. Logs/metrics
contain fixed outcomes and bounded modes, never credentials, URLs, claims,
browser data, raw frames/errors, paths, or resource IDs as labels.

## Migration, Compatibility, And Rollback

Deploy gateway first with legacy enabled. Checked current clients continue via
the explicit profile. Rollback to the previous gateway remains valid because
#266 has not changed the client. Disablement is reversible and tested.

## Observability And Operator Feedback

Add label-bounded attempts/success/legacy/failure counters, handshake duration,
and sanitized connection diagnostics. Warnings identify temporary legacy mode
without exposing session or identity data.

## Implementation Slices

1. Gateway state machine and pure selection integration.
2. SessionReady normalization, capability/direction enforcement, and legacy.
3. Metrics/logs/config plus no-side-effect and shared-session regressions.

## Test Strategy

### Unit

Cover all state transitions, timeout/limits, duplicate/order/direction errors,
selection, legacy eligibility, no malformed fallback, and immutable selection.

### Integration

Prove rejected peers do not resolve/start runtime, join hub, use viewer slots,
claim owner, resize, or forward data; verify existing clients stay healthy.

### Smoke And E2E

Use supported Compose with the checked old-client fixture, valid v1 test peer,
legacy disable/recovery, restricted viewer, late join, owner promotion,
reconnect, and cleanup.

### Coverage And Quality

Run gateway/protocol/host-focused tests, Rust workspace affected tests, clippy,
coverage evidence for negotiation branches, Compose checks, repository
validation, and `git diff --check` without lowering floors.

## Manual Test Sequence

1. Start Compose with gateway legacy mode explicitly enabled.
2. Connect the checked old client and verify legacy warning plus normal use.
3. Negotiate v1 with two optional-capability subsets.
4. Send every fixed malformed/unsupported/order/limit case while another client
   remains active; verify offender-only cleanup and no state side effects.
5. Disable legacy, verify typed old-client rejection, restore it, and reconnect.
6. Exercise late join, owner promotion, restricted viewer, and reconnect.

## Documentation And Claim Impact

This slice may claim gateway-side v1 enforcement and an initial old-client
overlap. It cannot claim new-browser enforcement or complete rolling support.

## Definition Of Done

- Pre-session enforcement and no-side-effect evidence pass.
- Legacy enable/disable/rollback and diagnostics are tested and documented.
- Shared-session, policy, replay, and existing-client behavior remain green.
- Configuration, issue, plan, architecture, validation, and claims agree.

## Post-Implementation Smoke Sequence

1. Run focused gateway/protocol/host tests and coverage.
2. Run valid v1 and checked old-client Compose connections.
3. Run the complete typed rejection/no-side-effect set.
4. Run viewer, late-join, promotion, reconnect, and cleanup checks.
5. Run repository validation and inspect log/metric redaction.

## Evidence Record

- Implementation commit: `93804b7c`, based on `b537e0c0fc68`.
- Configuration: 3,000 ms default with a validated 100–10,000 ms range;
  checked legacy compatibility is explicit and initially enabled in local and
  single-node Compose. API/OpenAPI, database, Admin-New, CLI, and SDK changes
  are N/A because this is a WebTransport gateway boundary.
- `cargo test -p bpane-gateway transport::negotiation -- --nocapture`: pass,
  10 negotiation/capability/direction/timeout/limit tests.
- `cargo test -p bpane-gateway`: pass, 499 passed and 1 ignored; shared-session
  owner promotion, restricted viewer, late-join, reconnect, policy, and replay
  regressions are included in the suite.
- `cargo test -p bpane-protocol`: pass, including 98 unit, 17 integration, 3
  negotiation-fixture, 16 property, 4 wire-fixture, and 1 doc test.
- `cargo test -p bpane-host`: pass, 371 tests. `cargo test --workspace`: pass.
- `cargo clippy -p bpane-gateway --all-targets --all-features -- -D warnings`:
  pass. `node scripts/run-rust-coverage.mjs`: pass and report written.
- In `code/web/bpane-client`, `npm run check && npm test && npm run build`:
  pass, 695 tests and production build. `npm run test:coverage`: pass, coverage
  baseline accepted at 93.21% statements and 88.32% branches.
- `docker compose -f deploy/compose.yml config --quiet`: pass.
  `docker compose -f deploy/compose.yml up -d --build gateway`: pass with the
  final release gateway image.
- In `code/web/bpane-client`, `npm run smoke:gateway-protocol -- --headless
  --connect-timeout-ms 60000`: pass for two selected capability subsets,
  selection/readiness ordering, typed malformed/unsupported/order/limit/
  timeout rejection, no pre-handshake runtime or client effects, offender-only
  rejection, checked current-client overlap, legacy disablement, restoration,
  reconnect, and cleanup.
- In `code/web/bpane-client`, `npm run smoke:multisession -- --headless`: pass
  for two live runtimes, shared-session late join, MCP routing, and cleanup.
- `node scripts/validate.mjs --profile fast`: pass, all 44 stages on the final
  tree. `git diff --check`: pass.
- Live `/metrics` and bounded gateway log review showed only fixed outcome/
  reason labels and the fixed legacy warning; no resource IDs, credentials,
  URLs, claims, content, raw frames, or raw errors were emitted.
- Residual work remains intentionally owned by #266 (browser enforcement),
  #267 (fuzzing), and #268 (final rolling compatibility qualification).
