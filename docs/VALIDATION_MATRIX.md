# Consolidated Validation Matrix

Revalidated against current package scripts: 2026-07-31

This matrix keeps validation tied to the current `/admin-new` development state.

## Baseline Checks For Any Unified Admin Slice

Run in `code/web/bpane-admin-unified`:

```bash
npm run check
npm test
npm run build
```

Run in `code/web/bpane-admin` when behavior overlaps with the old app:

```bash
npm run check
npm test
npm run build
```

Run in `code/web/bpane-client` for browser/client/smoke-facing changes:

```bash
npx tsc --noEmit
npm test
npm run build
```

Use `npm run test:coverage` when the slice affects browser client behavior,
CLI behavior, or test coverage needs to be demonstrated.

## Dependency And Supply-Chain Floor

Issue `#151` must establish one reproducible local/CI dependency scan covering
`Cargo.lock` and every committed Node `package-lock.json`. The selected tooling
and wrapper command must be checked into the repository before being added to
the canonical command lists in `AGENTS.md` and `README.md`.

The validation policy must:

- reconcile local results with open Dependabot alerts,
- fail on unreviewed critical and high findings,
- distinguish runtime from development-only dependency exposure,
- remediate findings when a patched version is available,
- require documented scope, reachability, owner, and expiry for any temporary
  exception,
- retain the package-specific unit, build, integration, and smoke checks after
  lockfile updates.

The 2026-07-31 audit found one critical development-only advisory and multiple
high runtime advisories with patched versions available. Treat that as active
input to `#151`, not as accepted baseline debt.

## Current Admin-New Smoke Coverage

Run from `code/web/bpane-client` against local compose:

```bash
npm run smoke:admin-unified-dashboard -- --headless
npm run smoke:admin-unified-projects -- --headless
npm run smoke:admin-unified-browser-contexts -- --headless
npm run smoke:admin-unified-egress-profiles -- --headless
npm run smoke:admin-unified-file-workspaces -- --headless
npm run smoke:admin-unified-sessions -- --headless --connect-timeout-ms 60000
npm run smoke:admin-unified-workflows -- --headless
npm run smoke:admin-unified-workflow-runs -- --headless
```

## Broader Existing Client Smoke Matrix

The old and new admin apps coexist, so broader browser/client smokes must remain
runnable while promotion work continues. Run the relevant subset when a slice
touches the associated behavior, and run the full set before a promotion
decision:

```bash
npm run smoke:admin-session -- --headless
npm run smoke:admin-session-detail -- --headless
npm run smoke:admin-session-files -- --headless
npm run smoke:admin-recording -- --headless
npm run smoke:admin-workflow -- --headless
npm run smoke:admin-workflow-catalog -- --headless
npm run smoke:admin-workflow-run-detail -- --headless
npm run smoke:admin-browserpane-tour -- --headless
npm run smoke:admin-egress-profiles -- --headless
npm run smoke:admin-browser-contexts -- --headless
npm run smoke:admin-file-workspaces -- --headless
npm run smoke:admin-mcp -- --headless --connect-timeout-ms 60000
npm run smoke:admin-metrics -- --headless
npm run smoke:admin-realtime -- --headless
npm run smoke:admin-event-reconnect -- --headless
npm run smoke:automation-tasks -- --headless
npm run smoke:bpane-cli -- --headless
npm run smoke:browser-policy -- --headless
npm run smoke:file-workspaces -- --headless
npm run smoke:mcp-session-endpoints -- --headless --connect-timeout-ms 60000
npm run smoke:multisession -- --headless
npm run smoke:recording -- --headless
npm run smoke:session-files -- --headless
npm run smoke:test-embed-lifecycle -- --headless
npm run smoke:test-embed-overlay -- --headless
npm run smoke:workflow-admission -- --headless
npm run smoke:workflow-cancel -- --headless
npm run smoke:workflow-cli -- --headless
npm run smoke:workflow-credential-injection -- --headless
npm run smoke:workflow-credentials -- --headless
npm run smoke:workflow-embed -- --headless
npm run smoke:workflow-embed-operations -- --headless
npm run smoke:workflow-events -- --headless
npm run smoke:workflow-extension -- --headless
npm run smoke:workflow-failure -- --headless
npm run smoke:workflow-intervention -- --headless
npm run smoke:workflow-queued-cancel -- --headless
npm run smoke:workflow-reconnect -- --headless
npm run smoke:workflow-restart-safety -- --headless
npm run smoke:workflow-runtime-hold -- --headless
npm run smoke:workflow-workspace -- --headless
npm run smoke:workflows -- --headless
npm run workflow:cli -- --help
npm run test:coverage
```

Each migrated route should cover:

- unauthenticated or expired-auth redirect/logout behavior,
- validation errors,
- missing resources,
- conflict responses,
- backend unavailable responses,
- empty lists,
- loading states,
- disabled actions,
- destructive action feedback,
- upload/download failure feedback,
- no horizontal overflow on desktop and narrow mobile viewports.

## Old Admin Regression Smokes To Keep Until Promotion

Run from `code/web/bpane-client` when the touched area overlaps old `/admin/`:

```bash
npm run smoke:admin-session -- --headless
npm run smoke:admin-session-detail -- --headless
npm run smoke:admin-session-files -- --headless
npm run smoke:admin-recording -- --headless
npm run smoke:admin-workflow -- --headless
npm run smoke:admin-workflow-catalog -- --headless
npm run smoke:admin-workflow-run-detail -- --headless
npm run smoke:admin-mcp -- --headless --connect-timeout-ms 60000
npm run smoke:admin-realtime -- --headless
npm run smoke:admin-event-reconnect -- --headless
npm run smoke:admin-metrics -- --headless
```

## MCP-Specific Validation

Required for MCP delegation/control changes:

```bash
cd code/integrations/mcp-bridge
npm test
npm run build
```

```bash
cd code/web/bpane-client
npm run smoke:admin-mcp -- --headless --connect-timeout-ms 60000
npm run smoke:mcp-session-endpoints -- --headless --connect-timeout-ms 60000
npm run smoke:bpane-cli -- --headless --connect-timeout-ms 60000
```

Manual MCP check:

1. Open `/admin-new/sessions`.
2. Connect a session.
3. Authorize MCP.
4. Copy `/sessions/{session_id}/mcp`.
5. Connect a Streamable HTTP MCP client.
6. Run `tools/list`.
7. Call `browser_navigate`.
8. Verify the selected BrowserPane preview navigates.

## Recording-Specific Validation

Required for recording lifecycle or artifact-boundary changes:

```bash
cargo test -p bpane-gateway recordings
cargo test -p bpane-gateway --test compose_api_surface compose_recording_artifacts_and_playback_api_surface -- --ignored --test-threads=1
```

```bash
cd code/integrations/recording-worker
npm run build
```

```bash
cd code/web/bpane-client
npm run smoke:recording -- --headless --connect-timeout-ms 90000
npm run smoke:admin-recording -- --headless --connect-timeout-ms 90000
```

If unified admin recordings UI changes:

```bash
cd code/web/bpane-admin-unified
npm test -- Recording
```

Then manually verify `/admin-new/recordings` can list and download the expected
artifact.

## Workflow-Specific Validation

Required for workflow source, launch, runs, events, or produced files:

```bash
cargo test -p bpane-gateway workflow
```

```bash
cd code/integrations/workflow-worker
npm run build
```

```bash
cd code/web/bpane-client
npm run smoke:admin-unified-workflows -- --headless --connect-timeout-ms 90000
npm run smoke:admin-unified-workflow-runs -- --headless --connect-timeout-ms 90000
npm run smoke:workflow-cli -- --headless
npm run smoke:workflow-failure -- --headless
npm run smoke:workflow-runtime-hold -- --headless
```

For source hardening or source browser changes, include:

```bash
npm run smoke:workflow-workspace -- --headless
npm run smoke:admin-browserpane-tour -- --headless
```

BPM Workflow Endpoint issue `#172` additionally requires:

- endpoint/grant lifecycle and cross-project authorization tests,
- immutable endpoint revision, compatibility, environment promotion, rollback,
  and historical-run pinning tests,
- client-credentials service-principal invocation without an interactive owner
  token,
- concurrent idempotency tests proving one run and one browser side-effect
  path,
- JSON Schema dialect/schema/input/output validation, including rejection
  before session/worker creation,
- RFC 9457 request problems and typed business/technical/policy/timeout
  outcomes,
- gateway-enforced queue/execution/Human Handoff deadlines,
- endpoint/caller concurrency, rate-limit, accepted-queue, `429`, `503`,
  `Retry-After`, degraded, maintenance, and dependency-readiness tests,
- progress heartbeat, stale worker, cancellation request, acknowledgement, and
  terminal-state tests,
- attempt/checkpoint and side-effect uncertainty tests that prevent unsafe
  whole-run retry assumptions,
- declared process-variable mapping and strict separation of integration
  credentials from target-system Credential Bindings,
- external-managed and BrowserPane-managed Human Handoff tests proving exactly
  one task owner per endpoint revision,
- W3C Trace Context continuity through gateway, worker, event, log, artifact,
  and callback evidence,
- CloudEvents/AsyncAPI schema, signing, retry, replay, redelivery, secret
  rotation, cursor, and receiver-deduplication tests,
- Postgres transactional run/event/delivery persistence, supported-store
  parity, per-run sequence, replay, reorder, duplicate, and reconciliation
  tests,
- inline-result size and artifact checksum/media-type/authorization/expiry
  tests,
- polling, signed webhook, and callback-token completion-profile tests,
- canonical OpenAPI and generated compatibility-export drift tests,
- proof that callback tokens and connector credentials never enter labels,
  logs, events, diagnostics, or UI,
- Admin-New, CLI, raw API, reference connector, durable-activity wrapper, and
  deterministic fake-orchestrator conformance smokes.

Do not count successful owner-token `POST /api/v1/workflow-runs` coverage as
Workflow Endpoint validation. The smoke must use the stable endpoint key and an
explicit endpoint caller grant.

Teach Mode issue `#171` additionally requires:

