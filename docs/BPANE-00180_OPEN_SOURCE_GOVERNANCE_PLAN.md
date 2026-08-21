# BPANE-00180 Open-Source License And Contribution Governance Plan

## Metadata

- Issue: `#180`
- State: Qualified
- Owner: BrowserPane maintainers; the accountable legal/business approver is a
  required external input and is not yet named
- Lane: Foundation
- Target gate: Phase 0 external-Pilot governance and Production Baseline
- Depends on: a dated, reviewed license/contribution decision; `#151` provides
  the existing CI and dependency-safety baseline
- Coordinates with: `#72` for technical security hardening and `#75` for SBOM,
  signing, provenance, and release promotion
- Last verified commit/date: `b7d7bddd4ce8` / 2026-08-21

## Business Outcome

A prospective operator or contributor can determine one authoritative
BrowserPane license and contribution posture from repository and distribution
evidence without reconciling contradictory metadata or guessing at ownership,
security-reporting, trademark, or sign-off rules.

The repository engineering contract is specified here, but it does not select
the legal posture. The reviewed decision and its accountable approver remain a
fail-closed prerequisite. Until approval and repository alignment are both
recorded, an external Pilot must not rely on BrowserPane's open-source or
contribution posture.

## Example Use Case

A design partner evaluates a bounded BrowserPane Pilot, wants to deploy the
container images, and proposes a security fix. The root license, Cargo and Node
metadata, image labels, included notices, contribution instructions, private
security-reporting path, conduct rules, and review ownership all agree with the
reviewed decision. A repository check rejects a missing or contradictory
identifier before merge, and the partner can follow the contribution path
without treating a README claim as legal advice.

## Current Evidence

- Root `LICENSE` contains the GNU Affero General Public License version 3 text,
  and GitHub currently detects the repository as `AGPL-3.0`.
- Root `Cargo.toml` declares `license = "MIT"` in `[workspace.package]`; all
  seven Rust crates inherit that value.
- Nine tracked first-party `package.json` files omit a top-level `license`
  field. Eight corresponding npm lockfiles and `Cargo.lock` are the current
  committed third-party dependency inputs.
- Eleven tracked Dockerfiles build BrowserPane or BrowserPane-owned fixture
  images. Only `deploy/Dockerfile.ci-rust` currently declares an OCI license
  label, using `AGPL-3.0-only`; the other Dockerfiles do not declare their own
  aligned BrowserPane license label or notice inclusion.
- No tracked `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`,
  `CODEOWNERS`, third-party notice, trademark, or general governance file
  exists. GitHub reports no contribution or conduct file, repository web
  commit sign-off is disabled, and branch protection does not require code
  owner review.
- `scripts/check-dependency-safety.mjs` checks vulnerabilities in Cargo and npm
  lockfiles, but no current command inventories dependency licenses, evaluates
  license policy, or verifies first-party metadata and distributed notices.
- `scripts/check-repository-documents.mjs` checks Markdown links, YAML, and
  workflow policy; it does not require or validate contributor-governance
  files.
- ADR 0004 is Proposed. It treats the root license as the current public
  posture while requiring an explicit reviewed decision before metadata
  alignment, external contribution promotion, dual licensing, or a commercial
  exception.
- `R-014` remains a High/Critical Phase 0 and Production risk. The release-gate
  and maturity documents do not permit a Pilot or Production claim from the
  current inconsistent state.

## Scope

1. Record the reviewed decision in ADR 0004 or a superseding canonical
   decision. It must identify the exact SPDX expression, covered and excluded
   components, whether the grant is `-only` or `-or-later`, treatment of prior
   distributions and contributions, the contribution model, source-header
   policy, third-party compatibility policy, trademark/name policy, and any
   intentionally selected commercial exception or dual-license terms.
2. Align every tracked first-party licensing surface with that decision:
   root license files, Cargo workspace/crates, Node manifests and lockfile root
   metadata, final container-image labels, source-header policy, build/package
   metadata, and the declared distribution inventory.
3. Generate deterministic human-readable third-party notices and a
   machine-readable license inventory from `Cargo.lock` and every committed npm
   lockfile. Fail on unknown, malformed, incompatible, missing, or newly
   unreviewed results; any exception must be narrow, accountable, dated, and
   expiring.
4. Include the authoritative license and required third-party notices in every
   declared distributable package or final image, and add one canonical local
   validation command to the required fast CI floor.
5. Add contributor-facing policy for contribution setup and review, the
   selected DCO/CLA/other sign-off mechanism, private vulnerability reporting,
   conduct, review ownership, and project-name/trademark use. Configure the
   selected GitHub enforcement where repository files alone cannot enforce the
   reviewed contribution model.
