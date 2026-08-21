# Consolidated Validation Matrix

Revalidated against current package scripts: 2026-08-16

This matrix defines the available validation surfaces for product slices. Use
`PRODUCT_PHASES_AND_RELEASE_GATES.md` to decide which evidence is required for
a Foundation, Phase 0, Phase 1, Production, or Phase N claim. Admin-new remains
one important surface, but it is not the only delivery lane.

## Verified Baseline And Gaps

The current coverage ratchet records:

- all non-ignored Rust workspace tests passed under `cargo llvm-cov`, with
  54.88% line coverage on the canonical pinned Ubuntu runner and 57.58% on the
  current local macOS run; the cross-platform floor is 54.8%,
- browser-client tests pass on pinned Node 22/Linux with 92.88% lines, 92.88%
  statements, 93.19% functions, and 87.58% branches across all maintained `js`
  sources; the cross-platform function floor is 92.9%,
- admin-new's 620 tests pass with 90.43% lines, 92.28% statements, 93.86%
  functions, and 76.76% branches across `src/lib`,
- MCP bridge has focused unit tests,
- recording-worker has 13 package tests and workflow-worker has 19 package
  tests covering finite request deadlines, parent cancellation, OIDC refresh
  coalescing, bounded output, UTF-8 truncation, and single-flight recording
  finalization polling; both packages run tests before builds in the canonical
  fast validation and hosted validation jobs,
- compose and docker-pool tests remain ignored unless the supported stack is
  explicitly started,
- Slice 1 of #151 now provides a repository dependency scanner and an expiring
  exception policy.
- Slice 2 of #151 provides one local validation runner. Slice 3 adds checked-in
  Rust, browser-client, and admin-new coverage floors plus pinned,
  least-privilege fast and bounded compose GitHub Actions workflows. Hosted
  fast execution passes the required BrowserPane checks, and `main` requires
  them in strict mode as GitHub Actions checks.

This is meaningful Prototype evidence, not a Production gate. #151 owns the
enforced baseline; #165 adds the worker test floor and bounded runtime hygiene.

## Canonical Local Runner

```bash
node scripts/validate.mjs --profile fast
node scripts/validate.mjs --profile compose
node scripts/validate.mjs --profile full
node scripts/check-repository-documents.mjs
node scripts/check-production-security-baseline.mjs
node scripts/observability/validate-prometheus-rules.mjs
node scripts/observability/validate-grafana-dashboard.mjs
```

- `fast` is the clean, non-compose validation floor.
- `compose` is the explicit bounded API/admin/CLI/MCP/recording/workflow path.
- `full` runs `fast` followed by `compose`.
- `--list`, `--dry-run`, and repeatable `--stage <id>` selections expose the
  exact stage catalog without duplicating package-owned commands.

The runner stops on the first failure, preserves its exit code, reports the
stage-specific rerun command, enforces per-stage timeouts, and terminates the
active child process on interrupt. The compose profile may build or start the
local stack and leaves it running for inspection. Smokes that temporarily
change gateway admission limits restore the normal compose configuration before
returning.

Verified catalog on 2026-08-14:

- the fast profile contains 44 stages, including the composed production
  security baseline plus Rust, browser-client,
  recording-worker, workflow-worker, and admin-new coverage ratchets,
- the compose profile contains 35 bounded stages, including 24 admin promotion
  stages plus Prometheus rule, Grafana dashboard, API, CLI, MCP, recording,
  session-file, and workflow evidence,
- the compose gateway stage passes 17 default API and four docker-pool cases,
- representative admin-new, compatibility-admin, CLI, MCP, recording, and
  workflow admission journeys pass against the running stack,
- issue `#162` promotion evidence additionally passes the named session-file,
  workflow CLI, workflow workspace, and workflow event stages; the workspace
  fixture passes repeatedly against retained Postgres metadata.

The fast GitHub workflow exposes stable repository, dependency, Rust, and
per-package Node checks. It parses all committed YAML, validates local Markdown
links, and enforces immutable action revisions, least-privilege permissions,
fixed runner images, job timeouts, lockfile-derived cache keys, and bounded
artifact paths. `main` branch protection requires those nine BrowserPane jobs
in strict mode and binds them to the GitHub Actions app.

The `Compose` workflow runs on pushes to `main`, a weekday schedule, and manual
dispatch. It is not a pull-request gate until hosted-runner reliability is
demonstrated. Split gateway, browser-integration, unified-admin, and
compatibility-admin jobs execute the canonical evidence journeys, capture only
selected control-plane status/log tails after a failure, redact credential and
identity material before upload, and always remove BrowserPane containers and
compose volumes.

