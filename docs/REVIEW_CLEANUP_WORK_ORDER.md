# Review Cleanup Work Order

Created: 2026-07-07

This file turns the cleanup and refactoring findings from `review/` into a
practical work order. It is intentionally narrower than
`REVIEW_FINDINGS_COVERAGE_AUDIT.md`: that file proves coverage, while this file
answers what to do first.

## Selection Criteria

Priority is based on:

1. production risk reduction,
2. dependency order,
3. ability to ship in one coherent PR,
4. testability,
5. how much future work the cleanup unlocks.

This order does not rank broad product features such as teams, billing, HA,
BYOK, stealth, CAPTCHA, mobile, or SDK expansion. Those remain product roadmap
items, not cleanup/refactoring slices.

## Already Closed Or Baseline-Protected

Do not reopen these as cleanup slices unless a regression is found:

- workflow git source RCE and dangerous git protocols,
- workflow source-preview symlink host-file read,
- bridge-local `/control-session` unauthenticated takeover,
- ignored `docs/*_PLAN.md` planning files.

Keep regression coverage for these areas when adjacent code changes.

## Work Order

### 1. Token Domain Separation And URL Credential Cleanup

Priority: P0.

Why first:

- It is confirmed open in the current tree.
- It crosses API, transport, logs, and both admin apps.
- It is a trust-boundary cleanup with limited product-design ambiguity.

Scope:

- separate connect-ticket and automation-token signing domains or bind purpose
  into the MAC,
- use constant-time signature verification,
- redact query material from WebTransport and admin-event logs,
- replace raw owner bearer query auth for admin events with a scoped
  event-stream credential,
- update old admin and admin-new together.

Validation:

- wrong-purpose token rejection tests,
- malformed/expired-token tests,
- transport log redaction tests,
- old and new admin event-stream smokes.

### 2. Shared Admin Browser Auth And Web-Security Hardening

Priority: P0.

Why next:

- It removes duplicated security-sensitive auth behavior across the two admin
  apps.
- It is a prerequisite for promoting admin-new.
- It directly addresses the review's browser-facing auth findings.

Scope:

- extract or align shared admin auth logic,
- add OIDC nonce validation,
- verify ID-token issuer/audience/signature before using identity claims,
- reduce JavaScript-readable refresh-token exposure where practical,
- add CSP and standard security headers to the static serving path,
- keep demo credentials documented as local-only.

Validation:

- auth unit tests for nonce/state/logout/refresh,
- negative ID-token validation tests,
- security-header checks,
- old admin and admin-new login/logout/expired-auth smokes.

### 3. Webhook SSRF Controls

Priority: P0/P1.

Why here:

- It is a high-risk network boundary.
- It is independent enough to ship without touching admin-new layout work.
- It protects workflow event subscriptions before broader automation work grows.

Scope:

- parse `target_url` with URL semantics,
- reject loopback, link-local, private, multicast, and unspecified IP ranges,
- disable redirects by default,
- add optional allowlist configuration for enterprise deployments,
- document remaining DNS-rebinding tradeoffs.

Validation:

- URL/IP-range unit tests,
- redirect rejection tests,
- API validation tests for bad subscription URLs.

### 4. Browser Context Import Safety

Priority: P1.

Why here:

- It is a confirmed DoS and archive-safety gap.
- It has clear acceptance criteria.
- It protects a resource type already exposed in admin-new.

Scope:

- restore or replace request body limits,
- cap declared and actual uncompressed profile size,
- cap ZIP entry count and manifest size,
- reject symlink and hardlink tar members,
- move CPU-heavy archive parsing off async runtime threads where needed.

Validation:

- oversized ZIP/profile tests,
- multiple-profile and missing-manifest tests,
- symlink/hardlink rejection tests,
- existing browser-context clone/export/import tests.

### 5. Recording Artifact Finalization Boundary

Priority: P1.

Why here:

- Recording download/export is already visible to operators.
- The current worker completion path is a file-boundary cleanup, not a UX
  feature.
- It reduces risk before recording routes become deeper in admin-new.

Scope:

- treat recording completion as trusted-worker-only behavior,
- enforce a configured staging root,
- canonicalize and reject relative paths, path escapes, symlinks, directories,
  and wrong-session paths,
