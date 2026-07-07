# Security And Runtime Cleanup Roadmap

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

## Next Cleanup Slices

### Recording Artifact Finalization Boundary

Priority: high.

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

### Token Domain Separation And Credential Redaction

Priority: high.

Risk:

- connect tickets and automation tokens must not be interchangeable,
- WebTransport request paths and admin-event URLs can expose bearer material in
  logs or browser-visible URLs,
- admin event stream query auth uses raw owner bearer token.

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
7. Update both old and unified admin event clients together.

Validation:

- connect ticket rejected by automation token validation and vice versa,
- malformed/expired/wrong-purpose token tests,
- transport log tests proving raw `token`, `access_token`, and
  `session_ticket` values are not logged,
- admin event tests proving raw owner bearer tokens are not in WebSocket URLs,
- old and unified admin realtime/event smokes.

### Webhook SSRF Controls

Priority: high/medium.

Risk:

- workflow event subscription delivery can target internal services if URLs are
  only prefix-validated.

Required implementation:

1. Parse `target_url` with URL semantics.
2. Disable redirects by default.
3. Deny loopback, link-local, private, multicast, and unspecified IP ranges.
4. Add optional allowlist configuration for enterprise webhook domains.
5. Resolve hostnames with IP pinning where feasible to reduce DNS rebinding.

Validation:

- URL parser and IP-range unit tests,
- redirect/internal target rejection tests,
- API validation tests for bad subscription URLs.

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

Risk:

- gateway has limited deployable readiness/lifecycle behavior,
- abrupt shutdown can disrupt sessions, recordings, and workflow workers.

Required implementation:

1. Add SIGINT/SIGTERM handling.
2. Add graceful shutdown to the API server.
3. Define WebTransport drain behavior:
   - stop accepting new connections,
   - allow bounded grace period,
   - finalize recordings/workflows where possible,
   - emit clear lifecycle logs.
4. Add `/healthz` for process liveness.
5. Add `/readyz` for dependency readiness:
   - Postgres when configured,
   - Docker/runtime manager,
   - Vault when credential provider is configured,
   - artifact stores.

Validation:

- readiness status composition tests,
- integration test for graceful shutdown path where feasible,
- compose smoke for `/healthz` and `/readyz`.

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

Validation:

- Postgres contract tests,
- API tests proving payload shape remains stable,
- compose API smoke with enough sessions/runs to catch regressions.

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

Required cleanup before old planning docs are removed:

1. Fix architecture docs where they conflict with implementation.
2. Add both admin apps to TypeScript/Svelte standards scope.
3. Add gateway configuration reference for important CLI flags.
4. Add a security/threat-model document with local-dev caveats.
5. Add OpenAPI examples/descriptions for high-use operations.
6. Update contributor guidance to create future planning documents in this
   consolidated location once the old folder is retired.
7. Add minimal CI for Rust fmt/clippy/tests, Node checks/tests/build, and
   targeted smoke wrappers.
