# BPANE-00285 Compose Test Image Bundle Plan

## Metadata

- Issue: `#285`
- State: Qualified
- Owner: BrowserPane maintainers
- Lane: Foundation
- Target gate: efficient immutable Compose qualification
- Depends on: `#283`; integration follows `#284`; completed `#184` and `#185`
- Last verified commit/date: `e79164cc3a84` / 2026-08-22

## Business Outcome

Every Compose lane qualifies the same immutable BrowserPane build products, and
the workflow stops paying repeated image-build/setup cost in each parallel job.
The result is faster and more reproducible without relying on mutable tags or a
persistent self-hosted runner.

## Example Use Case

A pull request changes the gateway and Admin-New. One trusted job builds the
required gateway, host, web, MCP, broker, workflow-worker, and recording-worker
images, emits their digests, and every selected lane pulls exactly those images
with source builds disabled. A missing or mismatched image fails before tests.

## Current Evidence

- #184 split Compose into parallel lanes and reduced overall wall time.
- #185 publishes a deterministic GHCR Rust builder, which current jobs resolve
  independently in roughly 35-48 seconds per lane.
- Lanes still prepare overlapping application/worker images and services.
- Existing Compose manifests support explicit image inputs but local developer
  flow still needs a source-build fallback.

## Scope

- Add one trusted build job for the image set selected by the test plan.
- Emit a versioned manifest containing git tree, test-plan/workflow revision,
  logical image name, immutable digest, platform, and provenance reference.
- Make downstream hosted lanes pull and use exact digests with builds disabled.
- Use registry-backed BuildKit cache with branch/fork-safe write permissions.
- Preserve deterministic cold-cache and local source-build paths.
- Measure build, pull, and startup phases against #283 evidence.

## Non-Goals

- No mutable `latest` inputs in qualification.
- No self-hosted persistent runner or cross-job live stack sharing.
- No path selection, test reduction, or post-merge reuse.
- No replacement of the CI Rust builder or production release pipeline.

## Decisions And Dependencies

- The manifest schema reuses #283 identity and artifact constraints.
- #284 isolation must be available before changing all lanes to the shared image
  bundle, so timing changes do not hide state leakage.
- Hosted qualification fails closed if a required image cannot be resolved by
  exact digest.
- Build cache is an acceleration input, never qualification evidence.
- A trusted main/manual build may write shared cache; untrusted contexts are
  read-only or isolated.

## Contract Changes

- API/OpenAPI: N/A.
- Protocol/event schemas: N/A; add an internal versioned image manifest.
- Database/migrations: N/A.
- Admin-new: N/A; its image becomes one bundle entry.
- CLI/SDK: N/A.
- Deployment/configuration: additive Compose image overrides and hosted build
  workflow; local compose defaults remain source-build capable.
- README/ARCH/AGENTS/operator docs: update CI/local build commands if their
  invocation changes; architecture support claims do not change.

## Security And Data Impact

- Registry publication uses least-privilege trusted jobs and immutable digests.
- Manifests and logs must not expose registry credentials, build secrets, OIDC
  tokens, or source credentials.
- Cache scopes prevent untrusted branches from overwriting trusted entries.
- Image provenance, base-image identity, and dependency policy remain enforced.

## Migration, Compatibility, And Rollback

- Introduce manifest production and verification before consumers switch.
- Migrate one lane at a time while comparing image identity and scenario output.
- Retain an explicit local source-build mode; hosted qualification must not
  silently fall back after migration.
- Rollback restores lane-local build preparation and leaves product images and
  deployment contracts unchanged.

## Observability And Operator Feedback

- Record build cache hit/miss, build duration, upload/pull duration, manifest
  digest, per-image digest, and stack-start duration in #283 evidence.
- Errors name the logical image and validation reason without exposing registry
  credentials or internal response bodies.

## Implementation Slices

1. Freeze manifest schema, selected image inventory, and trust/permission model.
2. Add deterministic build job and cold/warm cache behavior.
3. Add strict manifest verifier and Compose digest override generation.
4. Migrate gateway/browser lanes, then Admin-New/compatibility lanes.
5. Remove duplicate hosted builds and record before/after timings.

## Test Strategy

### Unit

- Manifest schema, digest syntax, duplicate/missing entries, tree/test-plan
  binding, platform matching, permission decisions, and override generation.

### Integration

- Warm cache, cold cache, denied cache publication, registry pull failure,
  digest mismatch, unavailable image, and local source-build fallback.

### Smoke And E2E

- All five hosted lanes consume one emitted manifest and retain the complete
  scenario inventory.
- A controlled missing digest fails before Compose starts.

### Coverage And Quality

- Preserve dependency, provenance, immutable-action, and workflow policy checks.
- Add changed-code tests for manifest/build orchestration.
- Compare total compute minutes and critical-path wall time with #283 baseline.

## Manual Test Sequence

1. Run the image-build job from a clean tree with an empty cache.
2. Validate every manifest digest and pull each image by digest.
3. Run the selected Compose lanes with builds disabled.
4. Remove one manifest entry and verify fail-closed preflight.
5. Repeat with a warm cache and compare timing evidence.
6. Run local source-build fallback and verify it remains explicit and functional.

## Documentation And Claim Impact

Update `VALIDATION_MATRIX.md`, contributor CI guidance, and any command examples
that distinguish hosted immutable inputs from local builds. No product maturity
or deployment-support claim changes.

## Definition Of Done

- One trusted job produces all required immutable images and a valid manifest.
- Every hosted lane consumes exact digests from that manifest without rebuilding.
- Cold cache, warm cache, missing image, mismatch, and permission cases pass.
- Local source-build behavior remains documented and tested.
- Timing/compute comparison and one full green hosted run are linked in #285.

## Post-Implementation Smoke Sequence

1. Run manifest and workflow contract tests.
2. Execute one cold-cache image build and validate/pull all digests.
3. Run all hosted Compose lanes with `--no-build` semantics.
4. Exercise missing/mismatched manifest failures.
5. Execute one warm-cache run and compare timings.
6. Verify explicit local source-build fallback and cleanup.

## Evidence Record

Record the PR, commit, manifest artifact, image digests, cold/warm workflow runs,
negative cases, permission review, and timing comparison in issue `#285`.