## Docker Runtime Topology Evidence

The canonical Compose profile preserves the direct `docker_pool` compatibility
path. Validate that path and its API allowlist with:

```bash
node scripts/validate-docker-runtime-boundary.mjs
scripts/run-gateway-compose-e2e.sh --suite docker-pool
```

The production-like Docker-host path uses `broker_pool`. Start and validate the
gateway-isolated topology with:

```bash
./scripts/start-runtime-broker-browser-overlay.sh
node scripts/validate-runtime-broker-browser-overlay.mjs
./scripts/smoke-runtime-broker-isolation.sh
./scripts/smoke-runtime-broker-storage.sh
cd code/web/bpane-client
npm run smoke:runtime-broker-restart -- --headless
```

The isolation smoke must prove that the running gateway has no Docker endpoint,
socket, proxy dependency, or Docker-control network membership; cannot resolve
or connect to `docker-proxy`; and can still reach the runtime broker. The
restart smoke must preserve the same browser container across a broker restart,
avoid a duplicate runtime, retain lifecycle/automation access, and clean the
container on stop.

Before closing #214, also run the 17-case Compose API surface and the browser,
MCP, admin-new session, session-file, workflow, recording, and multi-session
journeys against the broker topology. Run the four-case docker-pool suite
separately to preserve direct local compatibility evidence.

## Single-Node Deployment Baseline

Issue #225 adds an independent broker-only package rather than extending local
Compose. Its static and test fixture checks are:

```bash
node scripts/check-single-node-deployment.mjs --repository-fixture
node --test scripts/single-node/single-node-deployment.test.mjs
node --test deploy/single-node/render-config.test.mjs
node --test scripts/single-node/single-node-workflow-fixture.test.mjs
```

The repository fixture renders synthetic configuration into an isolated
temporary directory. Static validation must not overwrite
`deploy/single-node/generated`, because that directory can be mounted by a
running qualification deployment.

Run the live qualification against the current pushed branch and commit:

```bash
./scripts/start-single-node-fixture.sh
node scripts/qualify-single-node-deployment.mjs
./scripts/stop-single-node-fixture.sh
```

The live qualifier must prove dependency readiness, two distinct browser
runtimes, pinned workflow execution, produced-file retention across gateway
restart, no duplicate runtime, gateway Docker denial, secret-marker redaction,
worker credentials absent from inspectable env/command/files, and a nonempty
recording plus playback export. It is repository qualification evidence, not a
target load, restore, HA, network-policy, or compliance test. The operating
boundary and target obligations are in `SINGLE_NODE_DEPLOYMENT.md`.

## Production Security Baseline

Issue #223 composes the existing application, admin-header, Docker-proxy, and
runtime-broker controls into one discoverable fast-profile stage:

```bash
node scripts/check-production-security-baseline.mjs
node --test scripts/security/*.test.mjs
node --test scripts/validate-runtime-broker-foundation.test.mjs
node --test scripts/validate-runtime-broker-browser-overlay.test.mjs
```

The `production-security-baseline` stage parses the real base Compose and
runtime-broker overlay through `docker compose ... config --format json`, and
the fast profile separately runs the single-node structured preflight. Together
they distinguish local development, broker validation, and bounded single-node
packaging; prove that the gateway has no Docker authority in broker profiles;
keep broker/proxy listeners and process posture constrained; enforce protected
secret inputs and immutable images; and retain the admin browser-header
contract.

Every new static invariant needs a failing fixture. Static success is not live
deployment evidence: security-sensitive runtime changes must also run
`scripts/smoke-runtime-broker-isolation.sh` plus the affected owner API,
admin-new, MCP, workflow, recording, and storage journeys. The negative evidence
inventory and residual owners live in `THREAT_MODEL.md`; required deployment
controls live in `PRODUCTION_SECURITY_BASELINE.md`.

## Baseline Checks For Any Unified Admin Slice

Run in `code/web/bpane-admin-unified`:

```bash
npm run check
npm test
npm run test:coverage
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

The browser-client and admin-new `test:coverage` scripts enforce the floors in
`quality/coverage-baselines.json` and write human-readable summaries below
`test-results/coverage/`. Rust uses:

```bash
cargo install cargo-llvm-cov --version 0.8.7 --locked
node scripts/run-rust-coverage.mjs
```

These floors are regression ratchets, not a claim that all critical behavior
has adequate test depth. Raise them with added tests; lowering one requires an
explicitly reviewed rationale.

## Dependency And Supply-Chain Floor

Slice 1 of issue `#151` establishes the local dependency scan covering
`Cargo.lock` and every committed Node `package-lock.json`:

