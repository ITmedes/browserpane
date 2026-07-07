# Review Findings Reconciliation

Created: 2026-07-07

This document compares the raw reports in `review/` with the consolidated
planning workspace in `docs/` and with spot-checks against the current tree.
The raw review folder is useful source material, but it should not become the
working plan directly because some findings were produced before later fixes
landed.

## Source Reports

Review inputs checked:

- `review/00_OVERVIEW.md`
- `review/00_VERIFICATION.md`
- `review/SECURITY.md`
- `review/SECURITY_V2.md`
- `review/PERFORMANCE.md`
- `review/PERFORMANCE_V2.md`
- `review/MAINTAINABILITY.md`
- `review/MAINTAINABILITY_V2.md`
- `review/DOCUMENTATION.md`
- `review/DOCUMENTATION_V2.md`
- `review/MISSING_FEATURES.md`
- `review/MISSING_FEATURES_V2.md`

The `*_V2.md` files are treated as the corrected review layer where they
disagree with the first-pass reports.

For report-by-report traceability, see
`REVIEW_FINDINGS_COVERAGE_AUDIT.md`.

## Current Reconciliation Result

The consolidated docs already represented much of the review:

- workflow source hardening,
- MCP bridge control hardening,
- token/log cleanup,
- webhook SSRF controls,
- browser-context import safety,
- gateway health/readiness,
- admin/session catalog scalability,
- raw Docker socket production-boundary risk,
- durable docs and CI gaps,
- admin-new promotion blockers.

The review still adds sharper current guidance in these areas:

- token-domain separation is the highest confirmed-open security cleanup in
  the current code,
- admin browser auth and web security need their own explicit slice,
- graceful shutdown is not just an HA topic; it is a cheap single-node
  reliability blocker,
- admin event snapshots and identity/access-review share the same scalability
  risk class as `/api/v1/sessions`,
- the performance backlog needs explicit host capture/classify, client
  decode/cache, keyframe fan-out, ZIP, and worker-output items,
- maintainability needs explicit Postgres store contract tests and CI/lint
  ratchets,
- documentation needs explicit ARCH.md capture/cache corrections and a security
  posture document.

## Security Findings

| Review finding | Current status | Consolidated owner |
| --- | --- | --- |
| C1 workflow git RCE through `repository_url` and dangerous git protocols | Superseded by current code/docs. The current tree validates repository URL forms, sets `GIT_ALLOW_PROTOCOL`, and disables `protocol.ext`. | `SECURITY_RUNTIME_ROADMAP.md`, `DOMAIN_REQUIREMENTS.md`, `VALIDATION_MATRIX.md` |
| N1 workflow source-preview symlink host-file read | Superseded by current code/docs. The current tree uses symlink/canonicalization checks on workflow source paths. | `SECURITY_RUNTIME_ROADMAP.md`, `VALIDATION_MATRIX.md` |
| N2 bridge-local `/control-session` unauthenticated takeover | Partially superseded. Current bridge local control routes require internal authorization and gateway proxying exists. Remaining production issue: host-exposed MCP transports still need inbound auth/origin/network policy. | `SECURITY_RUNTIME_ROADMAP.md`, `RUNTIME_OPERATOR_REQUIREMENTS.md` |
| N3 connect ticket and automation token are cryptographically interchangeable | Open. Current code still uses the same shared secret, byte-identical claims, identical token format, and plain equality for HMAC bytes. | `SECURITY_RUNTIME_ROADMAP.md`, `NEXT_WORKING_ROADMAP.md` |
| M3 bearer material in WebTransport/admin URLs and logs | Open. Transport warnings still log raw request paths, and old admin event clients still use `access_token` query auth. | `SECURITY_RUNTIME_ROADMAP.md`, `NEXT_WORKING_ROADMAP.md` |
| H1 webhook SSRF through weak `target_url` validation and redirects | Open and represented. Needs URL parsing, redirect blocking, IP/private-range denial, and optional allowlist. | `SECURITY_RUNTIME_ROADMAP.md` |
| M1 browser-context import unbounded decompression | Open and represented. Current import still disables the body limit and reads `profile.tar.gz` into memory without an uncompressed-size cap. | `SECURITY_RUNTIME_ROADMAP.md`, `RESOURCE_LIFECYCLE_REQUIREMENTS.md` |
| M2 no CSP/security headers | Open, previously under-specified. Now promoted into an explicit admin web-security cleanup. | `SECURITY_RUNTIME_ROADMAP.md` |
| H2/H3 admin tokens in `sessionStorage`, no OIDC nonce, no ID-token verification | Open. Needs shared admin auth hardening across old and unified apps. | `SECURITY_RUNTIME_ROADMAP.md`, `ADMIN_NEW_IMPLEMENTATION_GUARDRAILS.md` |
| M4 legacy HMAC dev-token fallback and generated token logging | Open as a local-dev caveat. Needs documentation and safer defaults before non-local deployment. | `SECURITY_RUNTIME_ROADMAP.md`, `RUNTIME_OPERATOR_REQUIREMENTS.md` |
| L1 non-constant-time HMAC comparison | Open. Best handled inside the token-domain separation slice. | `SECURITY_RUNTIME_ROADMAP.md` |
| L2 tar symlink/hardlink import risk | Open. Best handled inside the browser-context import safety slice. | `SECURITY_RUNTIME_ROADMAP.md` |
| L4 legacy singleton status/MCP routes authorize any principal | Low and legacy-scope. Keep behind compatibility gating and avoid treating as production multi-tenant surface. | `SECURITY_RUNTIME_ROADMAP.md`, `ADMIN_NEW_API_COVERAGE.md` |
| N4 log injection through unsanitized request path | Open. Best handled with transport log redaction/sanitization. | `SECURITY_RUNTIME_ROADMAP.md` |
| N5 worker writes live automation token to `context.json` with default perms | Open low-severity worker hygiene. | `SECURITY_RUNTIME_ROADMAP.md` |
| N6 `RwLock` poison cascade in session hub | Open low-severity resilience hardening. | `SECURITY_RUNTIME_ROADMAP.md` |

