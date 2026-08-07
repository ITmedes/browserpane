# BPANE-00148 Browser Context Import Safety Plan

## Metadata

- Issue: `#148`
- State: In Progress
- Lane: Foundation
- Target gate: Foundation Gate and prerequisite for `#160`
- Depends on: merged browser-context clone/export/import API baseline
- Enables: safe admin-new archive import in `#160`
- Last verified: `main` at `a933443e68e106b0c07b51a0505190e8a8fbb256`,
  2026-08-07

## Business Outcome

An authenticated operator can import a BrowserPane browser-context archive
without giving one request unbounded gateway memory, CPU, filesystem, or Docker
volume impact. Valid BrowserPane exports remain portable, while oversized,
malformed, ambiguous, traversal-capable, link-bearing, or expansion-heavy
archives fail before profile data is materialized.

This security boundary is required before `#160` can expose archive upload in
admin-new. Clone and export are not blocked by this work, but import must not be
promoted as an operator feature until this plan is complete.

## Example Use Case

A support team exports an inactive reusable Chromium profile from one
BrowserPane environment and imports it into another. A normal archive creates a
new context that can start a session. A modified archive containing a hardlink,
symlink, parent-path entry, excessive file count, oversized manifest, or highly
expanded profile is rejected with an actionable bounded response and does not
leave a context row or Docker profile volume behind.

## Current Evidence

- `POST /api/v1/browser-contexts/import` currently disables Axum's default body
  limit and extracts the complete request as `Bytes`.
- Authorization occurs inside the handler after request extraction, so body
  buffering is not an explicitly authenticated operation.
- ZIP parsing and nested gzip/tar processing are not isolated from async request
  execution.
- The outer archive allows only `manifest.json` and optional
  `profile.tar.gz`, and the manifest already has a 128 KiB declared-size check.
- The nested profile archive has no compressed-size, uncompressed-size, entry-
  count, or path-length cap.
- Docker import validates tar path strings with `tar -tzf`, but does not reject
  symlink, hardlink, device, or FIFO entry types before extracting as root.
- Runtime import cleanup removes the target volume on extraction failure, and
  the API removes imported data if metadata persistence fails. Rejection before
  runtime import currently creates no context row or profile volume.
- `#160` explicitly requires safe import behavior and is therefore dependent on
  this issue.

## Scope

- Authorize import requests before reading their bodies.
- Replace the disabled body limit with an explicit bounded body read and a
  configurable compressed request limit.
- Add bounded process-level import concurrency and return backpressure instead
  of queueing unbounded archive work.
- Move ZIP, manifest, and profile preflight parsing to `spawn_blocking`.
- Enforce outer ZIP entry, manifest, compressed profile, uncompressed profile,
  tar entry-count, and tar path-length limits.
- Reject absolute paths, parent traversal, symlinks, hardlinks, devices, FIFOs,
  and other unsupported tar entry types before Docker volume creation.
- Keep valid BrowserPane format-version 1 archives compatible.
- Preserve cleanup guarantees for runtime extraction and metadata persistence
  failures.
- Return stable JSON errors for malformed input, payload limits, and import
  concurrency backpressure.
- Update OpenAPI, runtime configuration, operator documentation, and validation
  evidence for the new limits.

## Non-Goals

- Admin-new clone/export/import controls remain owned by `#160`.
- Cross-owner, cross-tenant, or overwrite-in-place imports are not introduced.
- Arbitrary ZIP or browser-vendor profile formats are not supported.
- Malware scanning and content inspection are deployment integrations, not part
  of archive-structure validation.
- Browser profile semantic migration across incompatible Chromium versions is
  not guaranteed.
- The existing export archive format is not revised beyond documenting enforced
  import limits and response behavior.

## Decisions And Limits

- Parse archives with established Rust crates (`zip`, `flate2`, and `tar`), not
  shell-output parsing or custom compression code.
- Read the request body only after owner authentication using Axum's bounded
  body API.
- Use typed, validated gateway configuration for:
  - maximum compressed import request bytes,
  - maximum nested compressed profile bytes,
  - maximum uncompressed profile bytes,
  - maximum profile tar entries,
  - maximum concurrent imports.