- lifecycle and authorization tests for training drafts, demonstrations,
  candidates, scenarios, reviews, and controlled repairs,
- semantic trace normalization and selector-ranking tests,
- secret-redaction tests covering passwords, cookies, authorization headers,
  TOTP values, and compiler diagnostics,
- compiler input-minimization and external-provider policy-denial tests,
- fresh-context positive and negative replay,
- proof that failed validation blocks publication,
- proof that controlled repair creates a candidate lineage without mutating the
  published workflow version,
- Admin-New route/component coverage plus a deterministic local Teach Mode
  smoke fixture.

Do not count a successful video recording as Teach Mode validation. The smoke
must inspect semantic steps, annotations, generated source/schemas, replay
results, provenance, and the immutable publication gate.

## Gateway/API Safety Checks

For backend API/security/runtime changes:

```bash
cargo test --workspace
cargo test -p bpane-gateway
cargo test -p bpane-gateway --test compose_api_surface <target_test_name> -- --ignored --test-threads=1
```

For route/API contract changes, check:

- `openapi/bpane-control-v1.yaml`
- `README.md`
- `ARCH.md`
- `AGENTS.md`

Only update those docs when behavior, topology, setup, API, or validation flow
actually changes.

## Runtime, CLI, Identity, And Resource Lifecycle Checks

Use these focused checks for slices that touch the older platform domains now
captured in the standalone requirement docs:

Runtime and local workflow:

```bash
cargo test -p bpane-gateway
cd code/integrations/mcp-bridge && npm test && npm run build
cd code/web/bpane-client && npm run smoke:admin-browserpane-tour -- --headless
cd code/web/bpane-client && npm run smoke:admin-mcp -- --headless
```

Operator CLI:

```bash
cd code/web/bpane-client
npm test -- bpane-cli
npm run smoke:bpane-cli -- --headless
npm run bpane:cli -- --help
```

Browser contexts and resource lifecycle:

```bash
cargo test -p bpane-gateway browser_context
cd code/web/bpane-client && npm run smoke:admin-unified-browser-contexts -- --headless
cd code/web/bpane-client && npm run smoke:admin-browser-contexts -- --headless
```

Network identity and egress:

```bash
cargo test -p bpane-gateway api::tests::network_identity -- --nocapture
cargo test -p bpane-gateway session_control::tests::validation -- --nocapture
cd code/web/bpane-client && npm run smoke:admin-unified-egress-profiles -- --headless
cd code/web/bpane-client && npm run smoke:admin-egress-profiles -- --headless
```

Identity and access:

```bash
cargo test -p bpane-gateway identity -- --nocapture
cargo test -p bpane-gateway identity_mapping -- --nocapture
cargo test -p bpane-gateway service_principal -- --nocapture
cd code/web/bpane-client && npm run smoke:bpane-cli -- --headless
```

## Review-Derived Validation Additions

Use these checks when implementing items reconciled from `review/`:

Token and URL credential cleanup:

- connect tickets must fail automation-token validation and automation tokens
  must fail connect-ticket validation,
- malformed, expired, and wrong-purpose tokens must fail deterministically,
- transport warning logs must not contain raw `token`, `access_token`, or
  `session_ticket` query values,
- admin event clients must not place owner bearer tokens in WebSocket query
  strings after the replacement auth path lands.

Admin browser auth and web security:

- old and unified admin auth tests must cover OIDC nonce creation/validation,
  wrong-state/wrong-nonce rejection, refresh, logout, and expired-auth
  behavior,
- ID-token display claims must come from verified token claims or a clearly
  server-validated source,
- nginx/static serving tests or smoke checks must verify CSP and security
  headers.

Webhook, import, and lifecycle:

- webhook target validation must reject loopback, link-local, private,
  multicast, unspecified IPs, and redirect-to-internal cases,
- browser-context import tests must cover body/profile limits, ZIP entry count,
  symlink/hardlink tar entries, duplicate profile archives, and manifest
  errors,
- graceful shutdown tests should prove SIGINT/SIGTERM stops new work, drains
  bounded in-flight work, and exposes readiness/lifecycle state.

Scalability and performance:

- seed enough sessions, workflow runs, queued sessions, identity mappings, and
  admin event subscribers to catch O(N^2) or N+1 query regressions,
- include `/api/v1/sessions`, `/api/v1/identity/access-review`, dashboard
  snapshots, and admin-event snapshots in query-count or latency checks,
- update ARCH.md accuracy checks when capture, tile cache, or render behavior
  changes.

## Manual Promotion Gate

Before `/admin-new` can become default:

1. Verify every visible navigation route exists or is intentionally hidden.
2. Run all current old-admin smokes for migrated behavior.
3. Run all admin-new smokes.
4. Manually test:
   - create/connect/disconnect/reconnect session,
   - preview popup resize and metrics,
   - MCP delegation and real MCP tool call,
   - recording enablement and download,
   - workflow launch and run inspection,
   - file workspace upload/download,
   - egress profile edit/probe,
   - identity/access review once implemented.
5. Keep `/admin/` as fallback until a dated removal gate is accepted.