Security conclusion: the most severe old review findings for workflow source
and bridge-local control are now covered by the current baseline, but the token
and browser-auth findings remain confirmed-open and should outrank lower
confidence cleanup if only one production-hardening slice is selected.

## Performance Findings

| Review finding | Current docs status | Action |
| --- | --- | --- |
| `/api/v1/sessions` O(N^2), Rust-side pagination, missing workflow-run `session_id` index | Covered, but now needs sharper scope. | Keep in `Admin And Session Catalog Scalability`; include SQL pagination, targeted counts, queued-position query, and index work. |
| `/identity/access-review` and admin event snapshots share the O(N)/N+1 aggregation shape | Under-specified. | Add these endpoints explicitly to the scalability slice. |
| Host captures full screen per damaged frame and hashes full grid; ARCH.md says damaged-tile capture | New explicit docs/action item. | Fix ARCH.md and add host capture/classify perf backlog. |
| Client decodes cache-hit tiles and cache stores `ImageData`, not `ImageBitmap` | New explicit docs/action item. | Add client render perf backlog and ARCH.md cache correction. |
| Per-viewer frame/keyframe re-encode and large memcpy on async runtime | Under-specified. | Add gateway fan-out perf backlog. |
| Whole-artifact buffering and ZIP creation on async runtime | Partially covered for recording/context safety. | Add `spawn_blocking` and streaming export expectations where APIs allow. |
| Serial webhook delivery with batch abort on one failed target | Covered as SSRF only, not performance. | Add delivery concurrency/backoff/partial-batch behavior to webhook/event backlog. |
| Node workers accumulate stdout/stderr unbounded and use timeout-less fetches | New explicit reliability backlog. | Add worker-output limits, request timeouts, and poll in-flight guards. |
| `Mutex<SendStream>` held across write awaits, write syscall coalescing, rAF parking, NAL scan | Lower priority. | Keep as profiling backlog, not a production gate. |

Performance conclusion: the current docs have the right top-level scalability
slice, but the review adds important endpoint names and media/rendering backlog
items that should not be lost.

## Maintainability Findings

| Review finding | Current docs status | Action |
| --- | --- | --- |
| Postgres session-control backend has no in-tree unit/contract tests | Under-specified. | Add as a first-class maintainability requirement for store/API work. |
| No CI, no ESLint, no lint ratchet around existing quality metrics | Covered generally. | Keep minimal CI as a durable guardrail; include admin apps and `.mjs` scripts. |
| Two admin apps and duplicated auth stack | Covered. | Keep old admin fallback until promotion, but shared auth hardening should avoid duplicated fixes. |
| Per-domain admin scaffolding, smoke helpers, gateway API test request chains duplicated | Partially covered as pattern/library work. | Treat as opportunistic cleanup when touching those areas, not a standalone promotion gate. |
| No domain ID newtypes despite standards preference | New explicit backlog. | Consider typed IDs for high-risk gateway/store boundaries after security cleanup. |
| Same-typed long positional flag lists in tile pipeline | New explicit backlog. | Prefer context structs for hot-path APIs when touching host capture/emit work. |
| Three-way error strategy: `anyhow`, typed enums, stringly backend errors | New explicit backlog. | Standardize per subsystem when refactoring store/runtime boundaries. |
| Crate-wide `use super::*` glob seams | New low/medium backlog. | Avoid expanding glob seams in new code; clean locally when refactoring. |

