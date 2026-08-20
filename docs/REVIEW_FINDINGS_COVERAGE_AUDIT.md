# Review Findings Coverage Audit

Created: 2026-07-07
Revalidated: 2026-08-20

This document is the traceability check between the raw `review/` reports and
the consolidated `docs/` workspace. It complements
`REVIEW_FINDINGS_RECONCILIATION.md`: that file explains the current status and
priority changes, while this file proves that each review family is represented
by a current owner, intentionally superseded, or intentionally deferred.

The corrected `*_V2.md` reports are authoritative when they disagree with the
first-pass reports.

## Coverage Result

All reports in `review/` have been processed. The active findings are reflected
in the consolidated docs, with the highest-priority work owned by
`NEXT_WORKING_ROADMAP.md` and `SECURITY_RUNTIME_ROADMAP.md`. Lower-priority
performance, maintainability, documentation, and enterprise feature gaps are
kept as explicit backlog or guardrail items instead of being mixed into the next
admin implementation slice.

The raw `review/` folder remains source material only. It should not become the
working plan because several high-severity findings were later fixed or
superseded by the current code baseline.

## Source File Coverage

| Source report | Coverage status | Current owner |
| --- | --- | --- |
| `review/00_OVERVIEW.md` | Covered. The cross-cutting themes are mapped to token/browser-auth hardening, admin promotion gates, operations/readiness, ARCH accuracy, and CI/lint guardrails. | `NEXT_WORKING_ROADMAP.md`, `SECURITY_RUNTIME_ROADMAP.md`, `ADMIN_NEW_STATUS.md` |
| `review/00_VERIFICATION.md` | Covered. Workflow-source and preview-symlink findings are superseded; token/log auth findings are implemented by #145. | `REVIEW_FINDINGS_RECONCILIATION.md`, `SECURITY_RUNTIME_ROADMAP.md` |
| `review/SECURITY.md` | Covered through the corrected V2 layer. Still-open H/M/L items are mapped explicitly. | `REVIEW_FINDINGS_RECONCILIATION.md`, `SECURITY_RUNTIME_ROADMAP.md`, `VALIDATION_MATRIX.md` |
| `review/SECURITY_V2.md` | Covered. This is the main security source for the next cleanup slices. | `NEXT_WORKING_ROADMAP.md`, `SECURITY_RUNTIME_ROADMAP.md` |
| `review/PERFORMANCE.md` | Covered. Top findings are mapped to catalog scalability, host/client render backlog, gateway fan-out, archive/export, webhook delivery, and worker reliability. | `REVIEW_FINDINGS_RECONCILIATION.md`, `SECURITY_RUNTIME_ROADMAP.md`, `VALIDATION_MATRIX.md` |
| `review/PERFORMANCE_V2.md` | Covered. Corrections and newly discovered opportunities are retained as explicit backlog, not immediate security gates. | `SECURITY_RUNTIME_ROADMAP.md`, this audit |
| `review/MAINTAINABILITY.md` | Covered. Admin duplication, session-control test debt, CI/lint, smoke helper duplication, and standards drift are retained. | `SECURITY_RUNTIME_ROADMAP.md`, `ADMIN_NEW_IMPLEMENTATION_GUARDRAILS.md`, `VALIDATION_MATRIX.md` |
| `review/MAINTAINABILITY_V2.md` | Covered. Corrected debt counts, ID newtypes, positional params, glob seams, and error strategy are retained as maintainability backlog. | `SECURITY_RUNTIME_ROADMAP.md`, this audit |
| `review/DOCUMENTATION.md` | Covered. OpenAPI examples, gateway config reference, ARCH drift, README split, TS package docs, and contribution/security docs are retained. | `SECURITY_RUNTIME_ROADMAP.md`, `ADMIN_NEW_API_COVERAGE.md`, `SOURCE_PLAN_INVENTORY.md` |
| `review/DOCUMENTATION_V2.md` | Covered. Corrected ARCH capture/cache inaccuracies and admin-app documentation gaps are retained explicitly. | `SECURITY_RUNTIME_ROADMAP.md`, `REVIEW_FINDINGS_RECONCILIATION.md` |
| `review/MISSING_FEATURES.md` | Covered. Enterprise access control, observability, reliability, packaging, SDK, rate-limit, billing, recording, and compliance gaps are retained as backlog. | `NEXT_WORKING_ROADMAP.md`, `SECURITY_RUNTIME_ROADMAP.md`, domain requirement docs |
| `review/MISSING_FEATURES_V2.md` | Covered. Graceful shutdown, dependency-aware readiness, metrics facade, and event-stream scalability are promoted above broad enterprise features. | `SECURITY_RUNTIME_ROADMAP.md`, `VALIDATION_MATRIX.md` |

## Security Traceability