```bash
cargo install cargo-audit --locked
node scripts/check-dependency-safety.mjs
node --test scripts/dependency-safety/*.test.mjs
```

The local validation runner composes this command into its fast profile. The
remaining #151 slices must run the same policy in required CI checks without
weakening its failure behavior.

The validation policy must:

- reconcile local results with open Dependabot alerts,
- fail on unreviewed critical and high findings,
- distinguish runtime from development-only dependency exposure,
- remediate findings when a patched version is available,
- require documented scope, reachability, owner, and expiry for any temporary
  exception,
- retain the package-specific unit, build, integration, and smoke checks after
  lockfile updates,
- establish coverage baselines and fail unexplained regressions,
- run as GitHub Actions checks required by branch protection,
- prove each major stage fails visibly through controlled fixtures.

The 2026-08-03 live Dependabot inventory contains one critical and 24 high
alerts against the default branch. The Slice 1 lock updates remove all local
npm critical/high findings and all patched RustSec findings. One medium,
no-fix RSA advisory remains only in SQLx's disabled optional MySQL graph and is
covered by the exact, expiring exception in
`security/dependency-exceptions.json`; it is not accepted silently.

#151 is in hosted-remediation review under
`BPANE-00151_MINIMAL_CI_VALIDATION_PLAN.md`; keep its measured baselines and
final required-check names aligned with this matrix.

## Public Contract And Protocol Floor

Issue #179 adds OpenAPI lint, implementation conformance, executable examples,
and breaking-change detection to the #151 validation baseline. Run the focused
contract floor with:

```bash
npm ci --ignore-scripts --prefix scripts/openapi
npm test --prefix scripts/openapi
npm run check --prefix scripts/openapi
npm run compatibility --prefix scripts/openapi -- --base-ref origin/main
cargo test -p bpane-gateway openapi_contract
```

The OpenAPI package validates the canonical document, the complete generated
operation inventory, one classification per operation, representative success
and error examples, and semantic compatibility. The Rust contract proves that
every documented path and method is registered by the in-memory Axum router.

Issue #175 owns the BrowserPane remote protocol specification, version/
capability negotiation, shared Rust/TypeScript vectors, malformed-input tests,
and fuzzing. Until that issue passes its gate, successful current-client
connection smokes do not constitute a broad compatibility promise.

## Current Admin-New Smoke Coverage

Run from `code/web/bpane-client` against local compose:

```bash
npm run smoke:admin-unified-dashboard -- --headless
npm run smoke:admin-unified-promotion -- --headless --connect-timeout-ms 60000
npm run smoke:admin-unified-projects -- --headless
npm run smoke:admin-unified-browser-contexts -- --headless
npm run smoke:admin-unified-egress-profiles -- --headless
npm run smoke:admin-unified-file-workspaces -- --headless
npm run smoke:admin-unified-sessions -- --headless --connect-timeout-ms 60000
npm run smoke:admin-unified-workflows -- --headless
npm run smoke:admin-unified-workflow-runs -- --headless
npm run smoke:admin-unified-identity -- --headless
npm run smoke:admin-unified-recordings -- --headless
npm run smoke:admin-unified-resource-catalogs -- --headless --connect-timeout-ms 60000
npm run smoke:admin-unified-api-companion -- --headless --connect-timeout-ms 60000
```

The unified sessions smoke opens and reloads the canonical overview, live,
files, recordings, and network routes, validates active navigation and
responsive layout, switches each subarea to a second session with distinct
network evidence, then exercises MCP delegation and the popup browser
connection lifecycle.

The canonical operator CLI smoke covers profile permissions, identity,
governed resource lifecycles, exact binary workspace transfer, session
create/status/access/disconnect/release/stop/kill/cleanup, and MCP
doctor/preflight/repair. The API-family boundary and local diagnostic sequence
are maintained in `OPERATOR_CLI_AND_LOCAL_DIAGNOSTICS.md`.

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
../../../scripts/bpane workflow --help
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
npm test
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
npm test
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
- client-credentials service-principal invocation without an interactive owner
  token,
- concurrent idempotency tests proving one run and one browser side-effect
  path,
- JSON Schema dialect/schema/input/output validation, including rejection
  before session/worker creation,