6. Synchronize repository, operator, website, and investor-facing claims with
   the exact approved posture and its limits. External repositories or hosted
   settings require separately authorized evidence; they are not silently
   mutated by the implementation PR.

## Non-Goals

- Selecting a license, DCO, CLA, trademark grant, commercial exception, or
  dual-license model in this plan or by inference from the current mismatch.
- Relicensing past copies or third-party code by changing metadata.
- Providing legal advice or publishing confidential review material.
- Completing the `#72` threat model/hardening backlog or the `#75` SBOM,
  signing, provenance, versioning, release-channel, and release-promotion
  program.
- Changing BrowserPane runtime behavior, API, protocol, persistence, UI,
  product CLI/SDK behavior, deployment topology, or supported capacity.
- Adding per-file license headers through a repository-wide mechanical rewrite
  unless the reviewed source-header policy explicitly requires them.
- Claiming Pilot-ready, Production-ready, enterprise-ready, supported
  commercial licensing, or trademark permission from repository consistency
  alone.

## Decisions And Dependencies

### Approval and engineering gates

The work has three ordered gates:

1. **Reviewed-decision input:** an accountable maintainer and appropriate
   legal/business reviewer record the exact choices listed in Scope item 1,
   plus the decision date, approval evidence, and next review date. The public
   record may link to approval without exposing confidential advice. Until this
   exists, implementation must not change the effective license or enable a
   contribution model by assumption.
2. **Repository engineering:** align first-party metadata and distributions,
   add governance files and enforcement, generate notices, and make controlled
   failures part of required CI. A changed decision returns to Gate 1; it is
   not patched around with inconsistent exceptions.
3. **External-use acceptance:** verify public/operator/investor claims and the
   intended distribution against the approved result. Only then may the Phase
   0 gate treat `#180` as satisfied for an external Pilot. Production still
   requires the independent Production Baseline owners.

The issue and this plan remain Qualified while Gate 1 is absent. This
specification PR does not satisfy or simulate legal approval.

### Phase 0 and Production gate effect

- **Phase 0:** `#180` passes for external-Pilot reliance only after the reviewed
  decision, repository/distribution alignment, contribution and disclosure
  path, artifact smoke, and public-claim review are recorded. It does not
  select or accept the real activity, deployment, owners, or data/threat
  profile owned by `#174`.
- **Production Baseline:** the same consistent governance is necessary but not
  sufficient. Production also requires the independent security, release,
  compatibility, recovery, observability, and named-deployment evidence in the
  Production gate, including unresolved `#72` and `#75` scope.

### Ownership boundaries

- `#180` owns the authoritative project-license/contribution decision,
  first-party metadata, third-party license notices and compatibility policy,
  `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, `CODEOWNERS`, sign-off
  selection, ownership/governance references, trademark/name rules, and claim
  consistency.
- `#72` owns product threat modeling, security controls, hardening, deployment
  assumptions, and technical remediation evidence. `#180` owns only the public
  private-reporting and coordinated-disclosure entry point; it must route
  reports without claiming that `#72` is complete.
- `#75` owns SBOM creation, artifact/image signing, provenance/attestation,
  dependency and image vulnerability release policy, release channels,
  compatibility bundles, and release promotion/rollback. `#180` owns license
  inventory/notices and makes them consumable by `#75`; it does not call a
  notice file an SBOM or signature.
- `#151` already owns the required fast validation and vulnerability-scanning
  baseline. `#180` extends that floor with license-governance validation rather
  than replacing or duplicating the existing advisory scanner.
- `#174` remains externally deferred. It may consume `#180` only after the
  external-use acceptance gate passes; `#180` does not select its activity,
  owners, deployment, or data/threat profile.

## Contract Changes

- API/OpenAPI: N/A; no control-plane operation, schema, status, or error changes.
- Protocol/event schemas: N/A; no browser transport or event contract changes.
- Database/migrations: N/A; no persisted product resource changes.
- Admin-New: N/A; no dedicated license or governance route is required.
- CLI/SDK: N/A for product behavior and commands. First-party package metadata
  is aligned, but no new SDK surface or compatibility promise is created.
- Deployment/configuration: N/A for runtime topology, environment variables,
  ports, and Compose behavior. Final image metadata and included license/notice
  files are distribution evidence and must be validated.
- README/ARCH/AGENTS/operator docs: update `README.md` and the relevant
  contributor/release/operator references with the approved posture and
  validation command. `ARCH.md`, OpenAPI, and runtime operations documents are
  N/A unless implementation changes their existing claim or file map.