| Finding | Coverage decision |
| --- | --- |
| C1 workflow git RCE | Superseded by current workflow source hardening; retained as completed baseline and regression validation. |
| N1 workflow source-preview symlink read | Superseded by current source preview containment; retained as regression validation. |
| N2 unauthenticated bridge-local `/control-session` takeover | Mostly superseded by bridge control auth; remaining host-exposed MCP transport hardening stays open. |
| N3 token cryptographic confusion | Resolved by #145 with purpose-bound tokens and independent derived signing keys. |
| M3 bearer credentials in URLs/logs | Resolved by #145 with scoped first-frame event authentication and redacted transport targets. |
| H1 webhook SSRF | Open as a focused security/runtime cleanup slice. |
| H2/H3 admin token storage, nonce, ID-token verification | Resolved by #146 with memory-only tokens and certified OIDC validation. |
| M1 browser-context import decompression | Open as browser-context import safety. |
| M2 missing CSP/security headers | Resolved by #146 with shared static and live-validated controls. |
| M4 dev HMAC fallback and token logging | Retained as local-dev caveat plus safer-defaults/documentation work. |
| L1 non-constant-time HMAC | Resolved by #145 through library-backed HMAC verification. |
| L2 tar symlink/hardlink import | Folded into browser-context import safety. |
| L3 workflow-worker entrypoint containment | Treated as defense-in-depth; current source hardening and worker validation should keep regression coverage. |
| L4 legacy singleton routes authorize any principal | Retained as compatibility-surface risk; not a promotion blocker if kept out of production multi-tenant flows. |
| L5 demo password shipped in local auth config | Retained as local-only documentation/security posture caveat. |
| L6 secret zeroization and Vault error body hygiene | Retained as lower-priority secret-handling hardening. |
| N4 log injection via request path | Resolved by #145 through control-character removal and bounded path-only diagnostics. |
| N5 worker writes live token to `context.json` | Retained as worker hygiene backlog. |
| N6 `RwLock` poison cascade | Retained as resilience hardening backlog. |

## Performance Traceability

| Finding family | Coverage decision |
| --- | --- |
| F1/F2, V2 A.1, B5: host capture and classify full-screen/full-grid work | Retained in performance backlog and documentation accuracy requirements. ARCH.md must stop claiming damaged-tile capture. |
| F3/F6/F10, V2 A.2/B1: session, project, workflow-run, identity, and admin-event aggregation | Retained in `Admin And Session Catalog Scalability` with Postgres contract tests and seeded regression requirements. |
| F4/F5/F11, V2 A.3/B8: client decode/cache/GPU/rAF inefficiencies | Retained in client render backlog and ARCH.md cache correction requirements. |
| F7/F12, V2 A.4/B2/B4: per-viewer frame/keyframe encode, full-refresh broadcast, send-stream backpressure | Retained as gateway fan-out/performance backlog. Not required before the next security/admin slice unless touched. |
| F8, V2 A.5/B3: whole-artifact buffering and ZIP work on async runtime | Retained under archive/export hardening and recording/context import safety. |
| F9, V2 A.5: serial webhook delivery and batch abort | Retained under webhook/event-delivery backlog alongside SSRF controls. |
| F13: `docker system df -v` for context storage | Retained as resource lifecycle scalability backlog. |
| F14: untuned bb8 pool sizing | Retained in catalog scalability and gateway config-reference backlog. |
| F15: O(n) sent-hash LRU | Retained as lower-priority render/emit backlog; not a production gate. |
| F16, V2 B7: per-frame write syscalls | Retained as lower-priority host/gateway transport optimization. |
| F17, V2 B6: Node worker output buffering, per-line posts, timeout-less fetches, overlapping polls | Retained as worker reliability backlog. |
| F18: NAL fragmentation/extraction copies | Retained as lower-priority video pipeline optimization. |
| F19: host loop idle wakeups | Retained as lower-priority host power/perf backlog. |
| F20: pointer layout reads and scroll hash allocation | Retained as lower-priority browser-client interaction/render backlog. |

## Maintainability Traceability