- RFC 9457 request problems and typed business/technical/policy/timeout
  outcomes, including `external_intervention_required`,
- gateway-enforced execution timeout, cancellation, and terminal-state tests,
- side-effect certainty tests that prevent unsafe whole-run retry assumptions,
- declared process-variable mapping and strict separation of integration
  credentials from target-system Credential Bindings,
- inline-result size and artifact checksum/media-type/authorization/expiry
  tests,
- polling, canonical OpenAPI, Admin-New, CLI, and deterministic
  fake-orchestrator conformance smokes.

The focused local commands for that bounded contract are:

```bash
cargo test -p bpane-gateway workflow_endpoints --no-fail-fast
cargo test -p bpane-gateway api::tests::workflow_endpoints --no-fail-fast
cargo test -p bpane-gateway session_store_contract_in_memory
BPANE_SESSION_STORE_CONTRACT_POSTGRES_URL=postgresql://browserpane:browserpane-dev@localhost:5433/browserpane \
  cargo test -p bpane-gateway session_store_contract_postgres -- --ignored --test-threads=1
npm test --prefix scripts/openapi
npm run check --prefix scripts/openapi
npm run compatibility --prefix scripts/openapi -- --base-ref origin/main
cd code/web/bpane-admin-unified && npm run check && npm run test:coverage && npm run build
cd code/web/bpane-client && npx vitest run \
  js/__tests__/bpane-cli.test.ts \
  js/__tests__/workflow-endpoint-conformance.test.ts
cd code/web/bpane-client && npm run smoke:workflow-endpoint-compose -- \
  --headless --connect-timeout-ms 120000
```

The last command is the real-stack gate: it uses the imported local Keycloak
confidential caller, Postgres persistence, gateway-launched workflow worker,
docker-backed browser session, and fake-BPM polling fixture. A mocked token,
in-memory store, or owner-token `POST /api/v1/workflow-runs` is not equivalent.

Deferred Workflow Endpoint productization issue `#240` additionally requires:

- immutable endpoint revision, compatibility, environment promotion, rollback,
  and historical-run pinning tests,
- endpoint/caller concurrency, rate-limit, accepted-queue, `429`, `503`,
  `Retry-After`, degraded, maintenance, and dependency-readiness tests,
- progress heartbeat, stale worker, cancellation acknowledgement, attempt, and
  checkpoint evidence,
- W3C Trace Context continuity through gateway, worker, event, log, artifact,
  and callback evidence,
- CloudEvents/AsyncAPI schema, signing, retry, replay, redelivery, secret
  rotation, cursor, and receiver-deduplication tests,
- Postgres transactional run/event/delivery persistence, supported-store
  parity, per-run sequence, replay, reorder, duplicate, and reconciliation
  tests,
- signed webhook and callback-token completion-profile tests,
- canonical OpenAPI and generated compatibility-export drift tests,
- proof that callback tokens and connector credentials never enter labels,
  logs, events, diagnostics, or UI,
- Admin-New, CLI, raw API, reference connector, and durable-activity wrapper
  conformance smokes.

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

For session-control persistence changes, run the identical store contract
against the in-memory reference and a migrated, schema-isolated Postgres store:

```bash
cargo test -p bpane-gateway session_store_contract_in_memory
docker compose -f deploy/compose.yml up -d postgres
BPANE_SESSION_STORE_CONTRACT_POSTGRES_URL=postgresql://browserpane:browserpane-dev@localhost:5433/browserpane cargo test -p bpane-gateway session_store_contract_postgres -- --ignored --test-threads=1
```

The Postgres fixture creates a unique `bpane_store_contract_*` schema, runs the
normal gateway migrations there, and drops it after the contract completes.
`scripts/run-gateway-compose-e2e.sh` runs this contract automatically for every
non-stack suite, so the full local validator and hosted Compose workflow retain
persisted-store parity coverage.

For route/API contract changes, check:

- `openapi/bpane-control-v1.yaml`
- `README.md`
- `ARCH.md`
- `AGENTS.md`

Only update those docs when behavior, topology, setup, API, or validation flow
actually changes.

Platform telemetry changes must additionally prove:

- valid OpenMetrics content type and `# EOF` framing,
- bounded method, matched-route, and status-class labels,
- no owner, project, session, workflow, recording, artifact, raw-path, URL,
  credential, browser-content, or egress values in labels,
- in-flight gauge release on normal completion and cancellation,
- aggregate active/starting/limit runtime transitions for static and pooled
  backends,
