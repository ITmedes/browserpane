# BPANE-00264 Protocol Negotiation Codecs Plan

## Metadata

- Issue: `#264`
- State: Review
- Owner: BrowserPane maintainers
- Lane: Production
- Target gate: Production Baseline codec/conformance checkpoint
- Depends on: closed `#263`
- Parent program: `#175`; protocol slice 2 of 6
- Last verified commit/date: `7364e6a4` / 2026-08-22

## Business Outcome

Rust and TypeScript implement one byte-identical, bounded negotiation codec and
selection model before either runtime connection path consumes it.

## Example Use Case

A client offers v1, requires clipboard, and optionally supports ROI video. Both
languages select the same version/capabilities, while an unknown required
capability or downgrade offer maps to the same typed rejection.

## Current Evidence

#263 froze the numeric and semantic contract through PR #270. At this slice's
`9f5ce6dd29e5` base, code has envelope and typed-message codecs but no
`ClientHello`, selection, rejection, capability-registry implementation, or
shared negotiation vectors.

## Scope

- Add bounded Rust and TypeScript codecs for `ClientHello`, server selection,
  and protocol rejection using #263 assignments.
- Add pure highest-common selection, canonical unique lists, required/optional
  capability handling, deterministic intersection, and downgrade refusal.
- Expand the schema-versioned shared corpus with all negotiation messages,
  boundaries, valid selections, and fixed invalid outcomes.
- Add exact, boundary, property, and deterministic mutation tests.

## Non-Goals

- WebTransport integration, runtime/hub work, host normalization, legacy timing,
  Admin-New, fuzz campaigns, Compose compatibility, authorization, or policy.

## Decisions And Dependencies

- #263 is immutable input; this slice does not redefine numeric assignments.
- Selection is pure and side-effect free. Unknown optional IDs are ignored;
  unknown required IDs reject. The highest common version is mandatory.
- #265 and #266 may consume the APIs only after cross-language parity passes.

## Contract Changes

- API/OpenAPI: N/A.
- Protocol/event schemas: additive negotiation codecs and shared vectors.
- Database/migrations: N/A.
- Admin-new: N/A.
- CLI/SDK: intentional TypeScript protocol exports only; no connection change.
- Deployment/configuration: N/A.
- README/ARCH/AGENTS/operator docs: update implementation references only if
  public exports or architecture ownership change.

## Security And Data Impact

Bound message bytes, list counts, numeric ranges, duplicate/ordering work, and
allocation. Error types must not include raw peer payloads or sensitive data.
Capability results cannot grant authorization.

## Migration, Compatibility, And Rollback

The codecs are additive and unused by runtime code. Rollback removes the codec,
exports, and vectors as one unit. Existing envelope/message behavior remains
unchanged until #265/#266.

## Observability And Operator Feedback

N/A at runtime. Tests expose stable outcome codes; logging and UI are owned by
#265, #266, and #268.

## Implementation Slices

1. Rust types, strict codec, pure selector, and focused tests.
2. TypeScript mirror and intentional exports.
3. Complete shared negotiation vectors and cross-language parity checks.

## Test Strategy

### Unit

Cover empty/min/max, duplicate, unsorted, unknown optional/required,
unsupported version, downgrade, truncated, trailing, oversized, and malformed
values plus selection invariants.

### Integration

Encode hello/selection/rejection in each language and decode in the other;
assert identical bytes and fixed errors for every shared vector.

### Smoke And E2E

No new runtime smoke. Run current browser connection smoke to prove additive
codec exports do not affect initialization.

### Coverage And Quality

Run Rust protocol tests/properties, TypeScript tests/type/build/coverage,
format/clippy, dependency checks where manifests change, repository validation,
and `git diff --check`. Preserve coverage floors.

## Manual Test Sequence

1. Encode/decode representative negotiation frames in Rust.
2. Decode those bytes in TypeScript and reverse the direction.
3. Run all valid, boundary, and invalid shared vectors.
4. Verify the pure selector never returns a non-mutual version/capability.
5. Run the unchanged browser connection smoke.

## Documentation And Claim Impact

Documentation may say v1 negotiation codecs exist and conform to shared vectors,
not that deployed gateway/client negotiation or rolling compatibility exists.

## Definition Of Done

- Both languages agree on all bytes and outcomes.
- Strict boundaries and selector invariants are covered.
- Runtime behavior remains unchanged.
- Issue, plan, vector schema, exports, and docs agree.

## Post-Implementation Smoke Sequence

1. Run Rust unit/property/shared-vector tests.
2. Run TypeScript unit/shared-vector/coverage tests.
3. Run bidirectional cross-encoding checks.
4. Run current browser-client connection smoke.
5. Run repository validation and `git diff --check`.

## Evidence Record

- Branch `codex/BPANE-00264-protocol-negotiation-codecs`; implementation commit
  `7364e6a4`.
- Rust and TypeScript expose strict bounded codecs, typed payload-free failures,
  and the pure highest-common selector. The TypeScript codec and selector are
  intentional public protocol exports; neither runtime connection path consumes
  them in this slice.
- Shared schema v2 contains 66 enumerated cases: the 15 preserved envelope/
  current-message vectors, 41 negotiation wire vectors, and 10 pure selection
  vectors. Both languages reproduce valid bytes and fixed invalid outcomes.
- `cargo fmt --all -- --check`: PASS.
- `cargo clippy -p bpane-protocol --all-targets --all-features -- -D warnings`:
  PASS.
- `cargo check -p bpane-protocol --no-default-features`: PASS.
- `RUSTDOCFLAGS='-D warnings' cargo doc -p bpane-protocol --no-deps`: PASS.
- `cargo test -p bpane-protocol`: PASS, 139 tests including unit, integration,
  shared-vector, property, and documentation tests.
- `npx tsc --noEmit`: PASS in `code/web/bpane-client`.
- `npm test`: PASS in `code/web/bpane-client`, 91 files / 695 tests.
- `npm run test:coverage`: PASS in `code/web/bpane-client`; 93.21% statements,
  88.32% branches, 93.56% functions, and 93.21% lines.
- `npm run build`: PASS in `code/web/bpane-client`.
- `npm run smoke:test-embed-lifecycle -- --headless --connect-timeout-ms 60000`:
  PASS against the local Compose stack, including cleanup.
- `node scripts/check-repository-documents.mjs`: PASS, 114 Markdown files,
  19 YAML files, and 3 workflow files.
- `node scripts/validate.mjs --stage repository-documents`: PASS.
- `node scripts/validate.mjs --profile fast`: PASS, all 44 selected stages.
- `git diff --check`: PASS.
- API/OpenAPI, persistence, Admin-New, deployment, runtime behavior, and
  authorization are N/A for this isolated codec slice. README, architecture,
  protocol, validation, maturity, and risk documentation are aligned. No
  dependency manifest changed.
- No #264-required validation is deferred. Runtime negotiation integration,
  fuzz campaigns, and real rolling-compatibility matrices remain accepted
  non-goals owned by #265 through #268.
