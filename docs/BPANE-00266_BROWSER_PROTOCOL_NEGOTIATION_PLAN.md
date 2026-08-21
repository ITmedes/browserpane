# BPANE-00266 Browser Protocol Negotiation Plan

## Metadata

- Issue: `#266`
- State: Qualified
- Owner: BrowserPane maintainers
- Lane: Production
- Target gate: Production Baseline browser enforcement checkpoint
- Depends on: closed `#265`
- Parent program: `#175`; protocol slice 4 of 6
- Last verified commit/date: `e21e2206a93a` / 2026-08-21

## Business Outcome

The browser SDK negotiates before declaring a session ready, gates optional
behavior consistently, and reports stable compatibility failures. A checked
old gateway remains usable during client-first rollout or rollback.

## Example Use Case

An embedding application upgrades its browser client first. Against the checked
old gateway it enters explicit legacy mode with a warning. Against v1 it
validates selection and `SessionReady.version`; an incompatible peer yields a
typed error rather than a generic opening-handshake failure.

## Current Evidence

The TypeScript client parses the frame envelope and session flags but ignores
`SessionReady.version`, has no hello/selection state, and reports readiness
without a negotiated contract. Stable `BpaneError` subclasses already provide
the error abstraction to extend.

## Scope

- Send `ClientHello` first and validate selection/rejection plus matching
  `SessionReady.version` before public ready, input, or media handling.
- Gate features by negotiated support, runtime flags, browser support, project
  policy, and access role.
- Add the exact checked old-gateway fallback, explicit initial client setting,
  warning, reversible disablement, and no malformed-v1 fallback.
- Add typed compatibility errors and a read-only sanitized SDK snapshot.
- Preserve reconnect, late join, viewer restrictions, tiles/video/audio/input,
  independent resize, clipboard, and file behavior.

## Non-Goals

- Admin-New presentation, durable history, product CLI, fuzz campaign, final
  Compose matrix, authorization changes, or legacy removal/default change.

## Decisions And Dependencies

- #265 supplies the gateway-first path and selected contract.
- Ready/input/media are impossible before validated v1 or exact enabled legacy.
- Capability selection never grants behavior removed by runtime/policy/role.
- #268 owns final UI, deployment matrix, and program closure.

## Contract Changes

- API/OpenAPI and database: N/A.
- Protocol/event schemas: browser consumes #264 handshake and #265 ordering.
- Admin-new: N/A in this slice; errors/snapshot are integration inputs for #268.
- CLI/SDK: additive stable error codes/classes and read-only diagnostics.
- Deployment/configuration: explicit client legacy compatibility, initially on.
- README/ARCH/AGENTS/operator docs: SDK handshake, errors, fallback, rollback.

## Security And Data Impact

Reject malformed, changed, downgrade, premature, or capability-violating data.
The snapshot exposes only bounded version, mode, capability names, duration,
and fixed outcome. Exclude credentials, URLs, claims, content, media/file data,
paths, and raw peer bytes/errors.

## Migration, Compatibility, And Rollback

Client-first rollout works only against the exact checked old-gateway shape.
Disablement is reversible. Rollback uses the checked old client with #265
gateway legacy mode. Unknown or malformed v1 behavior never becomes legacy.

## Observability And Operator Feedback

Expose a read-only connection snapshot and stable `BpaneError` codes/messages.
No high-cardinality metrics or persistent protocol history are introduced.

## Implementation Slices

1. Client handshake state, delayed readiness, and strict selection validation.
2. Capability gating, old-gateway fallback, errors, and snapshot.
3. Regression coverage across browser/session features and reconnect behavior.

## Test Strategy

### Unit

Cover success, selection mismatch, malformed response, downgrade, timeout,
premature frames, capability violation, legacy eligibility/disablement, typed
errors, cleanup, and snapshot redaction.

### Integration

Exercise new client/new gateway, new client/checked old gateway, and current
client/new gateway fixtures. Assert event ordering and feature intersections.

### Smoke And E2E

Run affected browser-client, multisession, restricted-viewer, reconnect,
tile/video, input/resize, clipboard/audio, and file-transfer paths.

### Coverage And Quality

Run TypeScript test/type/build/coverage, protocol and gateway integration tests,
affected smokes, dependency checks where manifests change, repository
validation, and `git diff --check`. Preserve coverage floors.

## Manual Test Sequence

1. Connect to v1 and verify ready occurs after selection and SessionReady.
2. Exercise two capability subsets and role/policy-denied actions.
3. Connect to the checked old gateway with legacy enabled.
4. Disable legacy and verify a typed failure; restore and reconnect.
5. Run mismatch, malformed, downgrade, timeout, premature, and capability cases.
6. Exercise reconnect, late join, media, input, resize, clipboard, and files.

## Documentation And Claim Impact

After this slice, gateway and browser enforcement exist with both overlap
directions. Final qualification, Admin-New evidence, fuzzing, and broad matrix
remain #267/#268; do not claim completion before them.

## Definition Of Done

- Handshake ordering, typed failures, snapshot, and fallback pass.
- Feature/authority intersection and viewer restrictions remain enforced.
- Browser unit/build/coverage and impacted smokes remain green.
- SDK exports, docs, issue, plan, and rollback contract agree.

## Post-Implementation Smoke Sequence

1. Run browser unit/type/build/coverage.
2. Run v1 and old-gateway fixture integrations.
3. Run typed negative and legacy disable/recovery cases.
4. Run multisession/viewer/reconnect and feature smokes.
5. Verify snapshot/error redaction and repository validation.

## Evidence Record

Record PR/commit, SDK exports, compatibility fixtures, event ordering,
capability/authority assertions, coverage and smoke results, redaction review,
rollback result, and residual links.
