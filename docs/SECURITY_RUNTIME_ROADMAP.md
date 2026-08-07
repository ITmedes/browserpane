# Security And Runtime Cleanup Roadmap

Revalidated: 2026-08-04

This document preserves the still-valid cleanup findings that gate production
readiness and `/admin-new` promotion. It is standalone and does not require the
old review folder or old plan files.

## Priority Rule

Prefer one focused issue and one focused PR per slice. Do not mix admin route
parity with unrelated production hardening unless the UI directly depends on
the hardening.

Every cleanup slice should include:

1. narrow unit tests for modified Rust/TypeScript modules,
2. impacted gateway API or worker tests,
3. impacted admin tests when UI behavior changes,
4. at least one compose/browser smoke for runtime, workflow, recording, MCP,
   egress, or admin session behavior,
5. Documentation and OpenAPI updates only when behavior, topology, API, setup,
   or validation flow changes.

## Review Reconciliation Notes

The raw `review/` folder was compared against this consolidated workspace and
the current tree. The corrected review layer is represented in
`REVIEW_FINDINGS_RECONCILIATION.md`.

Important status:

- workflow git-source RCE and source-preview symlink read findings are treated
  as superseded by the current baseline and remain covered by the completed
  workflow-source hardening slice,
- bridge-local `/control-session` unauthenticated takeover is treated as
  superseded by the current bridge-control-auth slice, while production
  exposure of MCP transports still needs network/origin/auth hardening,
- token-domain confusion and raw credential logging/URL transport are resolved
  by #145; admin browser auth, webhook SSRF, browser-context import limits,
  graceful shutdown, and control-plane aggregation scalability remain open.

## Completed High-Priority Slices In The Current Baseline

### Workflow Git Source And Preview Safety

Goal: remove gateway-process RCE and host-file-read paths in workflow source
handling.

Implemented direction:

- validate workflow source URL schemes,
- reject dangerous URL forms such as local paths, `file://`, `ext::`, SCP-like
  syntax unless explicitly allowed, and leading-dash arguments,
- preserve local compose workflow development through explicit trusted local
  roots like `/workspace`,
- apply source validation at create/update, source validation, preview, and
  materialization points,
- restrict git protocols and disable dangerous protocol helpers,
- reject source preview symlink/path escape behavior,
- retain existing local workflow smokes.

Current state: included in merged PR `#143`.

### MCP Bridge Control Auth

Goal: prevent unauthenticated default-session takeover through bridge-local
control mutation.

Implemented direction:

- bridge-local `/control-session` requires internal bearer auth,
- admin/CLI mutate default bridge control through authenticated gateway proxy,
- gateway validates owner/session visibility before forwarding,
- gateway reconciles ambiguous bridge write failures when the bridge already
  applied the requested state,
- direct unauthenticated bridge control returns `401`,
- real MCP session-scoped endpoint smoke verifies `tools/list` and
  `browser_navigate`.

Current state: included in merged PR `#143`.

Remaining MCP hardening:

- if MCP transports are host-exposed in production, require inbound transport
  auth or bind them to internal-only networks,
- replace broad CORS with configured allowed origins,
- split health detail by trust level.

## Completed High-Priority Slice: #145

### Token Domain Separation And Credential Redaction

Priority: high.

Risk:

- connect tickets and automation tokens must not be interchangeable,
- WebTransport request paths and admin-event URLs can expose bearer material in
  logs or browser-visible URLs,
- the previous admin event stream query-auth design exposed the raw owner
  bearer token and required replacement,
- request paths can inject misleading log content if CR/LF or query material is
  not sanitized.

Required implementation:

1. Add purpose/audience domain separation to signed token payloads or derive
   distinct signing keys.
2. Use versioned token parsing so old/wrong-purpose tokens fail safely.
3. Use constant-time HMAC verification.
4. Redact query strings in transport warnings and sanitize logged paths.
5. Keep WebTransport tickets low-privilege and short-lived if query transport
   remains necessary.
6. Replace raw owner-token query auth for `/api/v1/admin/events` with a
   short-lived purpose-scoped event ticket or browser-compatible subprotocol
   auth.
7. Update the current old-admin event client and define a reusable secure event
   credential/client contract for the future unified-admin observability route.

Validation:

- connect ticket rejected by automation token validation and vice versa,
- malformed/expired/wrong-purpose token tests,
- transport log tests proving raw `token`, `access_token`, and
  `session_ticket` values are not logged,
- admin event tests proving raw owner bearer tokens are not in WebSocket URLs,
- old-admin realtime/event and reconnect smokes,
- admin-new auth/session regression coverage if shared credential issuance
  changes.

Current state: implemented and validated on `feature/BPANE-00145`. Connect,
automation, and admin-event credentials use separate v2 purpose domains;
admin-event WebSockets use a short-lived first-frame credential; transport
request logging strips credential-bearing query data and control characters.
The complete evidence is recorded in
`BPANE-00145_TOKEN_DOMAIN_SEPARATION_PLAN.md`.

