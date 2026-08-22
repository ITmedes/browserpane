# BPANE-00280 Protocol Bootstrap Cleanup Plan

## Metadata

- Issue: `#280`
- State: In Progress
- Owner: `thebackplane`
- Lane: Foundation
- Target gate: restore deterministic exact-head and post-merge Compose evidence
- Depends on: completed gateway negotiation slice `#265`
- Last verified commit/date: `02b2c1dd`, 2026-08-22

## Business Outcome

Operators can connect, switch, disconnect, stop, and release sessions through
the supported compatibility window without transport chunking leaving a stale
session owner. The Compose delivery gate then distinguishes product defects
from transient runner failures instead of repeatedly failing the same browser
lifecycle smoke.

## Example Use Case

An operator opens `/admin/`, creates a session, and connects while the browser
client flushes its resolution and keyboard frames in one WebTransport read.
BrowserPane accepts the first checked legacy frame, preserves the following
complete or partial bytes for normal ingress processing, and establishes the
session. If the connection closes while the gateway sends bootstrap frames,
the admitted client is removed from the registry so disconnect-all makes the
session stop-eligible and the operator can release or stop it.

## Current Evidence

- Main Compose run `32584253673` failed `compose-admin-compat` on attempt 1
  after gateway negotiation rejected the browser with
  `unexpected_protocol_frame`.
- Failed-job rerun attempt 2 selected checked legacy compatibility, then an
  initial-frame write failed for the original connection after registry
  admission.
- The gateway logged the later `disconnect-all` and WebTransport termination,
  but the session resource retained a stop blocker until smoke cleanup killed
  it.
- `GatewayNegotiation::ingest` currently rejects any buffered bytes after the
  first checked legacy frame, although stream reads may legally coalesce or
  split frames.
- `handle_session` currently uses `?` for `send_initial_frames` after
  `join_with_role`, bypassing `SessionRegistry::leave` and idle transition.
- The browser client does not yet negotiate protocol v1; that remains `#266`.

## Scope

1. Add a bounded `FrameDecoder` handoff for unconsumed bytes.
2. Preserve bytes after the first checked legacy frame and seed the normal
   ingress decoder without broadening which first frame may select legacy mode.
3. Ensure a client admitted to the session registry is removed and the usual
   idle transition is evaluated when bootstrap frame delivery fails.
4. Add focused unit regressions and rerun compatibility/protocol Compose
   validation.

## Non-Goals

- Browser-side v1 negotiation, typed browser compatibility errors, or readiness
  gating owned by `#266`.
- Relaxing malformed hello, downgrade, capability, direction, pending-buffer,
  or negotiated-v1 enforcement.
- Removing `/admin/`; it remains the documented compatibility fallback.
- Increasing smoke timeouts or post-merge retry budgets to mask the defect.

## Decisions And Dependencies

- WebTransport is a byte stream at this boundary; one read is not one protocol
  frame. Decoder state must cross negotiation into ingress.
- Only the first complete checked legacy client frame selects compatibility.
  Later bytes are ordinary ingress and remain subject to existing transport
  policy.
- A registry admission establishes lifecycle cleanup ownership. Every error
  after that point must leave the registry before returning.
- `#266` remains next in the protocol sequence after this repair is merged and
  its post-merge Compose evidence is green.

## Contract Changes

- API/OpenAPI: N/A; no HTTP resource shape or route changes.
- Protocol/event schemas: N/A; existing bytes and compatibility rules are
  preserved. Stream chunk handling is corrected.
- Database/migrations: N/A.
- Admin-new: N/A; no UI behavior or route changes.
- CLI/SDK: N/A; no public API changes.
- Deployment/configuration: N/A; existing
  `BPANE_GATEWAY_PROTOCOL_LEGACY_COMPATIBILITY` behavior is repaired.
- README/ARCH/AGENTS/operator docs: no product claim changes. Current context,
  roadmap, validation evidence, and issue indexes must record the repair.

## Security And Data Impact

No authentication, authorization, secret, URL, browser-content, file,
artifact, egress, retention, or tenant boundary changes. Negotiation retains
its fixed 153-byte first-frame bound and checked first-frame allowlist. The
complete negotiation buffer is separately bounded to one maximum negotiation
frame plus one 4 KiB transport read. Handoff copies only already-buffered
protocol bytes into the existing bounded ingress decoder. Cleanup reduces stale
authorization/lifecycle state rather than creating a new access path.