- prefer opaque staging artifact IDs if the API can evolve cleanly.

Validation:

- staging-path unit tests,
- API negative tests for each rejected path shape,
- recording-worker build/test,
- compose recording smoke with downloadable artifact/export.

### 6. Gateway Lifecycle, Health, And Readiness

Priority: P1.

Why here:

- It is the highest effort-adjusted operational gap from the review.
- It makes local and future production restarts less destructive.
- It gives later smoke/e2e suites stable readiness probes.

Scope:

- add SIGINT/SIGTERM handling,
- use graceful shutdown for the API server,
- define bounded WebTransport drain behavior,
- add `/healthz`,
- add dependency-aware `/readyz` for Postgres, runtime manager, Vault when
  configured, and artifact stores.

Validation:

- readiness composition tests,
- graceful shutdown integration test where feasible,
- compose smoke for `/healthz` and `/readyz`.

### 7. Minimal CI And Lint Ratchet

Priority: P1 enabler.

Why after immediate security cleanup:

- CI is the main guardrail for every following refactor.
- It should be installed before broad duplication or store cleanup.
- It turns existing manual quality expectations into enforced checks.

Scope:

- Rust fmt/clippy/tests,
- Node TypeScript checks/tests/builds for relevant packages,
- admin-new build and focused smokes where feasible,
- `node --check` or TS migration coverage for operational scripts,
- path/doc checks for AGENTS/README/ARCH drift if lightweight.

Validation:

- CI passes on the branch,
- local script or documented command sequence mirrors CI.

### 8. Postgres Session-Control Store Contract Tests

Priority: P1 enabler.

Why here:

- The review identifies backend divergence as the largest silent
  maintainability risk.
- Store refactors should not start before backend behavior is pinned.
- It supports the later catalog scalability work.

Scope:

- shared behavioral suite for in-memory and Postgres stores,
- seed coverage for sessions, projects, workflow runs, recordings, templates,
  browser contexts, egress profiles, service principals, and identity mappings,
- keep ignored compose e2e but add faster in-tree contract coverage where
  practical.

Validation:

- contract suite runs against both backends,
- negative cases cover visibility, ownership, project scoping, and lifecycle
  transitions.

### 9. Admin And Session Catalog Scalability

Priority: P1/P2.

Why after store tests:

- It changes query behavior and aggregate semantics.
- Contract tests reduce the risk of breaking the control-plane shape.
- It is needed before admin-new becomes the default operator surface.

Scope:

- push session pagination/filtering into SQL,
- add targeted counts and `ANY($ids)` batching,
- add workflow-run `session_id` and state-aware indexes,
- replace queued-position scans with targeted queries,
- batch identity/access-review and admin-event snapshots,
- document Postgres pool sizing.

Validation:

- seeded 1k-session/10k-run API test or perf smoke,
- payload-shape regression tests,
- admin-new dashboard/session/identity smoke.

### 10. Worker And Archive Runtime Hygiene

Priority: P2.

Why here:

- It is reliability cleanup with clear boundaries.
- It complements workflow and recording hardening without broad redesign.
- It reduces async-runtime and worker failure modes before larger workflow UI
  expansion.

Scope:

- cap workflow/recording worker stdout and stderr accumulation,
- add request timeouts to worker polling/fetches,
- prevent overlapping supervisor polls,
- move CPU-heavy ZIP/export work to blocking tasks,
- remove avoidable `Body::from(bytes.clone())` clones where safe.

Validation:

- worker unit/integration tests for timeout and bounded logs,
- archive/export tests for large artifacts,
- workflow and recording smokes.

### 11. Documentation Accuracy Pass

Priority: P2, but do before related implementation changes land.

Why here:

- ARCH.md currently misstates important hot-path behavior.
- Correct docs reduce future wrong refactors.
- This is low-risk and helps reviewers.

Scope:

- fix capture/classify/cache claims in ARCH.md,
- add both admin apps to durable architecture maps,
- remove stale hardcoded LOC/file-size claims,
- add gateway config reference for important flags,
- add security/threat-model and local-dev caveat docs,
- split or point README sections where duplication is highest.

Validation:

- link/path checks,
- compare key runtime claims against manifests/code,
- no stale `doc_updated` or deleted-plan references.

