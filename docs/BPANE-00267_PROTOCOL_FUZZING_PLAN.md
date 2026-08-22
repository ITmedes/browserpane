# BPANE-00267 Protocol Fuzzing And Malformed-State Plan

## Metadata

- Issue: `#267`
- State: Qualified
- Owner: BrowserPane maintainers
- Lane: Production
- Target gate: Production Baseline parser-resilience checkpoint
- Depends on: `#266` (must be closed before this slice enters Ready)
- Parent program: `#175`; protocol slice 5 of 6
- Last verified commit/date: `e21e2206a93a` / 2026-08-21

## Business Outcome

BrowserPane has deterministic regression and bounded fuzz/sanitizer evidence for
the attacker-controlled protocol parser/state boundary before v1 qualification.

## Example Use Case

A peer streams fragmented oversized headers, repeated negotiation frames, and
invalid media/file fragments. The connection fails without panic, hang, unsafe
access, unbounded allocation, runtime start, or impact on a healthy viewer, and
the minimized input becomes a permanent synthetic regression.

## Current Evidence

Rust has property round trips and bounded incremental decode tests; TypeScript
tests chunking and oversized lengths. No cargo-fuzz target, deterministic
cross-language malformed corpus, or sanitizer evidence exists.

## Scope

- Add cargo-fuzz targets for envelope/incremental decode, typed dispatch,
  negotiation state, and video/file fragmentation/reassembly.
- Seed from shared vectors and replay minimized bounded corpus entries in normal
  Rust tests; add corresponding TypeScript malformed/mutation replay.
- Add gateway negative integration proving fail-closed ordering, limits,
  offender-only close, and no pre-handshake runtime/hub side effects.
- Record local 60-second-per-target and Linux sanitizer runs of at least ten
  minutes per target before closure.

## Non-Goals

- Protocol semantic/numeric changes, UI/API/persistence work, formal proof,
  exhaustive verification, or unbounded fuzzing on every pull request.

## Decisions And Dependencies

- #263-#266 define the immutable tested behavior.
- A discovered defect is fixed forward with a documented compatibility review;
  it cannot silently change vectors or selection semantics.
- Checked corpora are synthetic, bounded, deterministic, and code-reviewed.

## Contract Changes

- API/OpenAPI, database, Admin-new, SDK/CLI, deployment runtime: N/A unless a
  validated defect fix requires a separately documented behavior correction.
- Test tooling: reproducible fuzz manifests/commands, corpus replay, and
  sanitizer evidence ownership.
- README/ARCH/AGENTS/operator docs: contributor commands and validation matrix.

## Security And Data Impact

Fuzz inputs are synthetic bytes only. Never seed credentials, URLs, claims,
browser content, recordings, workspace data, production traffic, or raw crash
artifacts containing sensitive state. Bound harness input, allocations,
fragments, and execution time.

## Migration, Compatibility, And Rollback

Test additions are additive. Parser fixes must retain all valid vectors and
document old/new invalid behavior plus forward-fix/rollback limits. Corpus
schema changes are versioned.

## Observability And Operator Feedback

Fuzz runs record revision, target, seed/corpus version, duration, sanitizer,
and fixed failure/crash status. Runtime diagnostics remain those of #265/#266.

## Implementation Slices

1. Fuzz harnesses, bounded dictionaries/seeds, and deterministic replay.
2. TypeScript mutation/malformed corpus parity.
3. Gateway no-side-effect integration and recorded local/Linux evidence.

## Test Strategy

### Unit

Replay every corpus entry and cover malformed, oversized, truncated, trailing,
unknown, wrong-direction/order, duplicate, replayed, fragmented, unsupported,
and capability-violating input.

### Integration

Run negative peers while a healthy viewer is active; assert offender-only close
and unchanged runtime/hub/viewer/owner state.

### Smoke And E2E

Build all targets, run bounded local campaigns, execute Linux sanitizer jobs,
then rerun normal protocol/gateway/browser behavior.

### Coverage And Quality

Require fuzz target compilation, corpus replay in the normal floor, unchanged
coverage/security limits, Rust format/clippy/tests, TypeScript tests/type/build,
repository validation, and `git diff --check`.

## Manual Test Sequence

1. Build all four targets and replay checked corpora.
2. Run each target locally for at least 60 seconds with bounds.
3. Run TypeScript deterministic mutation/malformed tests.
4. Run gateway negative integration beside a healthy viewer.
5. Run Linux sanitizer fuzzing for at least ten minutes per target.
6. Re-run all normal protocol/gateway/browser baselines.

## Documentation And Claim Impact

Evidence supports bounded parser-resilience for named targets and revisions,
not formal safety, exhaustive security, Production, or arbitrary-client claims.

## Definition Of Done

- Four targets build/run and normal tests replay minimized corpus entries.
- Local and Linux sanitizer durations/results are recorded and clean.
- Cross-language deterministic negative outcomes agree.
- Healthy-session isolation and no-side-effect assertions pass.
- Issue, plan, validation commands, risks, and claim wording agree.

## Post-Implementation Smoke Sequence

1. Replay all Rust/TypeScript malformed corpora.
2. Run four bounded local fuzz campaigns.
3. Run four recorded Linux sanitizer campaigns.
4. Run gateway healthy-viewer isolation tests.
5. Run normal protocol/gateway/browser checks and repository validation.

## Evidence Record

Record PR/commit, target/corpus versions, exact bounded commands, durations,
sanitizer environment, crashes or zero result, minimized regression links,
coverage/quality results, defect decisions, and residual risk.
