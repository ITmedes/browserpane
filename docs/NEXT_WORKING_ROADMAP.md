# Consolidated Next Working Plan

This plan consolidates the current state from the admin redesign requirements
and the security/runtime cleanup roadmap.

## Working Principle

Do not promote `/admin-new/` to the default admin console until:

1. the merged status-quo baseline remains stable,
2. advertised navigation routes exist or are hidden,
3. route-backed session/workflow detail gaps are closed,
4. the highest-risk cleanup/security items that affect admin trust are
   addressed,
5. old `/admin/` parity remains covered by tests or explicit deferral.

## Immediate State

Merged PR `#143` is the current status checkpoint:

- snapshots the unified admin progress,
- documents `/admin-new/` as under active development,
- includes workflow source hardening,
- includes MCP bridge control hardening,
- keeps `/admin/` as stable/default,
- references but does not close `#142`.

## Recommended Next Slices

Review reconciliation update:

- workflow-source RCE/preview-symlink and bridge-local control-auth findings
  from `review/` are treated as superseded by the current baseline,
- token-domain separation, raw credential URL/log cleanup, admin-event query
  auth, browser admin auth/CSP, webhook SSRF, browser-context import limits,
  and graceful shutdown remain confirmed-open,
- recording artifact finalization remains a valid cleanup item, but it should
  not displace confirmed-open token/auth/webhook/import/lifecycle findings when
  choosing the next security-driven slice.

### Slice A: Token Domain Separation And URL Credential Cleanup

Priority: high security cleanup before admin-new promotion.

Why:

- Connect tickets and automation tokens must not be interchangeable.
- WebTransport and admin-event paths should not log or transport raw owner
  bearer tokens in URLs.
- The current admin event stream uses owner bearer query auth for browser
  compatibility.
- This affects both admin apps and must be solved once, not duplicated.

Scope:

1. Create or confirm a canonical issue.
2. Add or update a focused implementation document in the consolidated planning
   workspace before coding starts.
3. Add token purpose/audience separation or distinct signing keys.
4. Use versioned token parsing so old/wrong-purpose tokens fail safely.
5. Use constant-time HMAC verification.
6. Redact query strings and sanitize transport warning paths.
7. Replace admin-event `access_token` query auth with a scoped event-stream
   credential or a browser-compatible alternative.
8. Update old admin and unified admin event consumers together.

Validation:

- token validation unit tests for wrong-purpose rejection,
- malformed/expired/wrong-purpose token tests,
- constant-time signature validation or library-backed verification tests,
- transport log redaction tests,
- admin event auth tests proving owner bearers are not placed in WebSocket
  query strings,
- old and new admin realtime/event smokes.

### Slice B: Admin Browser Auth And Web Security

Priority: high before default admin promotion.

Why:

- The old and unified admin apps both own security-sensitive browser auth code.
- OIDC nonce, ID-token verification, token persistence, and CSP/security
  headers are browser-facing trust boundaries.
- Solving this once reduces the risk of duplicated auth fixes across two apps.

Scope:

1. Extract or share admin auth implementation where practical.
2. Add OIDC nonce generation, storage, and validation.
3. Verify ID-token signatures/issuer/audience before using ID-token claims.
4. Reduce JavaScript-readable refresh-token exposure.
5. Add CSP and standard static-serving security headers.
6. Keep local demo credentials documented as local-only.

Validation:

- old and unified admin auth unit tests,
- login/logout/refresh/expired-auth smokes,
- negative nonce and ID-token tests,
- security-header checks.

### Slice C: Recording Artifact Finalization Boundary

Priority: medium/high security/runtime cleanup.

Why:

- The recording worker can complete a recording with a gateway-local
  `source_path`.
- Automation access currently reaches that route.
- The artifact store validates absolute path shape but must enforce a trusted
  staging boundary.
- This affects admin trust because `/admin-new/recordings` exposes artifacts to
  operators.

Scope:

1. Create or confirm a canonical issue.
2. Add or update a focused implementation document in the consolidated planning
   workspace before coding starts.
3. Enforce that completion paths are under the configured recording staging
   root.
4. Canonicalize and reject symlinks, directories, relative paths, path escapes,
   and wrong session/recording directories.
5. Consider opaque staging artifact ids instead of raw absolute paths if the API
   can be changed cleanly.
6. Keep existing ready-recording download/export behavior unchanged.

Validation:

