# BPANE-00263 Remote Protocol Contract And Vector Baseline Plan

## Metadata

- Issue: `#263`
- State: Review
- Owner: BrowserPane maintainers
- Lane: Production
- Target gate: Production Baseline protocol contract checkpoint
- Depends on: accepted ADR 0003, merged `#151` validation floor
- Parent program: `#175`; protocol slice 1 of 6
- Last verified commit/date: `5dc1c3f839ce` / 2026-08-21

## Business Outcome

BrowserPane has one language-neutral v1 wire contract and one shared current
vector baseline before runtime negotiation changes. Rust, TypeScript, gateway,
and host work can then be reviewed against the same byte-level truth.

## Example Use Case

A maintainer changes file-transfer or ROI-video framing. The normative document
and complete Rust/TypeScript consumption of the same fixtures expose any byte,
limit, direction, or error-classification drift before rollout.

## Current Evidence

- `bpane-protocol` has a five-byte little-endian envelope, 16 MiB payload cap,
  eleven channels, typed messages, bounded incremental decoding, properties,
  and mixed-channel tests.
- The host emits `SessionReady.version = 2`; the browser ignores that byte and
  the existing control fixture contains `1`. Neither is a published version.
- The shared JSON catalog has 15 valid/invalid seeds. Rust's shared-fixture
  tests reference all 15 current entries. TypeScript tests reference 13
  distinct entries across control, input/clipboard, file, tile, and video
  paths; only `audio_out_frame` and `cursor_shape_small` are not consumed.
  Neither language has catalog-level exhaustive enumeration, so a newly added
  unreferenced entry can still be ignored silently.
- No normative wire document, frozen capability registry, complete shared
  consumer contract, or published compatibility matrix exists.

## Scope

- Publish `REMOTE_PROTOCOL_V1.md` covering every current envelope, channel,
  direction, transport, message, media frame, limit, order, fragmentation,
  replay/duplicate, validation, and close rule.
- Freeze v1 numeric negotiation tags, capability IDs, typed failures, minimal
  core, and the initial current/legacy compatibility matrix.
- Version the shared fixture schema and make both languages enumerate and
  assert every existing seed without changing runtime connection behavior.
- Define protocol change/deprecation rules and align relevant architecture,
  validation, maturity, risk, and support wording.

## Non-Goals

- Runtime negotiation, gateway/client state changes, fuzz targets, Admin-New,
  release governance, render/fan-out optimization, API, or persistence work.
- Industry-standard, arbitrary-client, Production, or scale claims.

## Decisions And Dependencies

- Protocol v1 is the first published negotiated version. Existing version bytes
  are pre-contract evidence, not a retroactive public v2 promise.
- #263 freezes the contract consumed by #264-#268. Numeric or semantic changes
  after this slice require an explicit versioned compatibility review.
- Capabilities describe mutual wire understanding, never authorization,
  runtime availability, project policy, or browser role.

## Contract Changes

- API/OpenAPI: N/A; WebTransport remains outside the owner control API.
- Protocol/event schemas: normative v1 document, fixture-schema version,
  numeric registry, failure vocabulary, and initial compatibility matrix.
- Database/migrations: N/A; no persisted model changes.
- Admin-new: N/A; no runtime diagnostics exist yet.
- CLI/SDK: TypeScript test consumer only; no public runtime behavior.
- Deployment/configuration: N/A.
- README/ARCH/AGENTS/operator docs: align protocol facts and claim boundary;
  AGENTS changes only if architecture ownership changes.

## Security And Data Impact

The specification must bound lengths, counts, pending bytes, directions, and
state. Fixtures contain synthetic protocol data only. They must not include
credentials, URLs, identity claims, browser content, media/file payloads,
customer data, local paths, or production traffic.

## Migration, Compatibility, And Rollback

This is additive documentation and test-consumer work with no deployed behavior
change. Revert the slice as one unit before #264 if review rejects the contract.
Later slices must preserve the frozen vectors or version them deliberately.

## Observability And Operator Feedback

No runtime telemetry changes. The document freezes bounded diagnostic fields
and fixed failure names for later slices, including explicit redaction and
metric-cardinality constraints.

## Implementation Slices

1. Audit all Rust/TypeScript/host/gateway wire families and publish the spec.
2. Version and complete enumeration of the existing shared fixture catalog.
3. Add parity/failure tests and synchronize architecture and validation docs.

## Test Strategy

### Unit