- every scoped workflow/recording counter has stable HELP/TYPE metadata, an
  integer `_total` sample, and no labels,
- authenticated workflow/recording operations snapshots and `/metrics` read
  the same counter instances without duplicate increments,
- representative workflow upload/callback and recording finalize/playback
  smokes advance the expected counters and exported-byte total,
- scraping has no readiness, session, or runtime lifecycle side effects.

For Prometheus SLI/alert changes, additionally prove:

- upstream `promtool` accepts the scrape configuration and every rule file,
- deterministic rule tests cover healthy, firing, hold, recovery, no-traffic,
  process-counter-reset, and target-absent behavior,
- recording and alert expressions reference only shipped stable metrics,
- every alert has bounded severity/subsystem metadata and a resolving runbook
  heading,
- rule labels/annotations contain no dynamic resource or sensitive values,
- thresholds are documented as initial proposals rather than contractual SLOs.

Run:

```bash
node --test scripts/observability/prometheus-rules-contract.test.mjs
node scripts/validate.mjs --stage observability-prometheus-rules
```

The static contract is part of `validation-tool-tests`. The semantic stage is
compose-class because it executes the digest-pinned upstream Prometheus image;
the required Repository Metadata GitHub job invokes it explicitly.

For Grafana operations-dashboard changes, additionally prove:

- datasource and dashboard provisioning use stable UIDs and remain
  repository-authoritative,
- all 20 panels have stable ids, non-overlapping bounded geometry, descriptions,
  units, no-data semantics, and approved aggregate queries,
- no variables, dynamic annotations, resource selectors, embedded credentials,
  public ports, or sensitive query/legend/link fragments are present,
- the digest-pinned Grafana image provisions the dashboard and private
  Prometheus datasource without manual import,
- all 19 query panels execute through Grafana's authenticated datasource API,
- anonymous access, sign-up, plugin discovery, and plugin auto-update remain
  disabled, and the administrator password is operator-supplied,
- live validation detects error-level startup logs and cleans temporary
  containers, networks, and files on success or failure.

Run:

```bash
node --test scripts/observability/grafana-*.test.mjs
node scripts/validate.mjs --stage observability-grafana-dashboard
```

The static contracts are part of `validation-tool-tests`. The live stage is
compose-class because it creates an isolated private network and executes the
digest-pinned Prometheus and Grafana images; the required Repository Metadata
GitHub job invokes it explicitly.

For OpenTelemetry runtime-tracing changes, additionally prove:

- valid W3C caller-context continuation plus absent/malformed fallback,
- gateway client and broker server/auth/policy/runtime parentage in one trace,
- fixed span names and allowlisted low-cardinality attributes only,
- no credentials, resource identifiers, labels, URLs, baggage, browser content,
  source/thread metadata, log events, or raw errors in exported evidence,
- no public response reflection of `traceparent` or `tracestate`,
- redacted startup failure for invalid explicit configuration,
- bounded, non-blocking behavior during collector outage and export recovery,
- collector isolation from public/owner networks in the qualification fixture.

Run:

```bash
cargo test -p bpane-telemetry
node --test scripts/runtime-tracing/*.test.mjs
node scripts/validate-runtime-tracing-fixture.mjs
./scripts/start-single-node-fixture.sh
node scripts/smoke-runtime-tracing.mjs
```

The default Compose health/readiness case exercises the real `/metrics` surface,
including degraded readiness, unmatched-path redaction, and the complete
label-free workflow/recording counter catalog. The workflow event-delivery case
checks real upload/callback counter movement; the recording browser smoke checks
finalize/playback/export-byte movement. The docker-pool capacity case verifies
aggregate gauges while two, one, and zero runtimes are active and checks that
live session ids are absent.

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

Issue #145 evidence recorded on 2026-08-04:

- `cargo test -p bpane-gateway`: 370 non-ignored tests passed,
- compatibility admin: check, 203 tests, and production build passed,
- admin-new: check, 279 tests, coverage baseline, and production build passed,
- browser client: typecheck, 661 tests, coverage baseline, and build passed,
- `scripts/run-gateway-compose-e2e.sh --suite all`: 16 compose API tests and 4
  docker-pool lifecycle tests passed,
- `smoke:admin-event-reconnect`, `smoke:admin-session`, and
  `smoke:mcp-session-endpoints` passed against the rebuilt local stack,
- repository document and OpenAPI YAML checks passed.

Admin browser auth and web security:

- old and unified admin auth tests must cover OIDC nonce creation/validation,
  wrong-state/wrong-nonce rejection, refresh, logout, and expired-auth
  behavior,
