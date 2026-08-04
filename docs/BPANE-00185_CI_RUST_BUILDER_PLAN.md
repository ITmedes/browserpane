# BPANE-00185 CI Rust Builder Plan

## Metadata

- Issue: [#185](https://github.com/ITmedes/browserpane/issues/185)
- State: In Progress
- Owner: `thebackplane`
- Lane: Foundation
- Target gate: Foundation Gate
- Depends on: #184 compose-validation lane sharding
- Last verified: 2026-08-04 on `fix/BPANE-00185-ci-rust-builder-publish`

## Business Outcome

Reduce trusted hosted build and test feedback time by maintaining a
deterministic BrowserPane Rust builder image in GitHub Container Registry
(GHCR). The image should remove repeated toolchain, native package, registry,
and third-party dependency compilation from the gateway and host Docker builds
without changing the final runtime images or making local development depend on
registry availability.

## Example Use Case

A gateway source change reaches `main` without changing the Rust toolchain,
Cargo lockfile, manifests, native build dependencies, target architecture, or
release profile. Each compose-validation lane pulls the same content-keyed
builder from GHCR, resolves it to an immutable digest, recompiles only the local
BrowserPane crates, and runs the unchanged API and browser smoke suites. If the
builder package is unavailable, the Dockerfiles safely fall back to the current
Ubuntu-based cold build instead of skipping validation.

## Measured Baseline

Green hosted run
[30843583746](https://github.com/ITmedes/browserpane/actions/runs/30843583746)
completed in 20 minutes 50 seconds. Its browser lane spent 8 minutes 25 seconds
preparing the cold compose stack and 11 minutes 34 seconds running the selected
smokes. Host-side Cargo caches do not feed the isolated Docker builders.

## Scope

- Add one Linux `amd64` Rust builder image for gateway and host release builds.
- Pin the Ubuntu base digest and Rust toolchain, declare the native build
  dependency set, and freeze the resulting build environment behind the first
  immutable content-tag publication.
- Pre-fetch Cargo inputs and precompile the locked third-party release
  dependency graph using source-independent local crate stubs.
- Publish the image to `ghcr.io/itmedes/browserpane-ci-rust` under a
  deterministic tag derived from every material builder input.
- Attach OCI source, revision, description, and version labels plus BuildKit
  provenance/SBOM metadata where the hosted builder supports it.
- Publish only from trusted `main` pushes or explicit workflow dispatches with
  job-scoped `packages: write`; pull requests may build but must not publish.
- Resolve a pulled content tag to its repository digest before compose consumes
  it.
- Preserve an unauthenticated Ubuntu cold-build fallback for local development
  and cache misses.
- Measure one cold/fallback run and two trusted GHCR-backed runs without
  reducing #184 coverage.

## Non-Goals

- Publish BrowserPane production runtime images.
- Put application source, repository credentials, test credentials, generated
  certificates, or runtime data into the builder package.
- Treat a mutable convenience tag as a trusted build input.
- Share instrumented coverage output with ordinary release builds.
- Replace Cargo's dependency model or add a second package registry.
- Expand compose validation into additional runner lanes.

## Image Contract

- Registry: `ghcr.io/itmedes/browserpane-ci-rust`.
- Platform: `linux/amd64`, matching GitHub-hosted compose runners.
- Base: Ubuntu 24.04.
- Toolchain: `rust-toolchain.toml`, currently Rust `1.93.1` with the minimal
  profile plus `clippy`, `llvm-tools-preview`, and `rustfmt`.
- Native dependencies: the union required to compile the gateway and host.
- Dependency seed: root and crate manifests, `Cargo.lock`, and local crate
  stubs only. BrowserPane implementation source is not copied into the image.
- Tag: a human-readable toolchain/platform prefix plus a SHA-256 fingerprint of
  the Dockerfile, toolchain file, lockfile, and all workspace manifests.
- Consumption: workflows pull the content tag and pass its resolved
  `ghcr.io/...@sha256:...` reference into compose builds.
- Fallback: `ubuntu:24.04`, with the existing native package and rustup setup
  retained conditionally in the gateway and host Dockerfiles.

## Security And Supply-Chain Controls

- Keep workflow-level permissions read-only and grant `packages: write` only to
  the publish job.
- Use the repository `GITHUB_TOKEN`; do not add a long-lived package token.
- Do not publish from pull-request jobs, including fork pull requests.
- Keep action references immutable and preserve repository workflow-policy
  validation.
- Never pass the registry token as a Docker build argument or build secret.
- Link the package to this repository with
  `org.opencontainers.image.source=https://github.com/ITmedes/browserpane`.
- Resolve tags to digests before use and report cache misses explicitly.
- Inspect the published filesystem and image history for BrowserPane source,
  credentials, certificates, and unexpected build context.
- A missing or denied image must cause a visible cold fallback, never a skipped
  test. A pulled image with invalid labels, digest metadata, or platform must
  fail closed instead of being trusted or silently replaced.

## Contract Changes

- API/OpenAPI/protocol/database: none.
- Product runtime behavior: none.
- Final host/gateway images: same runtime stages and executable contract.
- Compose: optional `BPANE_RUST_BUILDER_IMAGE` build argument for host and
  gateway; default remains the cold Ubuntu builder.
- GitHub Actions: add a focused builder-package workflow and authenticated,
  read-only builder resolution to trusted compose jobs.
- Local development: existing compose commands remain valid without GHCR
  authentication.

## Implementation Slices

### Slice 1: Deterministic Image Contract

- Add the dedicated builder Dockerfile.
- Add a cross-platform Node helper that fingerprints the complete material
  input set and emits the expected GHCR tag/reference.
- Add unit tests proving stable output, material-input invalidation, lowercase
  registry naming, and rejection of malformed arguments.
- Add an image inspection helper that validates labels, platform, toolchain,
  expected build tools, and absence of repository source/credential paths.

### Slice 2: Trusted GHCR Publication

- Add a path-filtered workflow for pull-request build validation and trusted
  publication.
- Build but do not authenticate or push on pull requests.
- Authenticate with `GITHUB_TOKEN` and job-scoped `packages: write` only for
  `main` pushes and explicit trusted dispatches.
- Publish the deterministic content tag with OCI metadata, provenance, and
  SBOM attestations.
- Treat an already-published content tag as immutable and skip replacement.

### Slice 3: Gateway And Host Consumption

- Add the optional builder-image argument to the host and gateway Dockerfiles
  with their current cold setup retained as fallback.
- Wire the argument through compose without changing runtime service images.
- Add a resolver that authenticates read-only, pulls the expected content tag,
  resolves it to a digest, and exports that digest for later compose steps.
- On package miss or registry outage, emit a warning and leave the argument on
  the cold fallback; reject any successfully pulled image that fails its trust
  or platform contract.
- Apply the resolver to all three #184 lanes.

### Slice 4: Verification And Timing

- Validate the Dockerfile locally for the native architecture and in hosted
  `linux/amd64` pull-request execution.
- Publish the first trusted package after merge or explicit dispatch.
- Inspect its contents, history, labels, SBOM/provenance, and resolved digest.
- Run the complete compose workflow once with a forced fallback and twice with
  the published builder.
- Record per-lane setup, test, and total timing on #185 and in this plan.

## Test Strategy

### Unit

- Fingerprint helper input ordering, content invalidation, path handling, and
  deterministic tag/reference output.
- Resolver decision logic for successful pull, missing package, denied access,
  invalid digest, wrong platform, and command failure.
- Workflow contract tests for paths, permissions, publication guards, immutable
  action references, all #184 lanes, diagnostics, and cleanup.

### Integration

- Build the builder image and compile both real gateway and host sources from
  it.
- Build both services through compose with the image digest argument.
- Build both services without the argument and verify the cold fallback.
- Verify the final runtime images contain the same executable paths and no
  toolchain or builder cache.

### Smoke And E2E

- Run all 16 default gateway API tests.
- Run all four docker-pool gateway API tests.
- Run all eight browser/admin/integration smoke stages.
- Verify redacted diagnostics and cleanup remain independent for each lane.

### Performance

- Preserve the 20-minute-50-second #184 result as the post-sharding baseline.
- Target a trusted warm critical path of 15 minutes or less.
- Report package pull time separately from Docker build and scenario time.
- Reject the optimization if package transfer and maintenance cost do not
  materially improve the critical path.

## Documentation Impact

- Update `README.md` with the optional local builder override and fallback.
- Update the docs workspace and issue map with #185 implementation evidence.
- `ARCH.md` does not require a product architecture change; mention the CI
  package only if build/release architecture is documented there.
- Do not raise product maturity or production-readiness claims based on faster
  validation alone.

## Definition Of Done

- A trusted workflow maintains the deterministic GHCR builder package.
- Pull requests cannot publish or mutate the package.
- Compose consumes a pulled digest when available and cold-builds visibly when
  unavailable.
- Gateway and host runtime images preserve their existing runtime contract and
  do not contain builder content.
- All 20 API scenarios and eight browser/admin/integration stages pass in a
  GHCR-backed hosted run.
- A second trusted run reaches 15 minutes or less, or retained evidence shows
  why the additional mechanism is not justified.
- Issue #185, this plan, README, and PR evidence agree.

## Post-Implementation Smoke Sequence

1. Run fingerprint, resolver, workflow-contract, and repository-policy tests.
2. Build the builder image and run its inspection contract.
3. Build gateway and host with the local cold fallback.
4. Build gateway and host with the content-tagged builder image.
5. Publish from a trusted workflow and verify package/repository linkage.
6. Pull the tag, resolve its digest, and repeat both service builds from that
   digest.
7. Run the complete three-lane compose workflow twice and verify all 28
   selected scenarios/stages.
8. Compare cold, first-pull, and warm timings with run `30843583746`.
9. Trigger one controlled package miss and verify visible fallback, complete
   validation, redacted diagnostics, and cleanup.

## Evidence Record

- Baseline hosted run: `30843583746` (20 minutes 50 seconds)
- Local `linux/amd64` image contract: passed for
  `linux-amd64-rust-1.93.1-292cde4b8093ade4da38d88d`
- Local image size: 2.94 GB uncompressed; 1.08 GB through a Docker-save gzip
  transfer approximation. Hosted GHCR pull size and duration remain pending.
- Local image build under Apple-Silicon emulation: 4 minutes 0 seconds.
- Commit-metadata-only cache probe: 1.6 seconds with every expensive layer
  reused.
- Warm real-source builder stages under emulation: gateway 55 seconds; host 34
  seconds.
- Native Apple-Silicon cold fallback stages: gateway 2 minutes 10 seconds; host
  1 minute 25 seconds. These are portability checks, not hosted-run comparison
  measurements.
- Final runtime contracts: gateway and host executables present; Cargo,
  `/opt/cargo`, and `/build` absent.
- Containerized test contract: `bpane-protocol` passed 124 unit, integration,
  property, wire-fixture, and doc tests through `deploy/Dockerfile.test`.
- Repository fast profile: all 31 stages passed, including Rust tests and
  coverage, admin and browser-client tests and coverage, Node integration
  builds, dependency safety, and repository/workflow policy.
- Local compose replacement was intentionally not run because the developer
  stack contains active long-lived services; the three hosted compose lanes
  remain the non-destructive end-to-end publication gate.
- Published content tag and digest: pending
- Cold/fallback run: pending
- First-pull run: pending
- Warm run: pending
- Implementation PR: [#187](https://github.com/ITmedes/browserpane/pull/187)
- The first trusted publication run
  [30894372465](https://github.com/ITmedes/browserpane/actions/runs/30894372465)
  failed before the image build because the default Buildx `docker` driver does
  not support the requested SBOM and provenance attestations. The corrective
  workflow explicitly bootstraps a `docker-container` builder in both the PR
  validation and trusted publication jobs, and its contract test requires that
  builder before the attested push.
- Corrective publication PR:
  [#188](https://github.com/ITmedes/browserpane/pull/188)
- Corrective hosted builder validation:
  [30895052526](https://github.com/ITmedes/browserpane/actions/runs/30895052526)
  passed in 5 minutes 4 seconds, including Buildx builder bootstrap, the full
  Linux `amd64` image build, image contract inspection, and cleanup.
