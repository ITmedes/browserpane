# Rust Standards

These standards apply to Rust code across the BrowserPane workspace:

- `code/apps/bpane-host`
- `code/apps/bpane-gateway`
- `code/shared/bpane-protocol`

Use this file with `AGENTS.md`. If this file and live code disagree, prefer the code, then this file, then other prose.

## Scope

This project contains:

- application crates
- shared protocol/library crates
- performance-sensitive media and transport code

Standards should be applied with that split in mind. Library crates require stricter API discipline than application crates.

## Core Rules

- Keep code explicit and boring over clever.
- Optimize for readable invariants and predictable behavior first, then performance.
- Prefer compile-time guarantees to comments and convention.
- Avoid hidden allocations and silent lossy behavior in protocol, transport, and media paths.
- Minimize public API surface. Public Rust APIs are long-term maintenance costs.

## Code Organization

- Keep Rust source files focused and small. These file-size targets apply to Rust source files, not Markdown or other documentation files.
- Target 150-200 lines per Rust source file for normal production code.
- Files up to 500 lines are acceptable when the module is still cohesive and splitting it would make the code harder to follow.
- Treat files over 500 lines as refactor candidates unless they are generated, test fixture-heavy, or have a documented reason to stay together.
- Keep functions and methods near 50 lines or less.
- Treat 75 lines as the absolute maximum for a function or method; split into named helpers before exceeding it.
- Prefer extracting domain-specific helper types/modules over adding large private helper regions to an already large file.
- Organize substantial test code in separate test modules or files instead of appending large `#[cfg(test)]` blocks to production modules.

## Design Concepts

- Prefer a functional-core, imperative-shell structure: keep side effects at subsystem boundaries and put decisions in deterministic helper functions.
- Keep entry points, session loops, and task runners as orchestration code; move domain decisions into named modules with typed inputs and outputs.
- Centralize raw environment, CLI, filesystem, process, and network reads behind config or adapter modules. The rest of the crate should consume typed, validated values.
- Model OS/runtime backends behind traits or narrow adapter structs so production code and tests use the same boundary.
- Encapsulate mutable subsystem state in named structs with methods instead of long closures with many local variables.
- For complex pipelines, split phases into sibling modules and keep the top-level loop readable as a sequence of named phases.
- Put heuristics and policy decisions in pure functions with explicit constants and focused tests.
- Use bounded channels and document ownership, shutdown, cancellation, and backpressure for long-lived tasks.
- Keep protocol decode/encode, validation, dispatch, and business logic separate unless the protocol type itself owns the behavior.
- Treat large state structs and broad orchestration modules as temporary waypoints; keep extracting cohesive sub-state and helper modules as behavior grows.

## Validation Baseline

Rust changes should normally pass the narrowest relevant set of:

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test -p <crate>`
- `cargo test --workspace`

For crates that support `no_std` or optional features, also run:

- `cargo test -p <crate> --no-default-features`
- feature-specific validation for any changed feature combinations

For library crates, keep rustdoc warning-free:

- `RUSTDOCFLAGS='-D warnings' cargo doc -p <crate> --no-deps`

## API Design

- Prefer domain types over raw primitives when the value space is constrained.
  - Use enums, newtypes, or typed bitflags instead of raw `u8`/`u32` fields where practical.
- Prefer standard conversion traits.
  - Use `From`, `TryFrom`, `AsRef`, and `Display` instead of ad hoc conversion methods when the mapping is standard.
- Constructors should enforce invariants.
  - Do not expose public fields if invariants may need to tighten later.
- For public library APIs, decide future-proofing deliberately.
  - Use `#[non_exhaustive]` where callers should not exhaustively match.
  - Avoid committing to public struct fields unless that stability is intended.
- Keep feature flags additive.
  - Do not create mutually exclusive feature sets unless unavoidable and documented.

## Errors And Panics

- Library crates should prefer typed errors over `anyhow`.
- Application crates may use `anyhow` at orchestration boundaries.
- Use `thiserror` for non-trivial error enums in application crates.
- Do not `unwrap` or `expect` in production paths unless the invariant is truly impossible to violate.
- Panic only for programmer errors or impossible internal states, not routine bad input.
- Any public function that can panic must document it.
- Any public fallible function should document its error conditions.

## Protocol And Serialization

- Wire formats must be versioned or evolved intentionally.
- Protect wire compatibility with exact byte fixtures, not just round-trip tests.
- Test malformed input explicitly:
  - truncated payloads
  - unknown tags
  - oversized lengths
  - trailing data
  - invalid field combinations
- Prefer zero-copy and bounded allocation on hot decode paths.
- Validate declared lengths before allocating or copying.
- Keep framing and parsing logic centralized instead of duplicated across callers.

## Testing

- Unit tests should cover local logic and edge cases.
- Integration tests should cover realistic end-to-end flows between subsystems.
- Property tests are useful for codecs, framing, and message round-trips.
- Round-trip tests are necessary but not sufficient for protocol crates.
- Add regression tests for bugs before or alongside fixes.
- Keep tests deterministic unless randomness is the thing under test.

## Documentation

- Public library crates should have crate-level rustdoc.
- Public APIs should document:
  - purpose
  - important invariants
  - `# Errors` when fallible
  - `# Panics` when panicking is possible
- Keep examples short and runnable when practical.
- Document compatibility promises for shared crates and wire formats.

## Dependencies And Cargo

- Prefer small, well-established dependencies.
- Add dependencies only when the maintenance cost is justified.
- Avoid duplicate dependencies for overlapping purposes.
- Set and maintain `rust-version` for crates that are intended to be stable and reusable.
- Keep `Cargo.toml` metadata complete for reusable crates:
  - `description`
  - `repository`
  - `readme`
  - `keywords`
  - `categories`

## Performance And Memory

- Measure before optimizing broad code paths.
- In hot loops, avoid unnecessary allocation, copying, and temporary buffers.
- Watch enum layout and large variant imbalance in frequently moved types.
- Prefer borrowing and slicing where ownership is not required.
- Be explicit when trading readability for performance, and comment the reason.

## Concurrency And Async

- Prefer ownership transfer and channels over shared mutable state.
- Keep lock scopes tight and avoid holding locks across `.await`.
- Use cancellation-safe patterns for long-lived tasks.
- Surface backpressure explicitly in transport and media pipelines.

## Logging And Observability

- Use `tracing` for application crates.
- Log at subsystem boundaries and on degraded/fallback behavior.
- Avoid noisy per-packet/per-frame logs in hot paths unless gated behind debug-level diagnostics.

## Unsafe Code

- Avoid `unsafe` unless there is no practical safe alternative.
- Any `unsafe` block must state the invariant that makes it sound.
- Prefer narrow `unsafe` blocks over `unsafe fn`.

## BrowserPane-Specific Guidance

- `bpane-protocol` is not just internal plumbing; treat it as a contract.
- Host and gateway frame parsing should share protocol crate logic where possible.
- Media paths should prefer bounded memory growth and explicit size limits.
- Cross-language protocol behavior between Rust and the browser client must be verified with shared fixtures when the wire format changes.

## Review Checklist

Before merging Rust changes, check:

- Are invariants enforced by types instead of comments?
- Is the public API smaller and clearer, not larger and looser?
- Are error paths typed and documented?
- Are panics justified and documented?
- Are protocol changes covered by exact wire tests?
- Did validation include the relevant feature set and crate scope?
- Will this be easy for the next engineer to change correctly?