- ID-token display claims must come from verified token claims or a clearly
  server-validated source,
- nginx/static serving tests or smoke checks must verify CSP and security
  headers.

Current #146 evidence:

- shared auth: 4 files/30 tests, coverage ratchet, clean production dependency
  audit,
- compatibility admin: 45 files/200 tests, typecheck, and production build,
- admin-new: 101 files/278 tests, typecheck, and production build,
- rebuilt compose web image plus live auth/security, dashboard, admin-new
  session, and compatibility session smokes.

Current #147 evidence:

- 21 focused workflow-event tests cover URL structure, alternate IPv4 forms,
  IPv4-mapped IPv6, address classes, DNS failures/timeouts/mixed answers,
  canonical persistence, exact-origin matching, DNS pinning, redirect blocking,
  persisted-target revalidation, signing, ordering, and retries,
- all 382 gateway unit/integration tests pass,
- changed code is Clippy-clean; the unsuppressed crate command remains blocked
  only by pre-existing lint categories outside #147,
- dependency safety passes for Cargo and seven npm lockfiles,
- Rust workspace line coverage is 56.57% against the enforced 54.80% floor,
- all 16 authenticated default compose API surfaces pass against rebuilt
  images, including successful signed delivery to the fixed explicitly allowed
  receiver.

Current #150 evidence:

- focused gateway lifecycle and readiness tests cover monotonic/idempotent
  transitions, dependency composition, timeout/failure behavior, sanitized
  response schemas, and operational timeout validation,
- the compose API test verifies public probe schemas, Postgres loss producing
  `/readyz` 503 while `/healthz` remains 200, and readiness recovery after
  Postgres restart,
- a real gateway-container SIGTERM smoke observed readiness withdrawal before
  HTTP listener closure, bounded WebTransport task drain, clean process exit,
  and successful gateway restart,
- `node scripts/validate.mjs --profile full` passed all 46 stages: dependency
  policy, formatting, workspace Clippy/tests/coverage, all maintained Node
  checks/tests/coverage/builds, 17 default compose API surfaces, 4 docker-pool
  lifecycle/capacity surfaces, and auth, admin-new, compatibility admin, CLI,
  MCP, recording, and workflow browser smokes,
- the dedicated admin-event reconnect smoke also passed across a real gateway
  restart with fresh scoped authentication and realtime session-list recovery.

Webhook, import, and lifecycle:

- webhook target validation must reject loopback, link-local, private,
  multicast, unspecified IPs, and redirect-to-internal cases,
- browser-context import tests must cover body/profile limits, ZIP entry count,
  symlink/hardlink tar entries, duplicate profile archives, and manifest
  errors,
- gateway lifecycle regressions must continue to prove SIGINT/SIGTERM stops new
  work, drains owned in-flight work within the bound, and exposes sanitized
  readiness/lifecycle state.

Scalability and performance:

- seed enough sessions, workflow runs, queued sessions, identity mappings, and
  admin event subscribers to catch O(N^2) or N+1 query regressions,
- include `/api/v1/sessions`, `/api/v1/identity/access-review`, dashboard
  snapshots, and admin-event snapshots in query-count or latency checks,
- update ARCH.md accuracy checks when capture, tile cache, or render behavior
  changes.

## Phase 0 Evidence Gate

Issue #174 and `BPANE-00174_PHASE_0_REFERENCE_WORKFLOW_PLAN.md` select the
exact validation subset for a bounded reference workflow. At minimum it must
include:

- candidate qualification and API-first/browser-fallback evidence,
- input validation before runtime side effects,
- happy path and agreed portal/runtime failures,
- timeout, cancellation, and uncertain post-side-effect reconciliation,
- challenge or judgment mapped to terminal `external_intervention_required`
  without an internal BrowserPane handoff,
- endpoint/caller idempotency and explicit browser-side-effect certainty,
- credential, egress, context, file, and artifact boundaries,
- terminal run state and agreed evidence/retention behavior,
- operator start, monitor, stop, recovery, and escalation runbook,
- a recorded Stop, bounded Operate, or Phase 1 outcome.

Do not require unrelated enterprise features merely to complete a bounded
Phase 0. Do not omit a Foundation dependency selected by the process threat and
data profile.

## Preserved Manual Promotion Regression Gate

The `/admin-new` promotion completed through PRs #210 and #211. Preserve the
accepted gate after the root-route switch:

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
5. Keep `/admin/` directly available until a dated removal gate is accepted.