- Defaults should be conservative for the self-hosted pilot profile while
  remaining configurable for larger deployments. Configuration validation must
  reject zero limits and a nested compressed limit larger than the outer body
  limit.
- The manifest limit remains 128 KiB and the outer archive remains restricted to
  exactly one manifest plus at most one profile payload.
- Return `413 Payload Too Large` for request or archive size limits and `429 Too
  Many Requests` when the bounded import worker capacity is occupied. Structural
  and format violations remain `400 Bad Request`.
- Acquire the import permit after authentication and before body buffering.
- Preflight the exact profile bytes later passed to the runtime manager, so the
  validated and materialized payload cannot diverge.
- Reject link and special-file entries rather than trying to normalize them.

## Contract Changes

- API/OpenAPI: document bounded import behavior and `413`/`429` responses on
  `importBrowserContext`; operation shape and successful resource response stay
  unchanged.
- Protocol/event schemas: N/A.
- Database/migrations: N/A.
- Gateway CLI/config: add validated browser-context import limits and concurrency
  settings with compose defaults.
- Runtime manager: accept only a preflighted profile archive; keep volume cleanup
  on any extraction failure.
- Admin-new: N/A in this prerequisite slice.
- CLI: existing import command receives clearer bounded server errors; command
  syntax is unchanged.

## Security And Data Impact

- Authentication precedes archive allocation and decompression.
- Limits bound compressed input, decompressed bytes, archive entries, path
  lengths, and concurrent work independently; no single declared field is
  trusted as the only bound.
- Archive paths are validated component by component and special entry types are
  denied before Docker execution.
- Parsing runs on the blocking pool so malformed compression cannot stall async
  request handling.
- Error responses identify the violated class and configured limit without
  echoing archive contents, local paths, owner data, or profile metadata.
- Rejected imports must not create a session-control resource or Docker volume.
- Logs should record safe limit category and outcome, not filenames from an
  untrusted archive beyond sanitized fixed entry names.

## Migration, Compatibility, And Rollback

- Existing valid BrowserPane format-version 1 exports within configured limits
  remain importable.
- Archives containing link or special-file entries become intentionally invalid.
- Operators with legitimate larger profiles can raise validated deployment
  limits deliberately; the defaults must be documented.
- Rollback restores previous request behavior but is not recommended once the
  admin-new import UI is exposed.
- No persisted resource migration is required.

## Observability And Operator Feedback

- Emit one structured import outcome at the API boundary with result category
  and safe byte/entry counts.
- Distinguish authentication, capacity, request-size, structure, profile-
  preflight, runtime materialization, and metadata-persistence failures.
- Keep 413 and 429 responses machine-detectable for CLI and future admin-new
  feedback.
- Do not log raw manifests, profile contents, archive-provided paths, or owner
  bearer data.

## Implementation Slices

1. **Archive policy and parser**: extract archive logic from the oversized
   browser-context API module, add typed limits, bounded ZIP reads, nested
   gzip/tar preflight, path/type validation, and deterministic unit fixtures.
   Commit boundary: parser and unit tests.
2. **Authenticated request boundary**: authorize before body read, add bounded
   body handling, concurrency backpressure, blocking parser execution, typed
   400/413/429 mapping, and API integration tests. Commit boundary: secure API
   boundary.
3. **Runtime defense and cleanup**: tighten Docker extraction flags/invariants,
   prove invalid input never launches materialization, and verify volume cleanup
   for runtime and persistence failures. Commit boundary: runtime defense.
4. **Configuration and contract**: expose validated CLI/compose limits, update
   OpenAPI responses/examples and operator docs, and run contract validation.
   Commit boundary: deployable contract.
5. **Battle test and handoff**: gateway unit/integration/coverage, compose API,
   valid export/import/session smoke, malformed fixtures, CLI regression, and
   roadmap/status evidence. Commit boundary: validation and `#160` handoff.

## Test Strategy

### Unit

- Empty, malformed, truncated, and unsupported ZIP archives.
- Missing/duplicate manifest, duplicate/multiple profile, unknown entry, and
  excessive outer entry count.
- Declared and actual manifest-size limits.
- Compressed profile and total request-size limits.
- Uncompressed profile limit at boundary and one byte over.
- Tar entry count and path-length boundaries.
- Absolute paths, parent traversal, symlinks, hardlinks, character/block devices,
  FIFO, and unsupported tar metadata.
