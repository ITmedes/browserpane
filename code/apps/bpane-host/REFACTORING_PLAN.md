# bpane-host main.rs Refactoring Plan

## Overview

Refactoring `src/main.rs` (3842 lines) into cohesive modules using TDD.
Each implementation file stays under 150-200 lines. Tests in separate files.

**Target:** `main.rs` shrinks to ~100-130 lines (entry point only).

## Steps

| Step | Module | Lines | Status |
|------|--------|-------|--------|
| 1 | `config.rs` | ~150 | Pending |
| 2 | `region.rs` | ~150 | Pending |
| 3 | `video_classify.rs` | ~200 | Pending |
| 4 | `scroll/` (4 files) | ~400 | Pending |
| 5 | `video_region.rs` | ~100 | Pending |
| 6 | `tile_loop.rs` | ~150-200 | Pending |
| 7 | `message_dispatch.rs` | ~150 | Pending |
| 8 | `test_session.rs` | ~100 | Pending |
| 9 | `session.rs` | ~150 | Pending |

## TDD Cycle

1. Create module with type stubs
2. Create test file with tests
3. `cargo test` — Red (fail)
4. Implement — Green (pass)
5. Refactor
6. Verify: `cargo test -p bpane-host && cargo clippy -p bpane-host`

## Verification

```bash
cargo test -p bpane-host
cargo clippy -p bpane-host
cargo test --workspace
```