### 12. Admin-New Pattern And API Helper Consolidation

Priority: P2.

Why after security/auth and before many more admin routes:

- It prevents repeated scaffolding from multiplying.
- It should follow the auth decision so helpers do not encode a stale token
  model.
- It keeps admin-new route additions cheaper.

Scope:

- consolidate repeated catalog/detail/form patterns,
- consolidate admin API clients and mappers,
- extract shared smoke helpers,
- keep route view models DOM-free and individually tested.

Validation:

- component/view-model unit tests remain route-specific,
- admin-new smokes still cover projects, contexts, egress, files, workflows,
  sessions, recordings, and dashboard.

### 13. Host And Client Render Hot-Path Work

Priority: P2/P3, profile-guided.

Why not earlier:

- It may produce large diffs in sensitive media paths.
- The review is confident about waste, but magnitude should be measured before
  broad changes beyond mechanical fixes.
- It is not the top blocker for admin-new promotion.

Scope:

- measure host capture/classify under idle, cursor-only, and video scenarios,
- reuse frame buffers,
- hash once per tile per frame,
- scope classify work to damage where feasible,
- cache-check before tile decode,
- evaluate worker decode and `ImageBitmap` cache path,
- fix ARCH.md alongside behavior changes.

Validation:

- perf trace before/after,
- host tests for tile/classification behavior,
- browser render smoke with video/tile updates,
- regression check for reconnect/full-refresh behavior.

### 14. Gateway Fan-Out And Transport Optimization

Priority: P3.

Why here:

- It is important for multi-viewer scalability but should be profiled under a
  realistic viewer count first.
- It touches streaming behavior and should not be mixed with token/auth
  cleanup.

Scope:

- profile per-viewer frame/keyframe encode and large memcpy,
- share pre-encoded bytes where safe,
- examine send-stream lock/backpressure behavior,
- evaluate late-join full-refresh broadcast behavior.

Validation:

- 1/5/10-viewer allocation and CPU profile,
- multi-session/multiviewer smoke,
- late-join bootstrap regression tests.

### 15. Structural Refactors

Priority: P3/P4.

Why last:

- These are valuable but have the highest churn.
- They should be done after tests, CI, and contract coverage are stronger.
- They should follow touched-domain boundaries, not happen as one broad PR.

Scope:

- split `SessionStore` into per-domain repository traits,
- introduce domain ID newtypes at high-risk boundaries,
- standardize error strategy and stable error codes by subsystem,
- replace long same-typed positional parameter lists with context structs,
- reduce crate-wide `use super::*` seams when touching modules,
- split top monolith files by stable subdomain.

Validation:

- no behavior changes without tests,
- contract suite stays green,
- targeted smoke for each touched subsystem,
- no broad formatting-only churn.

## Work Not Recommended For The Next Cleanup Series

Keep these outside the cleanup/refactoring sequence unless product strategy
changes:

- stealth/fingerprint/CAPTCHA/mobile automation,
- Python SDK expansion,
- billing and broad enterprise packaging,
- HA/DR/BYOK/data-residency,
- deleting `/admin/` before admin-new reaches its promotion gate,
- rewriting the admin app shell again before route parity and auth hardening.

## Suggested Immediate Next Issue

Create one focused issue for work item 1:

Title: `Token domain separation and URL credential cleanup`

Business case:

- BrowserPane has multiple token-like credentials for different trust domains:
  owner API access, session connect tickets, automation access, and admin event
  subscriptions. They must not be interchangeable or leaked through URLs/logs.
  This is foundational for any admin-new promotion or production exposure.

Acceptance criteria:

- connect tickets cannot validate as automation tokens,
- automation tokens cannot validate as connect tickets,
- signature verification is constant-time or library-backed,
- WebTransport/admin-event logs do not expose raw credential query values,
- admin event clients no longer put owner bearer tokens in WebSocket query
  strings,
- old admin and admin-new both keep realtime/event behavior.

Smoke sequence:

1. start the compose stack,
2. log into `/admin/` and `/admin-new/`,
3. create or select a session,
4. connect and disconnect the browser preview,
5. observe admin event updates in both apps,
6. run negative token API tests,
7. inspect gateway logs for redacted credential material.