- Canonical governance docs: update ADR 0004, capability maturity, `R-014`,
  release-gate evidence, roadmap/current context, validation matrix, and issue
  context only when the corresponding decision or implementation evidence
  actually changes.

## Security And Data Impact

- `SECURITY.md` must point to a selected private reporting mechanism and tell
  reporters not to include vulnerability details, credentials, customer data,
  or exploit material in public issues. The smoke uses a harmless test inquiry,
  not a real vulnerability.
- The repository stores the public decision and approval evidence, not legal
  advice, private security reports, secrets, personal access tokens, or target
  credentials.
- A license inventory may expose only already-public package names, versions,
  sources, license identifiers, and notice text. It must not inspect or emit
  BrowserPane runtime data, browser content, identity claims, or local paths.
- `CODEOWNERS` is a review-routing mechanism, not authorization or an ownership
  transfer. Required-review or sign-off settings must match the selected
  contribution policy and be verified separately.
- Unknown dependency licenses and incompatible policy results fail closed.
  Exceptions require reason, scope, accountable owner, approval reference,
  expiry, and a test proving stale/expired exceptions fail.
- Security-report handling and retention remain outside product storage. The
  published disclosure policy must state the selected channel's handling and
  response expectations without pretending BrowserPane enforces them in code.

## Migration, Compatibility, And Rollback

- Runtime, API, protocol, database, Admin-New, and product CLI behavior remain
  compatible; no feature flag or data migration is required.
- License/package metadata is externally visible. The implementation must
  enumerate prior releases and distributions affected by the decision and must
  not claim that a new identifier retroactively changes rights already granted
  or licenses third-party material.
- Apply the reviewed decision atomically across root, Rust, Node, container,
  source-header-policy, notice, and documentation surfaces. Do not merge a
  temporary state where only one ecosystem advertises the new posture.
- Before an approved distribution is published, rollback is a normal revert of
  the coherent metadata/governance change plus any GitHub policy setting, with
  the decision history retained. After distribution, do not delete or rewrite
  history; record a superseding reviewed decision and forward-fix future
  metadata/notices. Any correction to already published artifacts belongs to
  the release process under `#75`.
- Contribution records accepted under a selected DCO/CLA/other model remain an
  audit trail. Rollback must not erase attestations or imply a transfer that did
  not occur.

## Observability, Failure Behavior, And Operator Feedback

The canonical license-governance check must return nonzero with an actionable,
path-scoped diagnostic when it finds:

- no approved decision record or an invalid/ambiguous SPDX expression;
- root, Cargo, Node, source-header, container, or distribution metadata that
  is missing or contradicts the decision;
- an unclassified tracked first-party manifest or distributable image;
- an unknown, malformed, incompatible, or unreviewed third-party license;
- stale generated inventory/notices or a distributable artifact missing the
  required license/notice payload;
- a missing governance file or sign-off/ownership configuration required by
  the selected policy; or
- an absent, overbroad, unowned, or expired exception.

The check reports bounded counts and repository-relative paths. It must not log
private legal material, security-report content, credentials, home-directory
paths, or full environment values. There are no runtime metrics, traces,
health/readiness changes, alerts, or Admin-New messages for this governance
slice; required CI and repository documentation are the operator feedback.

## Implementation Slices

1. Before starting implementation, confirm Gate 1 is supplied and stop if any
   required choice remains unresolved. Mark ADR 0004 Accepted or superseded
   only in the implementation that carries the approval evidence.
2. Add the focused, deterministic license-governance checker, controlled
   fixtures, declared first-party/distribution inventory, third-party license
   inventory/notices, and exception schema/policy.
3. Align root, Rust, Node, source-header policy, container labels, and
   distributed license/notice payloads in one reviewable metadata slice.
4. Add contribution, sign-off, security disclosure, conduct, ownership,
   trademark/name, and governance documentation plus selected GitHub
   enforcement evidence.
5. Add the check to the fast validation floor and synchronize repository,
   operator, website, and investor claims without expanding `#72` or `#75`.
6. Run the complete automated and manual evidence sequence, update maturity and
   `R-014` only to the level supported, and record the Phase 0/Production gate
   disposition.

Slices 2-5 must not merge as a claimed solution before Slice 1 is accepted.
If review changes the decision, regenerate and revalidate the complete
coherent set rather than preserving stale partial output.

## Acceptance Criteria

- One dated, reviewed decision supplies every choice required by Gate 1 and is
  represented by an Accepted or superseding ADR without exposing confidential
  advice.
- One maintained inventory proves that every tracked first-party manifest,
  final image, source-header policy, and distributable artifact is classified
  and agrees with the decision.
