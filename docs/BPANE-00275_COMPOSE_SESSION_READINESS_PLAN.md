# BPANE-00275 Compose Session Readiness Plan

## Metadata

- Issue: `#275`
- State: In Progress
- Lane: Foundation
- Related delivery correction: `#273`
- Regression source: protocol gateway merge `6dd4838c`
- Last verified: 2026-08-22

## Business Outcome

Compose validation reports real browser-session failures instead of acting on a
transport connection before the BrowserPane runtime is ready. Compatibility UI
actions that require host capabilities remain unavailable until those
capabilities have actually been advertised.

## Example Use Case

An operator connects to a stopped session. The WebTransport handshake completes
while Docker is still starting the browser runtime. The UI can show the
connection attempt, but Upload remains disabled and automation does not assert a
running runtime until the host sends its session-ready capability state.

## Current Evidence

- Main Compose run `32538701324` failed twice at commit `6dd4838c`.
- The CLI release smoke observed `runtime_state=starting` immediately after the
  fixture reported `connected=true`.
- The compatibility session-files smoke clicked Upload before file-transfer
  capability readiness, so `BpaneSession.promptFileUpload()` correctly emitted
  no file chooser.
- Gateway and Admin-New lanes passed on the rerun; the failures are isolated to
  compatibility-fixture readiness assumptions.

## Scope

1. Expose application readiness and the last advertised capabilities through
   the compatibility fixture's test control state.
2. Keep Upload disabled until file transfer is advertised.
3. Make CLI and compatibility file smokes wait for application capability
   readiness instead of transport-only connection state.
4. Add focused predicate tests and rerun the exact-head Compose matrix.

## Non-Goals

- Reverting protocol negotiation or allocating browser runtimes before the
  handshake.
- Changing the frozen protocol or gateway API.
- Expanding compatibility-admin functionality.

## Contract And Compatibility

- API/OpenAPI, protocol, persistence, CLI, and SDK contracts: N/A.
- The fixture-only `window.__bpaneControl.getState()` gains additive
  `applicationReady` and `capabilities` test fields.
- Existing `connected` semantics remain transport-oriented for compatibility.
- Rollback removes the additive fields and smoke predicates; no migration is
  required.

## Security And Data Impact

No credentials, user data, browser content, or authorization boundaries change.
The readiness state contains booleans only.

## Test Strategy

- Unit: verify transport-only state is rejected, application-ready state is
  accepted, and file readiness requires the advertised file-transfer flag.
- Integration: run browser-client tests/build and the two focused Compose smoke
  stages against local Compose when available.
- E2E: run the full exact-head Compose workflow and require all five lanes.
- Regression: keep gateway protocol negotiation tests and the complete fast
  validation profile green.

## Manual Smoke Sequence

1. Open `/admin/` and create or select a stopped session.
2. Connect and observe that Upload is not actionable during runtime startup.
3. Wait for the session-ready capability state; verify Upload becomes enabled.
4. Upload a file and verify it appears in the selected session's file list.
5. Release and reconnect a runtime; verify status reaches `running` before
   release lifecycle assertions proceed.

## Documentation And Claim Impact

Root README, ARCH, capability claims, deployment docs, and investor material are
unchanged because this corrects fixture and validation synchronization only.

## Definition Of Done

- Transport-only connection state cannot satisfy application-ready smoke gates.
- Upload is disabled until file-transfer capability readiness.
- Focused unit tests, browser-client validation, and exact-head Compose pass.
- Issue `#275` and PR `#274` contain the final evidence.

## Evidence Record

- Focused predicate tests: `3/3` passed.
- Browser client: `695/695` unit tests passed; TypeScript check and production
  build passed.
- Local Compose: `compose-cli` passed in 159.3 seconds and
  `compose-admin-compat-session-files` passed in 85.7 seconds after rebuilding
  the fixture-bearing web image.
- Hosted exact-head Compose: pending the repaired branch commit.