- Valid files/directories and BrowserPane export round trip.
- Configuration rejects zero, inconsistent, or overflow-prone limits.

### Integration

- Unauthenticated oversized requests are rejected before archive parsing.
- Authenticated over-limit requests return JSON `413`.
- Saturated import capacity returns JSON `429` without reading/materializing a
  second archive.
- Structural violations return JSON `400` and create neither context metadata
  nor profile data.
- Runtime failure and metadata persistence failure remove target profile data.
- A valid owner-scoped and project-scoped import preserves safe metadata.

### Smoke And E2E

- Export an inactive reusable context, import it, start a reusable session from
  the imported context, and verify expected profile continuity.
- Exercise malformed, traversal, link-bearing, excessive-entry, and over-limit
  fixtures through the compose API.
- Re-run existing browser-context API, CLI, admin compatibility, and admin-new
  browser-context smokes.
- Confirm no rejected fixture leaves a context row, runtime, helper container,
  or Docker profile volume.

### Coverage And Quality

- `cargo fmt --all --check`
- `cargo clippy -p bpane-gateway --all-targets --all-features -- -D warnings`
- `cargo test -p bpane-gateway`
- gateway Rust coverage ratchet
- OpenAPI lint, generated-evidence, conformance, example, and compatibility
  checks
- compose gateway API suites and focused browser-context smoke

## Manual Test Sequence

1. Start compose with docker-pool and authenticate as `demo / demo-demo`.
2. Create an inactive reusable browser context and start/stop one session to
   persist recognizable profile state.
3. Export the context with `./scripts/bpane browser-context export`.
4. Import the untouched archive with a new name and confirm a new ready reusable
   context is returned.
5. Start a session from the imported context and confirm profile continuity.
6. Submit an oversized archive and verify JSON `413` with no resource or volume.
7. Submit malformed, traversal, symlink, hardlink, and excessive-entry fixtures;
   verify bounded JSON `400` responses and no residue.
8. Run concurrent imports above the configured capacity and verify excess work
   receives JSON `429` while the admitted import completes.
9. Re-run clone/export/import with an inactive context and confirm active-writer
   conflict behavior remains unchanged.

## Documentation And Claim Impact

- Update `README.md`, `ARCH.md`, and compose configuration because deployment
  limits and API error behavior change.
- Update OpenAPI and generated evidence in the same slice.
- Update `DELIVERY_ROADMAP.md`, `OPEN_ISSUES_CONTEXT.md`, and
  `RESOURCE_LIFECYCLE_REQUIREMENTS.md` after validation.
- Mark `#160` unblocked for import UI only after this issue is merged.

## Definition Of Done

- Authentication happens before archive body buffering or parsing.
- Request memory, blocking work concurrency, ZIP/profile compressed bytes,
  profile expansion, entry count, and path length are bounded.
- Link, traversal, and special-file payloads fail before runtime materialization.
- Valid BrowserPane exports remain importable and usable by a new session.
- Rejected and failed imports leave no metadata, runtime, helper container, or
  profile volume.
- OpenAPI, deployment defaults, README, ARCH, issue, and roadmap agree.
- Unit, integration, coverage, OpenAPI, CLI, compose smoke, and e2e evidence pass.

## Post-Implementation Smoke Sequence

1. Run focused archive parser and API integration tests for every size, entry,
   path, and type boundary.
2. Run the full gateway test, clippy, formatting, and coverage gates.
3. Run OpenAPI lint/conformance/examples/compatibility validation.
4. Start compose and complete valid export -> import -> session reuse.
5. Submit malformed, oversized, expansion-heavy, traversal, symlink, hardlink,
   special-file, and excessive-entry fixtures.
6. Exercise import concurrency backpressure and verify 429 recovery.
7. Confirm failed fixtures leave no context rows, profile volumes, helper
   containers, or active runtimes.
8. Run existing CLI, compatibility admin, admin-new browser-context, and gateway
   compose API regressions.

## Evidence Record

- PR: pending
- Commits: pending
- Unit/integration results: pending
- Compose smoke results: pending
- Coverage/build results: pending
- README decision: required
- ARCH decision: required