- Rust unit tests for staging-path validation.
- API tests for outside-root, symlink, directory, wrong-session, relative-path,
  and valid staged-file cases.
- recording-worker build/test where applicable.
- compose/browser recording smoke proving a downloadable WebM/export still
  works.
- unified admin recording catalog smoke after backend changes.

### Slice D: Route-Backed Workflow Run Detail

Priority: admin-new parity.

Why:

- The old admin has workflow-run detail behavior.
- `/admin-new/runs` is only an overview.
- Operators need logs, events, produced files, run controls, intervention
  state, and related session links before `/admin-new` can replace `/admin/`.

Scope:

1. Add `/admin-new/runs/[run_id]`.
2. Add `/admin-new/workflow-runs` alias or redirect if the route naming plan
   still requires it.
3. Show run metadata, state, timestamps, project/session/workflow references,
   logs, events, outputs, produced files, and errors.
4. Add safe controls for cancel, resume, reject, and input submit where the API
   supports them.
5. Link to the related session detail/preview.

Validation:

- workflow-run client/view-model/component tests,
- unified workflow-runs smoke extended to open detail and exercise safe
  read-only assertions,
- old admin workflow-run-detail smoke as regression.

### Slice E: Route-Backed Session Subareas

Priority: admin-new parity.

Why:

- The current session detail route aggregates too much.
- The redesign plan explicitly calls for route-backed session areas.
- This is needed before the new app is ergonomic enough to replace the old app.

Scope:

1. `/admin-new/sessions/[session_id]/live` or keep preview popup and document
   the replacement.
2. `/files` for session files and file bindings.
3. `/recordings` for session recording segments/playback/export.
4. `/network` for effective network identity, egress diagnostics, probes.
5. `/automation` for MCP delegation, workflow associations, automation owner.
6. `/policy` for capabilities and browser policy evidence.
7. `/observability` for logs, metrics, snapshots, admin event state.

Validation:

- route-level component tests,
- unified sessions smoke extended to cover tab/subroute navigation,
- old admin session/detail/files/recording/mcp/metrics smokes as regression.

### Slice F: Identity And Access Review Route

Priority: admin-new enterprise parity.

Why:

- Backend identity/access-review and service-principal surfaces exist.
- The unified navigation already advertises `Identity & access`.
- Operators need current principal, mapped projects, service principals,
  identity mappings, delegated principals, and unmapped signals.

Scope:

1. Add `/admin-new/identity`.
2. Start read-only if CRUD is too broad for the first slice.
3. Include service-principal and mapping management as follow-up or within the
   same route if contained.

Validation:

- identity client/view-model/component tests,
- smoke for current principal/access-review,
- old admin identity/access-review behavior where available as regression.

### Slice G: API Companion And Coverage Routes

Priority: admin-new completion/documentation.

Why:

- Navigation advertises API docs/coverage.
- OpenAPI coverage has already been audited.
- Operators and integrators need copyable examples and clear separation between
  owner APIs, worker/internal APIs, and compatibility endpoints.

Scope:

1. Add `/admin-new/api`.
2. Add `/admin-new/coverage` or remove/hide nav until implemented.
3. Surface OpenAPI classifications.
4. Keep compatibility endpoints separated from frozen v1 API.

Validation:

- route/component tests,
- smoke that routes load and expose expected operation families,
- docs link checks.

## Do Not Do In The Next PR

- Do not delete the old admin app.
- Do not switch `/admin/` to the unified app.
- Do not mix enterprise roadmap items like DLP, BYOK, HA, or support bundles
  into an admin-new parity slice.
- Do not refactor all repeated admin components unless the slice directly
  benefits from it.

## Subsequent Product/Enterprise Backlog

Keep these as later roadmap work, not as blockers for the immediate
`/admin-new` parity slices:

- productized automation descriptors with attach/revoke/audit semantics,
- unified artifact/browser-output/recording-export metadata,
- generalized resource event subscriptions and security event export,
- enterprise API keys, immutable audit log, and retention policy controls,
- signed human handoff, challenge detection, and private fallback resources,
- debug/support bundle export with redaction,
- production Compose/Kubernetes/Fargate deployment packaging,
- backup/restore and disaster recovery drills,
- enterprise threat model and security baseline,
- supply-chain security and release governance,
- data residency, encryption, and BYOK,
- DLP/content-inspection hooks,
- central enterprise policy engine.