- The root license, all eight Cargo manifests, all nine Node manifests, all
  eleven Dockerfiles, generated notices, and declared artifact payloads pass
  the canonical consistency check with no unknown or unapproved result.
- Contribution, sign-off, conduct, ownership, private security disclosure, and
  trademark/name policies are present, mutually consistent, and enforce the
  reviewed choices without implying authorization or ownership transfer.
- Deterministic third-party inventory/notices cover `Cargo.lock` and all eight
  npm lockfiles; controlled incompatible, unknown, stale, missing, and expired
  cases fail closed.
- The existing vulnerability check and `#151` validation floor remain intact;
  the new required license-governance stage passes locally and in CI.
- Every declared distributable artifact exposes aligned metadata and contains
  the required license and notice payload when inspected after a real build.
- Compatibility, prior-distribution limits, revert-before-publication, and
  forward-fix-after-publication behavior are recorded and tested where
  mechanically testable.
- Repository/operator/website/investor wording states one approved posture and
  the exact claim limits; unavailable external evidence keeps the external-use
  gate blocked rather than being assumed.
- The final evidence explicitly leaves `#72` and `#75` open and records separate
  Phase 0 external-use and Production Baseline dispositions.

## Test Strategy

### Unit

- Parse and validate the approved decision schema, exact SPDX expression,
  component coverage, contribution choice, source-header policy, trademark
  posture, approval reference, owner role, decision date, and review date.
- Use isolated fixtures for root-license mismatch, inherited Cargo metadata,
  missing and contradictory Node root metadata, final-image OCI labels,
  allowed/forbidden source headers, unclassified distributions, and missing
  artifact notice payloads.
- Cover dependency license normalization, multiple SPDX expressions, required
  notice text, unknown/malformed/incompatible results, narrow approved
  exceptions, and stale/expired exception rejection.
- Cover every controlled failure listed above and prove diagnostics contain
  repository-relative paths without private input or absolute-path leakage.

### Integration

- Run one checker against the actual root license, all eight Cargo manifests,
  all nine Node manifests, all eight committed npm lockfiles, `Cargo.lock`, and
  all eleven tracked Dockerfiles through a maintained declared inventory.
- Regenerate license inventory/notices in a temporary directory and compare
  them byte-for-byte with committed canonical output; a second run must be
  identical.
- Exercise the repository-document and baseline contracts so required
  governance files, the new command, generated evidence, and workflow stage
  cannot drift.
- Verify the existing dependency-vulnerability policy still runs independently
  and that license exceptions cannot suppress security advisories.

### Validation Errors And Regression

- In controlled temporary fixtures, remove each required governance file,
  alter each ecosystem's identifier, add an unclassified manifest/image, make
  notices stale, add an unknown license, and expire an exception; each case
  must fail for its own reason.
- Preserve current Markdown/YAML/workflow validation, dependency safety, Rust
  metadata resolution, Node lockfile parsing, and OCI CI-builder label checks.
- API/OpenAPI, protocol, database, Admin-New, CLI behavior, runtime Compose,
  and browser-session regression suites are N/A because those contracts do not
  change.

### Smoke And E2E

- Build every currently declared distributable first-party image/package using
  the repository-owned license-artifact smoke and inspect the resulting
  metadata and filesystem for the approved license and required notices.
- Validate the selected sign-off path from a clean fork/branch through review
  routing without merging the smoke change.
- Follow the harmless private-security-reporting test path without submitting a
  vulnerability or sensitive value.
- Cross-check repository, operator, website, and investor statements against
  the exact approved decision and explicit claim limits.
- No live BrowserPane session, gateway/Postgres store, browser transport,
  Admin-New, or product CLI smoke is required.

### Coverage And Quality

- Run the focused Node test suite for the new checker with success, boundary,
  validation-error, redaction, and failure branches represented in the
  recorded coverage report; no new decision branch may remain untested.
- Run `node scripts/validate.mjs --stage validation-tool-tests --stage
  repository-baseline --stage repository-documents --stage dependency-safety`
  plus the new license-governance stage.
- Run `git diff --check` and review the final generated/notices diff for
  determinism, supported SPDX expressions, accidental legal claims, absolute
  paths, and unrelated source-header churn.
- Broad Rust, browser, Admin-New, gateway, OpenAPI, and Compose builds are not
  substitutes for this evidence and are not required unless the implementation
  expands beyond this contract.

## Manual Test Sequence

1. On a clean checkout, inspect the public decision record and confirm it names
   the exact SPDX/component, contribution, source-header, notice, trademark,
   owner-role, approval, decision-date, and review-date choices without
   publishing confidential advice.