| Finding family | Coverage decision |
| --- | --- |
| F1/V2 A2: `session_control` size and untested Postgres backend | Retained. Store contract tests are required before broad store/scalability refactors. |
| F2: two admin apps without retirement gate | Partially resolved. `/admin-new/` passed the explicit promotion gate and is the default; `/admin/` remains a compatibility fallback until a separate removal decision and regression gate. |
| F3: size-ceiling breaches and long functions | Retained as maintainability backlog; fix opportunistically inside touched modules. |
| F4/V2 A4: no CI, no ESLint, `.mjs` guardrail gap | Retained as durable CI/lint requirement. |
| F5/F6/F8: admin scaffolding, smoke helpers, and gateway test duplication | Retained as pattern-library/API-client/test-helper cleanup, not a standalone blocker. |
| F7: wall-clock sleeps and hermeticity hazards | Retained in validation guardrails for future smoke/e2e work. |
| F9/V2 B2: wide state bags and long positional parameter lists | Retained as boundary cleanup guidance, especially for host/tile pipeline work. |
| F10/V2 B4: mixed error strategy | Retained as subsystem refactor guidance. |
| F11: canonical guide drift | Retained as docs synchronization requirement. |
| F12: dependency hygiene | Promoted into focused issue `#151`. The 2026-07-31 Dependabot audit includes patched critical/high findings, including runtime dependencies, so vulnerability remediation and an enforced Rust/Node lockfile scan are P1 validation work rather than low-priority maintenance. |
| F13: integration-edge coverage holes | Retained in validation matrix and per-slice battle-testing expectations. |
| F14: dead code/minor consistency | Retained as opportunistic cleanup. |
| V2 B1: missing domain ID newtypes | Retained for high-risk gateway/store boundaries. |
| V2 B3: crate-wide `use super::*` glob seams | Retained as low/medium cleanup; avoid expanding in new code. |
| V2 B5: unmeasured allocation surface | Retained as profiling note, not a current roadmap slice. |

## Documentation Traceability

| Finding family | Coverage decision |
| --- | --- |
| OpenAPI examples/descriptions thin | Retained in API companion and OpenAPI documentation backlog. |
| Gateway rustdoc density low | Retained as backend documentation backlog, especially around public/store/runtime boundaries. |
| ARCH.md stale sizes and missing diagram | Retained in durable documentation cleanup. |
| ARCH.md capture/cache claims factually stale | Retained as explicit accuracy fix before production promotion. |
| Admin apps missing from durable architecture maps | Retained; ARCH.md and README need to reflect `/admin/` plus `/admin-new/` topology. |
| `docs/*_PLAN.md` ignored despite AGENTS mandate | Fixed in current docs branch through `.gitignore` exceptions; keep future plan files versioned. |
| No gateway config reference | Retained as production packaging/documentation work. |
| No `SECURITY.md`, threat model, vulnerability reporting, backup/upgrade docs | Retained as production-readiness documentation. |
| README too broad and duplicative | Retained as documentation cleanup; do not block the next security slice. |
| TS package READMEs and workflow author guide missing | Retained as SDK/workflow authoring docs backlog. |
| Minor inconsistency list | Retained only where it affects live behavior or current docs; stale numeric details should be fixed during the ARCH/README pass. |

## Missing-Feature Traceability

| Finding family | Coverage decision |
| --- | --- |
| Access control, teams/orgs, immutable audit, API keys/PATs | Retained as enterprise control-plane backlog, not an admin-new promotion blocker except identity route visibility. |
| Observability and ops | Split: readiness/graceful shutdown/metrics facade are promoted; broad observability remains backlog. |
| HA, backup/restore, disaster recovery | Retained as production packaging backlog. |
| Deployment packaging | Retained for production topology work; local compose remains the current primary runtime. |
| Automation parity, SDKs, stealth, proxy rotation, CAPTCHA, mobile | Retained as market/product backlog; not required before current admin/security cleanup. |
| Rate limiting, abuse controls, quotas, billing, usage metering | Retained in project governance and enterprise backlog; project-level quota work already exists and should not be conflated with global abuse protection. |
| Recording and playback | Partially implemented; remaining artifact finalization, session route, and playback/export depth are retained. |
| Security and compliance | Split across token/auth/webhook/import/runtime slices plus later compliance docs and controls. |
| V2 graceful shutdown and dependency-aware readiness | Promoted to gateway lifecycle/readiness slice. |
| V2 metrics facade and event bus gap | Retained as explicit observability/scalability backlog. The 750 ms admin-event poll/diff stream is not considered a durable audit/event bus. |

## Items Intentionally Not Promoted To The Next Slice

These findings remain valid but should not be bundled into the immediate
token/admin-auth/security work:

- stealth/fingerprint/CAPTCHA/mobile automation features,
- Python SDK and broad language SDK expansion,
- enterprise HA/DR/BYOK/data-residency packaging,
- lower-priority media/render micro-optimizations,
- broad monolith splitting without a touched-domain reason,
- deleting `/admin/` without a separate compatibility-removal decision and
  regression gate.

## Good-State Check

The consolidated docs are in a good planning state when:

1. `NEXT_WORKING_ROADMAP.md` continues to name the next shippable slices,
2. `SECURITY_RUNTIME_ROADMAP.md` keeps the detailed security/runtime backlog,
3. `VALIDATION_MATRIX.md` carries test obligations for each review-derived
   risk,
4. admin-new parity remains tracked in `ADMIN_NEW_STATUS.md` and
   `ADMIN_NEW_REQUIREMENTS.md`,
5. this audit can map every raw review report to one of:
   - implemented/superseded baseline,
   - active next slice,
   - explicit backlog,
   - intentional non-blocking deferral.