## Migration, Compatibility, And Rollback

The change is additive for supported checked legacy clients and neutral for
negotiated v1 clients. No persistence migration is required. Rollback is a code
revert, but would restore the known coalescing rejection and stale-client leak;
forward-fix is therefore preferred. Legacy compatibility remains explicitly
disableable and its removal stays owned by the protocol rollout plan.

## Observability And Operator Feedback

Existing bounded protocol selection/rejection metrics and sanitized gateway
logs remain authoritative. No resource identifiers or peer bytes are added to
metrics. Bootstrap failure remains an error, but registry and session status
must converge to zero clients and stop-eligible state.

## Implementation Slices

1. Decoder handoff and legacy coalescing/split-frame regression coverage.
2. Post-admission bootstrap cleanup and session lifecycle regression coverage.
3. Documentation/evidence synchronization and hosted Compose qualification.

## Test Strategy

### Unit

- Prove `FrameDecoder` can transfer unconsumed bytes without changing order.
- Accept two checked legacy frames received together.
- Preserve a partial second frame and complete it in ingress.
- Keep hello-plus-trailing-frame rejection and all typed negative outcomes.
- Force initial-frame writer failure and prove admitted registry clients are
  removed.

### Integration

- Run `cargo test -p bpane-protocol` and focused gateway transport tests.
- Run `cargo test -p bpane-gateway` and clippy/format checks.
- Run browser-client unit, type, and build validation because its legacy send
  behavior is the real peer exercised by Compose.

### Smoke And E2E

- Run `compose-admin-compat` against a freshly built local Compose stack.
- Run the gateway protocol smoke with checked legacy enabled and disabled.
- Dispatch exact-head Compose and require every matrix job to pass.
- After merge, require Validation and Compose on the exact merge SHA.

### Coverage And Quality

Changed branches require direct unit assertions; no timeout-only workaround is
accepted. Run rustfmt, clippy with warnings denied, focused Rust coverage where
practical, repository fast validation, and dependency/document policy checks.

## Manual Test Sequence

1. Start local Compose and wait for `/readyz`.
2. Open `/admin/`, authenticate, create a session, and connect.
3. Switch to another session and back while the first runtime stays available.
4. Disconnect through the lifecycle inspector.
5. Verify client count reaches zero and Stop/Release becomes enabled.
6. Release, reconnect, disconnect, and stop the session.
7. Disable legacy compatibility, reconnect with the legacy client, and verify a
   typed downgrade refusal without a runtime/registry client.
8. Restore compatibility and clean up the session.

## Documentation And Claim Impact

No README, ARCH, OpenAPI, capability-maturity, security-baseline, or investor
claim changes are required because the supported contract is unchanged. Update
delivery/current-context documents only to record the repair and validation
evidence.

## Definition Of Done

- Issue acceptance criteria are complete.
- Coalesced and split legacy bytes pass focused regression tests.
- Bootstrap write failure cannot leave a registry client or stop blocker.
- Protocol and compatibility Compose smokes pass without timeout increases.
- Exact-head PR and post-merge main Validation/Compose are green.
- Issue, plan, roadmap, current context, validation matrix, and evidence links
  are synchronized.

## Post-Implementation Smoke Sequence

1. `cargo test -p bpane-protocol frame_decoder`
2. `cargo test -p bpane-gateway transport::negotiation`
3. `cargo test -p bpane-gateway`
4. `node scripts/validate.mjs --profile fast`
5. From `code/web/bpane-client`, run `npm test`, `npx tsc --noEmit`, and
   `npm run build`.
6. Start Compose and run
   `node scripts/validate.mjs --stage compose-admin-compat`.
7. From `code/web/bpane-client`, run
   `npm run smoke:gateway-protocol -- --headless --connect-timeout-ms 60000`.
8. Dispatch exact-head Compose, merge only when green, and verify Validation
   plus Compose on the exact merge SHA.

## Evidence Record

- Triggering run: [Compose `32584253673`](https://github.com/ITmedes/browserpane/actions/runs/32584253673), attempts 1 and 2.
- Issue: [#280](https://github.com/ITmedes/browserpane/issues/280).
- PR, commits, focused output, coverage, and hosted run links: pending.