2. Run the canonical license-governance check and regenerate the third-party
   inventory/notices to a temporary directory; expect no diff and no unknown or
   unapproved result.
3. Run the controlled negative fixtures and confirm a missing identifier,
   incompatible dependency, stale notice, and expired exception each fail with
   an actionable repository-relative diagnostic.
4. Build the declared distributable images/packages and inspect each final
   artifact for the approved metadata plus required license and notice files.
5. From a clean fork or disposable branch, follow `CONTRIBUTING.md`, exercise
   the selected sign-off mechanism, and confirm review routes to the declared
   owners. Do not merge the test change.
6. Follow `SECURITY.md` with a harmless test inquiry, confirm the path is
   private and contains no real vulnerability or credential, then clean up the
   test according to the published procedure.
7. Compare README/operator, website, and investor wording with the approved
   posture and verify it does not imply Production readiness, a commercial
   exception, trademark permission, or completion of `#72`/`#75`.
8. Record evidence links, remove only the disposable smoke artifacts, and leave
   decision, approval, contribution, and issue history intact.

## Documentation And Claim Impact

- Before approval and alignment, the only supported claim is that the root
  license is the current public posture and metadata/governance alignment is
  pending. The mismatch remains `R-014` and an external-Pilot blocker.
- After approval, implementation, and artifact validation, documentation may
  state the exact reviewed license and contribution posture. It must still not
  imply signed/provenanced releases, complete threat hardening, Production
  readiness, broad Pilot acceptance, trademark permission beyond the recorded
  policy, or a commercial/dual-license option that was not selected.
- Mark ADR 0004 Accepted or superseded only with the reviewed decision. Update
  capability maturity, risk, roadmap, current context, release-gate evidence,
  validation documentation, and the live issue from actual merged evidence.
- Investor material in the sibling repository and hosted website/settings are
  separate change surfaces. Record their authorized update/evidence or keep the
  external-use gate blocked; do not claim they changed from this repository PR.

## Definition Of Done

- A dated, reviewed decision with accountable approval and next review date
  establishes the exact license/component, contribution, source-header,
  third-party compatibility, trademark, and commercial/dual-license posture.
- Root, Rust, Node, container, distribution, source-header-policy, and public
  documentation surfaces state that posture without contradiction.
- Deterministic third-party inventory/notices cover every committed Cargo/npm
  lockfile, fail closed on unknown/incompatible results, and ship with every
  declared distributable artifact.
- Contribution, sign-off, private security disclosure, conduct, ownership, and
  trademark/name policies are documented and selected enforcement is verified.
- Focused unit, integration, controlled-failure, artifact, contribution, and
  disclosure smokes pass with coverage and redaction evidence.
- Compatibility and rollback evidence records prior-distribution limits,
  setting rollback, and the forward-fix path without rewriting history.
- README/operator/website/investor claims use the approved bounded wording.
- `#72` and `#75` remain open for their independent scopes; no SBOM, signature,
  provenance, hardening, or Production claim is inferred.
- The issue, plan, ADR, maturity matrix, `R-014`, validation matrix, roadmap,
  and Phase 0/Production gate evidence are synchronized with merged facts.
- The external-use acceptance gate explicitly passes before `#174` or any
  external Pilot relies on BrowserPane's open-source posture.

## Post-Implementation Smoke Sequence

1. Verify the reviewed decision, accountable approval, and dated review record.
2. Run focused license-governance unit/fixture tests and the canonical checker.
3. Regenerate third-party inventory/notices and require a byte-identical result.
4. Run repository baseline, document, dependency-safety, and new CI stages.
5. Build and inspect every declared distributable artifact for aligned
   metadata, license text, and notices.
6. Exercise one controlled inconsistency, unknown license, stale notice, and
   expired exception; verify fail-closed diagnostics, then restore the fixture.
7. Follow the contribution/sign-off and CODEOWNERS review path from a clean
   disposable branch without merging it.
8. Follow the private security-reporting instructions with harmless content.
9. Cross-check repository, operator, website, investor, package, container, and
   distribution claims against the approved posture and claim limits.
10. Record the Phase 0 external-use and Production Baseline dispositions,
    residual `#72`/`#75` work, rollback evidence, and cleanup.

## Evidence Record

Record the decision/approval reference, implementation PR and commit, exact
validation commands and results, generated inventory/notices digest, artifact
inspection results, controlled failure output, contribution/disclosure smoke,
claim-review evidence, exceptions with expiry, rollback result, residual issue
links, and the Phase 0/Production gate decisions. Do not attach confidential
legal advice, real vulnerability details, credentials, or local run logs.