## Next Cleanup Slices

### Admin Browser Auth And Web Security

Priority: high before default admin promotion.

Current state: implemented and validated on `feature/BPANE-00146`. Both admin
apps consume one shared adapter backed by the OpenID-certified `oauth4webapi`
protocol core; all tokens remain in memory, bounded PKCE transaction state is
the only auth state stored per tab, verified claims drive identity display, and
both routes receive the shared CSP/security-header contract. Evidence is in
`BPANE-00146_ADMIN_AUTH_SECURITY_PLAN.md`.

Resolved baseline risk:

- both admin apps persist OIDC token sets in browser storage,
- login requests lack nonce replay defense,
- ID token claims are decoded for display without signature verification,
- the nginx/static layer lacks CSP and standard browser hardening headers,
- old and unified admin currently duplicate security-sensitive auth code.

Implemented controls:

1. Extract or share the admin auth implementation before deep hardening where
   practical.
2. Keep access tokens in memory where possible and minimize refresh-token
   exposure to JavaScript-accessible storage.
3. Add OIDC nonce generation, storage, and validation.
4. Verify ID-token signatures/issuer/audience before using ID-token claims for
   identity display; continue to treat gateway access as server-validated.
5. Add CSP, `X-Content-Type-Options`, `Referrer-Policy`, and appropriate frame
   and transport security headers to the local/proxy static-serving path.
6. Keep local demo-password exposure documented as local-only and avoid
   presenting it as production behavior.

Validation:

- shared auth unit tests for nonce validation, state validation, refresh, and
  logout,
- browser-token-store tests proving the chosen persistence model,
- negative tests for unverified or wrong-audience ID tokens,
- nginx/static header checks,
- old and unified admin login/logout/expired-auth smokes.

### Recording Artifact Finalization Boundary

Priority: medium/high.

Risk:

- recording completion currently accepts a gateway-local absolute
  `source_path`,
- automation access can call completion,
- artifact store validation must not allow arbitrary gateway-local file
  movement/readback.

Required implementation:

1. Treat recording completion as a trusted-worker operation.
2. Enforce a configured recording worker staging root.
3. Canonicalize staging root and submitted path.
4. Reject relative paths, path escapes, directories, symlinks, non-files, and
   wrong session/recording directories.
5. Prefer opaque staging artifact ids if the API can evolve without too much
   churn.
6. Recheck authorization so ordinary session automation tokens cannot finalize
   arbitrary files.
7. Keep ready recording download/export behavior unchanged.
8. Update OpenAPI to mark completion as worker/internal or document the staging
   contract.

Validation:

- unit tests for source-path validation,
- API tests for outside-root, symlink, directory, wrong-session, relative, and
  valid staged-file cases,
- recording worker build/test,
- compose recording smoke with downloadable WebM/export,
- unified admin recordings smoke if UI behavior changes.

### Webhook SSRF Controls

Priority: high/medium.

Current state: implemented and validated on `feature/BPANE-00147`. New and
persisted destinations are parsed, resolved, classified, and pinned before
delivery; redirects and implicit proxies are disabled. Exact-origin exceptions
are startup-validated deployment configuration. Evidence is in
`BPANE-00147_WORKFLOW_WEBHOOK_SSRF_PLAN.md`.

Risk:

- workflow event subscription delivery can target internal services if URLs are
  only prefix-validated.

Required implementation:

1. Parse `target_url` with URL semantics.
2. Disable redirects by default.
3. Deny loopback, link-local, private, multicast, and unspecified IP ranges.
4. Add repeatable exact-origin configuration for controlled receivers.
5. Resolve hostnames and pin the approved answers into the delivery client.

Validation:

- URL parser, alternate IP notation, IP-range, DNS timeout/mixed-answer, and
  exact-origin unit tests,
- redirect/internal target rejection and DNS-pinned delivery tests,
- API validation, canonical persistence, persisted-target revalidation, and
  live compose delivery tests.

### Browser Context Import Safety

Priority: medium.

Risk:

- import body limits and profile archive expansion can cause DoS,
- tar entries can include symlink/hardlink forms if not explicitly rejected.

Required implementation:

1. Restore or replace body-size limits for browser context import.
2. Cap declared and actual uncompressed `profile.tar.gz` size.
3. Cap ZIP entry count and manifest size.
4. Reject symlink and hardlink tar members before extraction.
5. Move CPU-heavy ZIP/package parsing off the async runtime where needed.

Validation:

- oversized ZIP,
- oversized profile,
- multiple profile archives,
- missing manifest,
- symlink/hardlink tar rejection,
- existing clone/export/import tests still pass.

### Gateway Lifecycle, Health, And Readiness

Priority: medium/high for deployability.

Status: implemented and fully validated on `feature/BPANE-00150`; review/merge
remain.

Implemented boundary:

1. SIGINT/SIGTERM drives one shared monotonic lifecycle coordinator.
2. HTTP and WebTransport stop admitting new work during drain and retain owned
   work until completion or the configured global timeout.