Assert exact bytes, decoded fields, stable invalid outcomes, schema version,
unique names, and exhaustive fixture enumeration in Rust and TypeScript.

### Integration

Cross-encode representative current frames in each language and decode them in
the other. No runtime integration changes are expected.

### Smoke And E2E

Run current protocol/browser-client baselines to prove the fixture-consumer
change does not alter the supported connection path.

### Coverage And Quality

Run protocol tests, TypeScript unit/type/build/coverage, repository document
validation, formatting, clippy where touched, and `git diff --check`. Do not
lower existing coverage floors.

## Manual Test Sequence

1. Run both shared-vector suites and confirm 15 entries are reported.
2. Temporarily alter one valid byte; verify both consumers fail, then revert.
3. Temporarily alter one invalid outcome; verify both fail, then revert.
4. Review the normative table against Rust and TypeScript constants.
5. Run repository document validation and inspect unsupported claims.

## Documentation And Claim Impact

The result may claim a published BrowserPane v1 contract baseline, not deployed
negotiation or broad compatibility. Update README, ARCH, validation, maturity,
R-007, and ADR references only to that evidence level.

## Definition Of Done

- #263 acceptance criteria and the manual sequence pass.
- Normative spec and all 15 seeds match code and both consumers.
- Numeric, capability, error, compatibility, and change rules are unambiguous.
- Runtime behavior and existing coverage remain unchanged and green.
- Issue, plan, roadmap, validation, maturity, risk, and claims agree.

## Post-Implementation Smoke Sequence

1. Run Rust protocol/shared-vector tests.
2. Run TypeScript protocol/shared-vector tests and coverage.
3. Perform the two temporary mutation checks and discard them.
4. Run unaffected browser-client unit/build validation.
5. Run repository-baseline/documents validation and `git diff --check`.

## Evidence Record

- Branch: `codex/BPANE-00263-remote-protocol-contract` from
  `ac3a5d5f27f5`; implementation commit `ad77af1e`.
- PR: [#270](https://github.com/ITmedes/browserpane/pull/270)
- Contract/catalog: BrowserPane protocol v1 is normative in
  `REMOTE_PROTOCOL_V1.md`; fixture schema version `1`, catalog
  `browserpane-current-seed`, with all 15 original vectors retained (12 valid,
  3 invalid). Rust and TypeScript enumerate and classify every entry.
- Focused Rust validation passed:
  - `cargo fmt --all -- --check`
  - `cargo test -p bpane-protocol` (125 unit, integration, property, vector,
    and doc tests passed)
  - `cargo clippy -p bpane-protocol --all-targets --all-features -- -D warnings`
  - `cargo check -p bpane-protocol --no-default-features`
  - `cargo doc -p bpane-protocol --no-deps`
- Browser-client validation from `code/web/bpane-client` passed:
  - `npx tsc --noEmit`
  - `npm test` (88 files and 679 tests passed)
  - `npm run test:coverage` (92.91% statements, 87.65% branches, 93.19%
    functions, 92.91% lines; baseline passed)
  - `npm run build`
- Repository validation passed:
  - `node scripts/check-repository-documents.mjs` (113 Markdown, 19 YAML,
    and 3 workflow files)
  - `node scripts/validate.mjs --stage repository-documents`
  - `node scripts/validate.mjs --profile fast` (all 44 local stages passed,
    including Rust workspace coverage at 60.98% lines, 63.77% functions, and
    64.84% regions)
  - `git diff --check`
- Controlled mutation evidence: in a detached temporary worktree, changing one
  valid `SessionReady` tag byte made both the focused Rust and TypeScript
  catalog tests fail. After explicit restoration, changing the expected error
  for the invalid unknown-tile-tag seed made both fail. The catalog was restored
  cleanly and the temporary worktree was removed.
- Contract/claim review: README, ARCH, validation, maturity, roadmap, and R-007
  distinguish this published BrowserPane-specific contract/current seed
  baseline from deployed negotiation, broad conformance, Production, scale, or
  third-party compatibility. API/OpenAPI, persistence, Admin-New, CLI/SDK,
  deployment, and runtime behavior are N/A for this slice; the fast profile
  still proved zero OpenAPI compatibility changes and all unaffected package
  gates.
- Residual work remains explicitly owned by #264-#268: negotiation codecs,
  gateway/browser enforcement, expanded conformance/fuzzing, real Compose
  rolling compatibility, diagnostics, and #175 closure. No #263 runtime smoke
  is deferred because #263 intentionally changes no runtime path.