Maintainability conclusion: the current docs are strong on admin migration but
thin on backend test contracts. Store contract tests and CI should be explicit
acceptance criteria for scalability/store changes.

## Documentation Findings

| Review finding | Current docs status | Action |
| --- | --- | --- |
| `docs/*_PLAN.md` was ignored despite AGENTS requiring plan files | Fixed in current docs branch. `.gitignore` now unignores `docs/*_PLAN.md` and nested plan files. | No further action except keep this guardrail. |
| ARCH.md capture-pipeline claim is false: capture is full screen per damaged frame, not damaged tiles only | Open. | Fix ARCH.md with exact capture/classify/emit behavior. |
| ARCH.md cache claims are stale: `ImageBitmap` path not implemented and tile cache cap differs | Open. | Fix ARCH.md when doing the capture/cache accuracy pass. |
| Admin apps absent from durable architecture/standards maps | Partially represented by `AGENTS.md`; still needs ARCH/README alignment where missing. | Update ARCH.md and root README if still stale. |
| OpenAPI has no examples and few schema/operation descriptions | Covered. | Keep API companion/docs route and OpenAPI examples in backlog. |
| No gateway config reference for many CLI flags | Covered. | Create durable config reference before production packaging. |
| No SECURITY.md, threat model, vulnerability reporting, backup/upgrade docs | Covered generally. | Add explicit security posture and operations docs before production promotion. |

Documentation conclusion: the review validates the consolidated docs direction
but identifies two specific ARCH.md inaccuracies that need a focused accuracy
pass.

## Missing-Feature Findings

| Review finding | Current docs status | Action |
| --- | --- | --- |
| No gateway graceful shutdown | Covered, now sharper. | Treat as single-node reliability work, not just HA. |
| No dependency-aware readiness | Covered. | `/readyz` should include Postgres, Docker/runtime, Vault when configured, and artifact-store checks. |
| No Prometheus/OpenTelemetry metrics facade | Under-specified. | Add metrics facade before exporter work so observability is not a one-off JSON rewrite. |
| Admin event stream is 750ms poll/diff, not a scalable event bus | Under-specified. | Do not count it as audit/event infrastructure; include in scalability/event backlog. |
| No RBAC/teams/orgs, audit log, API keys/PATs, rate limiting | Covered as enterprise backlog. | Keep as later enterprise control-plane work after immediate hardening. |
| No backup/restore, HA, deployment packaging, SBOM/signing, BYOK/data residency | Covered as enterprise backlog. | Keep later unless production packaging becomes the active slice. |
| No Python SDK, stealth/fingerprinting, proxy rotation, CAPTCHA support | Covered as market-positioning backlog. | Do not block admin-new promotion on these. |

Missing-feature conclusion: graceful shutdown and readiness are the most
effort-adjusted production gaps; larger enterprise features remain backlog.

## Recommended Roadmap Adjustment

The current code/docs baseline makes the old review C1/N1/N2 findings mostly
superseded, but it leaves several confirmed-open security items. The next
production-hardening priority should be:

1. token domain separation, log redaction, and admin-event credential cleanup,
2. admin browser auth and web security hardening,
3. webhook SSRF controls,
4. browser-context import safety,
5. gateway graceful shutdown plus health/readiness.

Recording artifact finalization remains a valid cleanup item from the older
plan set, but the review did not find the already-exposed recording
playback/export paths unsafe. It should not displace the confirmed-open token,
webhook, import, browser-auth, and lifecycle findings if we are choosing the
next security-driven slice.

## Validation Additions

Future implementation slices derived from this reconciliation should include:

- token cross-purpose rejection tests,
- HMAC constant-time verification tests or library-backed validation,
- transport log redaction tests for `token`, `access_token`, and
  `session_ticket`,
- admin event auth tests proving owner bearers are not placed in WebSocket
  query strings,
- OIDC nonce and ID-token verification tests in the shared admin auth module,
- CSP/security-header checks in nginx/build output,
- webhook private-IP/redirect rejection tests,
- browser-context zip/profile/tar-size and symlink/hardlink rejection tests,
- graceful shutdown and readiness composition tests,
- seeded control-plane aggregation tests for sessions, identity/access-review,
  and admin-event snapshots,
- ARCH.md accuracy check when capture/render behavior changes.