3. `/healthz` remains a resource-free process-liveness probe.
4. `/readyz` fails closed while starting, draining, or when Postgres, the
   runtime manager, configured Vault, or either local artifact store fails its
   bounded check.
5. Compose and the compose e2e preflight consume `/readyz`.
6. External workflow and recording worker jobs are not claimed as drained by
   the gateway process; their persisted assignment/reconciliation contracts
   remain the recovery boundary.

Validation evidence:

- lifecycle/readiness state, timeout, and response composition tests,
- compose Postgres loss/recovery test with liveness remaining available,
- real container SIGTERM smoke with observable 503 readiness withdrawal before
  listener closure and successful restart,
- canonical full validation passed all 46 stages, including workspace tests and
  coverage, 21 compose API surfaces, both admin paths, CLI, MCP, recording, and
  workflow browser smokes.

### Admin And Session Catalog Scalability

Priority: medium/high before default promotion.

Risk:

- session/admin aggregation can become O(N^2),
- workflow runs need stronger session/project indexes for scalable dashboards
  and status calculations.

Required implementation:

1. Push session pagination/filtering into store queries.
2. Add targeted per-session active workflow/automation counts.
3. Add workflow-run `session_id` and/or state-aware indexes.
4. Replace queued-position calculation with targeted query.
5. Reuse batched aggregates for dashboard and identity/access-review.
6. Replace admin-event snapshot N+1 scans with shared batched snapshots or a
   cached/published event model.
7. Size and document the Postgres pool for expected admin/catalog concurrency.

Validation:

- Postgres contract tests for the store backend, not only ignored compose e2e,
- API tests proving payload shape remains stable,
- seeded API/perf smoke with enough sessions/runs/workflow-runs/admin-event
  subscribers to catch query-count or latency regressions.

### Docker Runtime Launch Boundary

Priority: production hardening.

Risk:

- raw Docker socket exposure in a gateway container amplifies a gateway
  compromise into host control.

Required implementation:

1. Document raw Docker socket use as local-dev only.
2. Introduce a scoped runtime-launch boundary:
   - Docker socket proxy with allowlisted API subset, or
   - internal runtime-launch broker, or
   - future non-Docker production runtime manager.
3. Keep local dev session/workflow/recording launch working.
4. Update production topology docs.

Validation:

- compose smoke for local runtime launch,
- negative validation for denied Docker API operations if a proxy is used,
- docs proving production guidance no longer presents raw socket as safe
  default.

## Performance And Maintainability Backlog From Review

These are valid review findings but are not the first production-security
slices unless they directly touch active work:

- host capture/classify: ARCH.md must state that capture is full-screen for
  each damaged frame, while XDamage gates work and scopes downstream emission;
  future host work should scope capture/classify hashing to damage regions
  where feasible,
- client rendering: cache-before-decode for duplicate tile hashes, implement or
  remove the documented `ImageBitmap` cache path, and update ARCH.md cache-size
  claims,
- gateway fan-out: avoid per-viewer keyframe/frame re-encode where a shared
  pre-encoded frame can be safely reused,
- exports/archives: move CPU-heavy ZIP construction off async runtime threads
  and stream large artifacts where possible,
- workers: cap stdout/stderr accumulation, add request timeouts, and prevent
  overlapping supervisor polls,
- session-control store: add in-tree Postgres contract tests for the
  high-volume store API before broad store refactors,
- CI/lint and dependency safety: add a minimal pipeline for Rust
  fmt/clippy/tests, Node checks/tests, admin builds, targeted smoke wrappers,
  and Rust/Node lockfile vulnerability checks; remediate patched critical/high
  findings or record a bounded, owned exception,
- Rust boundaries: consider domain ID newtypes, context structs for long
  same-typed parameter lists, and error-strategy cleanup when touching those
  modules.

## Admin Promotion Cleanup

Before `/admin-new` becomes default:

- route-backed workflow-run detail must exist,
- route-backed session subareas must exist or be explicitly deferred,
- identity/access review must exist,
- visible `/api`, `/coverage`, `/docs` nav items must exist or be hidden,
- auth/security fixes must not need duplicate implementation in both admin
  apps,
- old admin must remain as a fallback until a dated removal gate is accepted.

## Durable Documentation And Guardrails

Required cleanup before production promotion:

1. Fix architecture docs where they conflict with implementation:
   - full-screen capture per damaged frame versus damaged-tile capture,
   - downstream damage-bounded tile emission,
   - current tile cache data type and size,
   - actual admin app topology and source locations.
2. Add both admin apps to TypeScript/Svelte standards scope.
3. Add gateway configuration reference for important CLI flags.
4. Add a security/threat-model document with local-dev caveats.
5. Add OpenAPI examples/descriptions for high-use operations.
6. Add minimal CI for Rust fmt/clippy/tests, Node checks/tests/build, targeted
   smoke wrappers, and Rust/Node lockfile vulnerability checks.
