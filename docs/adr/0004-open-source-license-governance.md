# ADR 0004: Open-Source License Governance

Status: Proposed

Date: 2026-07-31

Related issues: #75, #180

## Context

The repository root contains AGPL-3.0 license text, the Cargo workspace declares
MIT, Node package manifests omit license metadata, and contribution/security/IP
governance is not yet defined. Investor and operator material currently
describes BrowserPane as AGPL open source.

## Proposed Decision

Treat the root license as the current public posture while #180 obtains an
explicit reviewed decision and aligns all package/distribution metadata.
Select contribution, sign-off, security disclosure, third-party notice,
ownership, and trademark rules before Pilot distribution or external
contribution is promoted.

No engineering PR may silently change the effective license or introduce a
dual-license/commercial exception model without the reviewed decision.

## Consequences

- The license inconsistency remains an explicit Phase 0/Production gate risk.
- Management material may describe the current root license but must disclose
  that metadata/governance alignment is pending.
- #75 owns SBOM/signing/provenance; #180 owns legal/metadata/contribution
  consistency.

## Alternatives Requiring Review

- AGPL-3.0-only project with DCO or CLA.
- Dual-license/commercial exception model.
- Permissive licensing for selected SDK/client components.
- A different unified license after legal and business review.
