# BrowserPane Agent Guide

This file is the shared project memory for BrowserPane. Keep it short, code-aligned, and current.

Project-wide Rust coding standards live in `RUST_STANDARDS.md`.
- Apply them to all Rust crates in this repo.
- Update that file instead of expanding this one with detailed Rust style rules.

Project-wide TypeScript and Node.js coding standards live in `NODEJS_STANDARDS.md`.
- Apply them to `code/web/bpane-client`, `code/web/bpane-admin`,
  `code/web/bpane-admin-unified`, `code/integrations/mcp-bridge`,
  `code/integrations/recording-worker`,
  `code/integrations/workflow-worker`, and future TS/Node packages.
- Update that file instead of expanding this one with detailed TS/Node style rules.

When docs disagree, prefer:
1. The code
2. Runtime manifests and package scripts
3. This file
4. `README.md`

Read `docs/CURRENT_CONTEXT.md` before selecting work in a clean session. It is
the concise handoff for current product decisions, delivery order, issue
boundaries, and local working-tree guardrails.

For the frozen owner-scoped session-control contract, use `openapi/bpane-control-v1.yaml`.

## What BrowserPane is

BrowserPane is a browser-native remote browser/desktop stack for a Linux host container.

Current product shape:
- A Linux container runs Xorg dummy + Openbox + Chromium.
- `bpane-host` captures and classifies the surface.
- `bpane-gateway` exposes WebTransport plus legacy and versioned HTTP APIs.
- Phase 0 session resources are persisted in Postgres behind the gateway.
- The browser client renders a tile-first stream with optional ROI H.264 video.
- Shared sessions are collaborative by default; optional exclusive-owner mode can lock later browser clients into read-only viewers.

## Current support matrix

- Host runtime: Linux only. Ubuntu 24.04 container is the primary target.
- Browser runtime: Chromium desktop only. Firefox and Safari are not production targets.
- Shared sessions: supported for small curated groups, not broadcast-scale delivery.
- Exclusive browser-owner mode: optional in `bpane-gateway` via `--exclusive-browser-owner`; default is disabled.
- Viewer cap: configurable in `bpane-gateway` via `--max-viewers`, default `10` for restricted browser viewers.
- MCP automation: supported via `mcp-bridge` and gateway ownership APIs.
- Service-principal registry: owner-scoped external OIDC client metadata is supported through `/api/v1/service-principals`; disabled registered principals cannot be assigned as new automation delegates.
- Browser extensions: owner-approved unpacked extensions are supported for docker-backed sessions and workflow runs; `static_single` does not support session extension sets.
- Project policies can restrict live browser uploads/downloads, session-file
  bindings, and manual recording starts for project-scoped sessions. Session
  resources expose `capabilities.file_transfer=false` when a project disables
  either live browser upload or live browser download transfer.
- Egress traffic logging is proxy-side. BrowserPane should expose sanitized
  session/profile/container correlation metadata, while the configured egress
  proxy or secure web gateway owns outbound URL/status/timing and full traffic
  logs. BrowserPane can ingest sanitized per-session receive/transmit byte
  deltas for project usage and alerting, but must not ingest requested URLs,
  headers, proxy credentials, payload contents, decrypted traffic, or raw CA
  material.
  Egress profiles can be owner-scoped or project-scoped; project-bound profiles
  and project-bound proxy credential bindings are only usable by sessions in the
  same project. Session and egress-profile resources expose sanitized egress
  diagnostics so
  operators can distinguish configuration-only proof, runtime launch metadata,
  and active browser probe evidence without exposing requested URLs, proxy
  credentials, CA material, or decrypted traffic. Active browser probes run only
  against already-ready session runtimes; diagnostics calls must not implicitly
  start stopped sessions.
  Full HTTPS inspection must be explicit through an egress profile
  `traffic_observation.mode=tls_intercept` with proxy, custom CA, and approved
  sensitive-log sink references.
- Camera ingress: disabled by default in compose; requires browser H.264 encode support and a mapped `v4l2loopback` device on the host.
- In exclusive-owner sessions, restricted browser viewers are view-only: no input, clipboard, microphone, camera, upload, download, or resize.

## Architecture map

- `code/apps/bpane-host`
  - Linux host agent. Main orchestration lives in `src/main.rs`.
  - `capture/`: X11 capture and ROI video capture support.
  - `tiles/`: tile classification and Fill/QOI/zstd emission.
  - `audio/`: desktop audio out and microphone ingest.
  - `camera.rs`: H.264 browser camera ingress to virtual camera.
  - `clipboard.rs`, `filetransfer.rs`, `input/`, `resize.rs`: host-side interaction plumbing.
- `code/apps/bpane-gateway`
  - WebTransport gateway and shared-session coordinator.
  - `lifecycle.rs`: shared starting/running/draining state, signal handling, and bounded listener/task drain coordination.
  - `readiness.rs`: concurrent, timeout-bounded readiness checks for the configured session store, runtime manager, credential provider, and artifact stores.
  - `metrics.rs`: gateway-owned OpenMetrics registry, bounded HTTP RED labels,
    aggregate runtime-capacity gauges, and label-free workflow/recording
    subsystem counters exposed at `/metrics`. Subsystem metrics reuse the
    observability instances behind authenticated operations snapshots. Never
    add resource ids, raw paths, URLs, credentials, browser content, or egress
    data as metric labels.
  - `transport.rs`: browser connection loop, per-client policy, relay behavior.
  - `session_hub.rs`: fan-out, late-join bootstrap, viewer cap, telemetry.
  - `session_control.rs`: versioned session-control store and Postgres integration, including projects with admission quotas and template/egress/extension/context/file-workspace policy bindings, service principals, session templates, browser contexts, workflows, credential bindings, file workspaces, and approved extension metadata.
  - `browser_contexts/retention.rs`: background cleanup for ready reusable browser contexts whose per-context retention window expired; runtime-backed cleanup skips active writers and removes docker profile volumes through the session manager. Browser context resources can also carry per-context profile storage limits; the API reports over-limit usage and blocks new reusable sessions from contexts whose inspected profile storage exceeds that limit. Inactive reusable contexts can be cloned into new owner-scoped reusable contexts, exported as zip archives, or imported from BrowserPane export archives into new reusable contexts; docker-backed runtimes copy, package, or restore profile volume data when present.
  - `api/browser_context_archive.rs`: bounded BrowserPane browser-context export/import format handling. Import authentication and concurrency are enforced at the API boundary; nested profile archives are size-, count-, path-, and entry-type validated before runtime materialization. Export ZIP assembly runs on Tokio's blocking pool instead of an asynchronous request thread.
  - `session_manager.rs`: internal gateway boundary for session runtime lifecycle. The rest of the gateway should depend on this façade instead of backend details.
  - `credentials/provider.rs`: credential binding secret-provider boundary. Local compose uses HashiCorp Vault dev mode and the current implementation targets Vault KV v2. Credential bindings can be owner-scoped or project-scoped; workflow runs and egress-backed sessions must not consume project-bound bindings from another project.
  - `workflow/source.rs`: workflow source contract and git ref resolution. Workflow definition versions can pin git-backed source metadata to an immutable commit at publish time without embedding source blobs into the control plane.
  - `workflow/package.rs`: Phase 0 package publication contract. New supported
    versions are Git-backed Playwright TypeScript with fixed runtime/schema/
    resource/review evidence; additive compatibility state keeps older rows
    readable without rewriting immutable versions.
  - `workspaces/model.rs`: owner-scoped file workspace and workspace-file resource shapes persisted by the control plane.
  - `workspaces/file_store.rs`: workspace file content storage boundary. `local_fs` is the current implementation; workspace files carry opaque artifact refs plus optional provenance metadata instead of raw filesystem paths.
  - `session_files/`: session-scoped file binding resource shapes. Owners can bind workspace files to relative session mount paths; automation access can read/list those bindings before runtime materialization.
  - `recording/artifact_store.rs`: recording artifact storage boundary. `local_fs` accepts only the deterministic regular file under the configured recorder staging root, rejects aliases and symlinks, derives retained bytes from file metadata, and persists opaque artifact refs instead of raw filesystem paths.
  - `recording_lifecycle.rs`: recorder-worker launch, persisted assignment tracking, and restart reconciliation for session-scoped recording, including `recording.mode=always`. Recording resources are contiguous segments; restart recovery fails the stale in-flight segment and starts a linked fresh one instead of pretending the artifact is continuous.
  - `recording/playback/`: derives session-level playback/export resources from retained recording segments. Artifact reads remain asynchronous; CPU-bound ZIP assembly for the manifest, player, and included media files runs on Tokio's blocking pool.
  - `recording/observability.rs`: gateway-local counters/timestamps for recording finalization, playback export generation, and retention passes.
  - `recording/retention.rs`: periodic cleanup of completed recording artifacts after the session-scoped retention window expires; it clears artifact refs but preserves recording segment metadata.
  - `workflow_lifecycle.rs`: control-plane launch/supervision for workflow workers. The gateway can auto-start Playwright workflow workers through direct Docker control or the typed runtime broker, persist run-worker assignments, fail stale active runs after restart instead of leaving them orphaned, and manage awaiting-input runtime hold/release semantics for paused workflow runs.
  - `worker_runtime_control.rs`: shared direct/broker worker lifecycle boundary for workflow and recording launch, inspect, remove, bounded supervision, and sanitized failures.
  - `worker_process_output.rs`: bounded concurrent stdout/stderr draining shared by workflow and recording worker supervisors.
  - `workflow_event_delivery/`: owner-scoped workflow event subscriptions,
    signed outbound webhook delivery, retry/backoff, and persisted delivery
    diagnostics. Its destination policy uses standard URL/DNS/HTTP facilities,
    checks every resolved address, pins approved DNS answers, disables redirects
    and implicit proxies, and permits non-public/HTTP receivers only through
    repeatable exact-origin deployment configuration.
  - `workflow/observability.rs`: gateway-local counters/timestamps for workflow event delivery, produced-file uploads, and workflow retention passes.
  - `workflow/retention.rs`: periodic cleanup of retained workflow logs and structured outputs after the configured workflow retention windows expire.
  - `runtime_manager.rs`: current `SessionManager` backend implementation; supports `static_single`, `docker_single`, `docker_pool`, and opt-in `broker_pool`. Local compose defaults to `docker_pool` for browser-session testing. `broker_pool` preserves the Docker pool state machine but routes browser, worker, and storage-helper operations through the authenticated runtime broker. Docker-backed workers carry a session id plus explicit session data paths for Chromium profile, uploads, and downloads. Reusable browser contexts mount a context-scoped Chromium profile volume while keeping upload/download/session-file data session-scoped, and the runtime admits only one active writer per reusable context. Docker-backed browser-context cloning, export, and import package profile volume data through the session manager boundary. Docker runtime assignments are persisted/reconciled through Postgres on gateway restart.
  - `runtime_manager/docker/container.rs`: docker runtime launch argument materialization, including safe egress observer labels, startup audit logs for correlating proxy access logs back to BrowserPane sessions, and TLS-interception CA bundle materialization for docker-backed runtimes.
  - `session_access/`: purpose-separated v2 HMAC credentials for browser
    connect, automation, and admin-event access. Cross-purpose replay is
    rejected even though all managers derive keys from one configured root.
  - `api/health.rs`: unauthenticated, resource-free `/healthz` process liveness and `/readyz` lifecycle/dependency readiness probes.
  - `api.rs`: legacy compatibility endpoints plus the frozen owner-scoped `/api/v1/sessions` surface, scoped admin-event token issuance/first-message WebSocket authentication, and session-scoped `access-tokens`, `automation-owner`, `status`, `mcp-owner`, `egress-diagnostics`, and `egress-usage` routes.
- `code/apps/bpane-runtime-broker`
  - Internal policy-validating runtime operation service. It authenticates the
    gateway with audience-bound OIDC service credentials, applies bounded
    request, concurrency, timeout, replay, and idempotency controls, and keeps
    backend-specific launch construction behind a fail-closed executor boundary.
  - Base Compose keeps the broker fail-closed. The production-like
    `deploy/compose.runtime-broker.yml` topology enables all current Docker
    adapters and routes browser, worker, and storage operations through it.
  - `docker_browser/`: broker-owned browser container materialization and
    launch/inspect/stop/remove adapter built on Bollard. It derives names,
    volumes, environment, labels, network, security, and resource bounds from
    trusted policy plus typed resource ids. Base Compose keeps this adapter
    disabled; `deploy/compose.runtime-broker.yml` enables it together with the
    gateway's opt-in `broker_pool` backend.
  - `docker_workers/`: broker-owned workflow and recording worker
    materialization plus launch/inspect/stop/remove. Immutable images, fixed
    networks and commands, approved environment keys, recording artifact
    mounts, resource bounds, and bounded Docker logs come only from trusted
    startup configuration.
  - `docker_storage/`: broker-owned session-data and browser-context storage
    helpers. It derives named volumes and typed destinations, validates bounded
    context archives, stages validated input in request-scoped volumes, and
    launches network-disabled unprivileged helpers with fixed mounts and
    guaranteed helper/staging cleanup.
- `code/shared/bpane-runtime-contract`
  - Versioned typed runtime operations, redacted secret values, sanitized audit
    resources, and deny-by-default launch and lifecycle policy.
  - The wire contract intentionally contains no raw Docker arguments, host
    paths, environment maps, privilege switches, or Docker response models.
- `code/shared/bpane-runtime-client`
  - OAuth2 client-credentials token acquisition and bounded typed HTTP transport
    from the gateway to the runtime broker. Redirects, response sizes, request
    deadlines, token lifetimes, and error exposure are constrained centrally.
- `code/shared/bpane-telemetry`
  - Shared optional OpenTelemetry setup for Rust services, including standard
    W3C Trace Context propagation, OTLP gRPC export, bounded batching/sampling,
    and redacted startup errors. Export is disabled by default. Span names and
    attributes stay fixed and must not contain resource ids, URLs, credentials,
    browser content, baggage, or raw errors.
- `code/shared/bpane-protocol`
  - Shared wire protocol, frame envelope, channel IDs, and message types.
- `deploy/examples/observability`
  - Private Prometheus scrape example plus bounded gateway/runtime/workflow/
    recording SLI rules and conservative starter alerts. Rules are validated
    with digest-pinned upstream `promtool`; alert metadata links to
    `docs/operations/PROMETHEUS_ALERT_RUNBOOK.md`. A digest-pinned Grafana OSS
    example provisions a private datasource and aggregate operations dashboard
    without host ports or embedded credentials. Thresholds are proposals, not
    contractual SLOs.
- `code/web/bpane-client/js`
  - Real browser client implementation.
  - `bpane.ts`: public API and session orchestration.
  - `tile-compositor.ts` / `webgl-compositor.ts`: render path.
  - `audio-controller.ts`: desktop audio decode and microphone Opus encode.
  - `camera-controller.ts`: WebCodecs H.264 camera ingress.
  - `file-transfer.ts`, `input-controller.ts`, `session-stats.ts`: browser interaction and telemetry.
- `code/web/bpane-client`
  - TypeScript package. There is no meaningful Rust browser client crate in the current repo.
- `code/web/bpane-admin-auth`
  - Shared framework-neutral browser auth package for both SvelteKit admin apps.
  - Delegates standards-sensitive OIDC/OAuth behavior to `oauth4webapi`; local
    code owns runtime config, bounded PKCE transaction state, memory-only token
    lifetime, auth snapshots, redirects, and app recovery.
- `code/web/bpane-admin`
  - Compatibility SvelteKit admin console retained at `/admin/`.
  - Owns the current operations overlay, route-backed inspection surfaces, and
    gateway admin-event WebSocket consumer. The consumer mints a short-lived
    event token over authenticated HTTP, opens a query-free socket, and sends
    the scoped token in the first message on every connect/reconnect.
- `code/web/bpane-admin-unified`
  - Standard SvelteKit admin console served at `/admin-new/`; local Compose
    redirects the web root to this app while retaining `/admin/` as fallback.
  - Owns the route-backed dashboard, resource catalogs, session flows and
    subareas, preview popup, recordings, workflows/runs, identity review, and
    contract-derived API/coverage/docs companions described in
    `docs/ADMIN_NEW_STATUS.md`.
- `code/integrations/mcp-bridge`
  - Streamable HTTP and legacy SSE bridge to `@playwright/mcp`; owns session registration and MCP supervision behavior.
  - Can resolve an explicit control-plane session via `/api/v1/sessions`, accepts delegated-session assignment through its bridge-local `/control-session` compatibility API, supports per-connection session routing through `/sessions/{session_id}/mcp` and `/sessions/{session_id}/sse`, resolves the managed session's runtime CDP endpoint from the session resource, and uses session-scoped `status` / `mcp-owner` APIs when a managed session is configured, including in `docker_pool` mode. In local compose, browser/admin callers mutate the bridge-global control session through the authenticated gateway proxy at `/api/v1/mcp-bridge/control-session`; the direct bridge-local control target is protected by an internal bearer token.
- `code/integrations/recording-worker`
  - Playwright-driven recorder worker that attaches as a `recorder` browser client through the control plane.
  - Creates or adopts session recording resources via `/api/v1/sessions/{id}/recordings`, waits for stop/finalize signals with sequential polling and finite HTTP/OIDC deadlines, writes the deterministic staged WebM, and uses a gateway-issued session/recording-bound worker capability for completion or failure. Ordinary session automation credentials cannot finalize artifacts.
- `code/integrations/workflow-worker`
  - One-off workflow executor worker for owner-scoped workflow runs with git-backed source snapshots.
  - Loads the workflow run through the gateway using an owner bearer token, mints session automation access, downloads the run source snapshot and workspace inputs, materializes them locally, uploads produced files back through run-scoped artifact APIs, and executes the pinned Playwright entrypoint against the bound BrowserPane session with finite HTTP/OIDC deadlines and bounded stdout/stderr capture.
- `deploy/compose.yml`
  - Source of truth for local dev runtime defaults.
  - Local auth in compose is OIDC via Keycloak on `:8091`.
  - Local session-control persistence in compose is Postgres on `:5433`.
  - Local workflow credential binding dev/testing uses HashiCorp Vault dev mode on `:8200`.
  - Local compose defaults to `docker_pool` for browser-session workers, with a shared socket-only runtime volume and per-session browser data volumes; `mcp-bridge` resolves the delegated session's runtime endpoint dynamically in that mode.
  - The gateway reaches Docker through the internal, digest-pinned
    `docker-proxy` service and has no raw socket mount. The proxy is an
    allowlisted Compose defense-in-depth boundary, not a complete production
    authorization boundary; production still requires a typed launch broker or
    orchestrator adapter.
  - Local compose uses a one-shot helper to build the `deploy-recording-worker` image and configures the gateway to launch short-lived recorder containers for `recording.mode=always`; artifact handoff uses the trusted `bpane-recordings` staging volume and finalized artifacts use the separate `bpane-recording-artifacts` gateway store.
  - The gateway is configured to auto-launch workflow workers against the `deploy-workflow-worker` image on the compose network. Build that image before workflow-run smoke tests or local workflow execution.
  - `deploy/compose.runtime-broker.yml` is the opt-in production-like
    Docker-host topology. It pins
    browser and worker images by immutable image id and moves their container
    lifecycle plus typed storage operations through the broker. The gateway has
    no `DOCKER_HOST`, Docker socket, `docker-control` membership, or proxy
    dependency in this topology. Base Compose remains the explicit local direct
    `docker_pool` compatibility path.
  - The gateway mounts the repo at `/workspace:ro` so local git-backed workflow sources can be resolved and materialized during development smokes.
- `deploy/single-node/compose.yml`
  - Independent hardened baseline for one Linux Docker host: `web`, `gateway`,
    `runtime-broker`, and `docker-proxy` are the only long-lived BrowserPane
    services; browser and worker containers are broker-launched.
  - Requires immutable images, protected secret files, external OIDC/Postgres/
    Vault, and operator-owned ingress, storage, monitoring, and host controls.
    Dynamic worker credentials use bounded one-shot stdin and must not appear in
    Docker environment, command, or filesystem inspection.
  - Validate with `node scripts/check-single-node-deployment.mjs`; the complete
    operator and qualification procedure is in `docs/SINGLE_NODE_DEPLOYMENT.md`.
- `deploy/examples/egress-observer`
  - Local egress observation fixtures. `compose.yml` runs a metadata-only Squid forward proxy at `bpane-egress-observer:3128` and an auth-enforcing Squid proxy at `bpane-egress-auth-observer:3130` for proxy-auth validation. `compose.tls.yml` runs a mitmproxy TLS-intercept proxy at `bpane-egress-tls-observer:3129` using local CA material prepared by `prepare-mitmproxy-ca.sh`. `egress-usage-reporter.mjs` is the local sanitized usage-ingestion example: it joins Squid logs with docker runtime labels and calls `/api/v1/sessions/{id}/egress-usage` with byte counters and safe observer metadata only.

## Protocol and media facts

- `CH_VIDEO` is server-to-client datagram H.264 ROI video.
- `CH_TILES` is reliable tile rendering and is the primary visual path for UI/text.
- Desktop audio out uses codec-tagged frames; the compose stack currently defaults to Opus.
- Microphone ingress is Opus, not raw PCM.
- Camera ingress is H.264 via WebCodecs only. There is no MJPEG fallback.
- Tiles are QOI or zstd depending on emitter settings and heuristics.
- Viewers receive a filtered capability set and are enforced as read-only in both gateway and client.

## Shared-session behavior

- Browser sessions are collaborative by default.
- If `--exclusive-browser-owner` is enabled, one owner drives the session and additional browser clients join as viewers.
- MCP automation does not by itself lock browser clients into viewer behavior. If MCP is the initial connector it seeds the display size; if a browser client is already connected, that browser-defined display size remains authoritative.
- Late joiners are bootstrapped from cached session state and late-join refreshes are tracked in gateway telemetry.
- If a worker is still alive, reconnect returns to the exact live runtime. After idle-stop, reconnect restarts from the persisted Chromium profile instead of a true suspended process image.
- Gateway session status reports:
  - browser and viewer counts
  - `max_viewers` and remaining slots
  - session-scoped recording playback/export summary derived from retained segments
  - join latency telemetry
  - full-refresh burst telemetry

## Commands that matter

- Local fast validation: `node scripts/validate.mjs --profile fast`
- Explicit compose validation: `node scripts/validate.mjs --profile compose`
- Full local validation: `node scripts/validate.mjs --profile full`
- Dependency safety scan: `node scripts/check-dependency-safety.mjs`
- Repository document/workflow policy: `node scripts/check-repository-documents.mjs`
- Production security baseline: `node scripts/check-production-security-baseline.mjs`
- Redacted compose diagnostics: `node scripts/collect-compose-diagnostics.mjs`
- CI compose cleanup: `scripts/ci/cleanup-compose.sh`
- Dependency policy tests: `node --test scripts/dependency-safety/*.test.mjs`
- Rust coverage ratchet: `node scripts/run-rust-coverage.mjs`
- Full Rust test suite: `cargo test --workspace`
- Gateway tests: `cargo test -p bpane-gateway`
- Gateway in-memory session-store contract: `cargo test -p bpane-gateway session_store_contract_in_memory`
- Gateway Postgres session-store contract: `BPANE_SESSION_STORE_CONTRACT_POSTGRES_URL=postgresql://browserpane:browserpane-dev@localhost:5433/browserpane cargo test -p bpane-gateway session_store_contract_postgres -- --ignored --test-threads=1`
- Gateway compose e2e API suite: `cargo test -p bpane-gateway --test compose_api_surface -- --ignored --test-threads=1`
- Gateway docker-pool compose e2e suite: `cargo test -p bpane-gateway --test compose_api_surface_docker_pool -- --ignored --test-threads=1`
- Gateway compose e2e wrapper: `scripts/run-gateway-compose-e2e.sh --suite all`
- Runtime-broker storage smoke: `scripts/smoke-runtime-broker-storage.sh`
- Runtime-broker isolation smoke: `scripts/smoke-runtime-broker-isolation.sh`
- Runtime-broker restart smoke: run `npm run smoke:runtime-broker-restart -- --headless` in `code/web/bpane-client`
- Runtime-tracing fixture contract: `node scripts/validate-runtime-tracing-fixture.mjs`
- Runtime-tracing unit contracts: `node --test scripts/runtime-tracing/*.test.mjs`
- Runtime-tracing live smoke: `node scripts/smoke-runtime-tracing.mjs` after starting the single-node fixture
- Prometheus rule policy tests: `node --test scripts/observability/prometheus-rules-contract.test.mjs`
- Prometheus config/rule/unit validation: `node scripts/observability/validate-prometheus-rules.mjs`
- Grafana dashboard policy tests: `node --test scripts/observability/grafana-*.test.mjs`
- Grafana live provisioning/query validation: `node scripts/observability/validate-grafana-dashboard.mjs`
- Host tests: `cargo test -p bpane-host`
- Protocol tests: `cargo test -p bpane-protocol`

Run these in `code/web/bpane-client`:
- `npx tsc --noEmit`
- `npm run smoke:automation-tasks -- --headless`
- `npm run smoke:bpane-cli -- --headless`
- `npm run smoke:admin-browser-contexts -- --headless`
- `npm run smoke:admin-unified-browser-contexts -- --headless`
- `npm run smoke:admin-unified-dashboard -- --headless`
- `npm run smoke:admin-egress-profiles -- --headless`
- `npm run smoke:admin-unified-egress-profiles -- --headless`
- `npm run smoke:admin-unified-projects -- --headless`
- `npm run smoke:admin-unified-sessions -- --headless`
- `npm run smoke:admin-unified-workflows -- --headless`
- `npm run smoke:admin-unified-workflow-runs -- --headless`
- `npm run smoke:admin-unified-identity -- --headless`
- `npm run smoke:admin-unified-api-companion -- --headless`
- `npm run smoke:admin-unified-file-workspaces -- --headless`
- `npm run smoke:file-workspaces -- --headless`
- `npm test`
- `npm run build`
- `../../../scripts/bpane workflow --help`
- `npm run smoke:recording -- --headless`
- `npm run smoke:workflow-embed -- --headless`
- `npm run smoke:workflow-cancel -- --headless`
- `npm run smoke:workflow-cli -- --headless`
- `npm run smoke:workflow-credential-injection -- --headless`
- `npm run smoke:workflow-credentials -- --headless`
- `npm run smoke:workflow-workspace -- --headless`
- `npm run smoke:workflows -- --headless`
- `npm run smoke:workflow-extension -- --headless`
- `npm run smoke:workflow-failure -- --headless`
- `npm run smoke:workflow-reconnect -- --headless`
- `npm run smoke:workflow-queued-cancel -- --headless`
- `npm run smoke:workflow-restart-safety -- --headless`
- `npm run smoke:workflow-runtime-hold -- --headless`
- `npm run smoke:workflow-embed-operations -- --headless`
- `npm run smoke:multisession -- --headless`
- `npm run test:coverage`

Run these where applicable:
- `cd code/web/bpane-admin-auth && npm run test:coverage`
- `cd code/web/bpane-admin-unified && npm run test:coverage`
- `cd code/integrations/mcp-bridge && npm test && npm run build`
- `cd code/integrations/recording-worker && npm test && npm run build`
- `cd code/integrations/workflow-worker && npm test && npm run build`
- `node --test deploy/examples/egress-observer/egress-usage-reporter.test.mjs`
- `node --check deploy/examples/egress-observer/egress-usage-reporter.mjs`
- `npm ci --ignore-scripts --prefix scripts/openapi`
- `npm test --prefix scripts/openapi`
- `npm run check --prefix scripts/openapi`
- `npm run compatibility --prefix scripts/openapi -- --base-ref origin/main`

## Local development flow

1. `./deploy/gen-dev-cert.sh dev/certs`
2. Start the local stack:
   `BPANE_GATEWAY_MAX_ACTIVE_RUNTIMES=2 docker compose -f deploy/compose.yml up --build`
3. Wait for `curl -fsS http://localhost:8932/readyz`; use `/healthz` only for process-liveness checks.
4. Open `http://localhost:8080/` in Chromium. The web root redirects to
   `/admin-new/`; use `/admin/` only for compatibility fallback checks.
5. Log in through the local Keycloak realm with `demo / demo-demo`.
6. The admin console will resolve or create an owner-scoped `/api/v1/sessions` resource before transport connect.
7. The admin console will mint a short-lived session-scoped connect ticket before WebTransport connect.
8. Use `Delegate MCP` if you want the local `mcp-bridge` to adopt that same session.
9. If needed, use the SPKI fingerprint from `http://localhost:8080/cert-fingerprint` so Chromium trusts the local gateway cert. `./deploy/gen-dev-cert.sh dev/certs` also refreshes `dev/certs/cert-fingerprint.txt` from the same `cert.pem`.
10. `vault` listens on `:8200`, `keycloak` on `:8091`, `postgres` on `:5433`, `mcp-bridge` on `:8931`, and the gateway HTTP API on `:8932`.

## Guardrails for contributors and agents

- Trust code and runtime manifests over stale prose. `README.md` may lag behind implementation.
- For Rust work, follow `RUST_STANDARDS.md` in addition to this file.
- When an implementation changes user-visible behavior, local setup, runtime
  topology, API routes, commands, support matrix, or validation flow, check
  whether `README.md` needs a matching update in the same slice. If no README
  change is needed, mention that explicitly in the PR or handoff notes.
- When a change adds or alters a public/internal endpoint, credential type,
  runtime or storage adapter, callback, deployment profile, or sensitive data
  class, update `docs/THREAT_MODEL.md` and
  `docs/PRODUCTION_SECURITY_BASELINE.md` or record why their contracts are
  unchanged.
- Before starting a planned implementation slice, create or update a dedicated
  plan file under `docs/` whose filename matches `*_PLAN.md`. Each plan must
  include the targeted issue, an example use case, and a post-implementation
  smoke test sequence. Start from `docs/PLAN_TEMPLATE.md` and create the plan
  when the focused implementation slice enters Ready or In Progress; do not
  pre-create detailed executable plans for the whole Backlog. Existing
  feature-level or qualification `*_PLAN.md` documents are specifications, not
  authorization to implement an unready issue; derive a bounded slice plan
  before coding.
- The optional local Codex delivery loop lives under `dev_loop/`. It consumes
  only live `state:ready` issues with matching focused plans, uses separate
  proposal and repair sessions, and leaves automatic merging disabled unless a
  maintainer explicitly enables it. Read `dev_loop/README.md` before running
  it; never commit its generated `runs/`, `.lock/`, or `STOP` state.
- When working with GitHub issues, keep issue state implementation-oriented:
  prefer one canonical issue per shippable slice, document the business case,
  scope, acceptance criteria, example use case, and smoke sequence on that
  issue, and close duplicates only after commenting with the canonical target.
  Keep the local `docs/*_PLAN.md` file aligned with the canonical issue before
  implementation starts.
- Create and update BrowserPane GitHub issues only through the approved project
  identity `thebackplane` (or another identity explicitly approved by the
  user). Verify `gh api user --jq .login` before mutating an issue; never use a
  personal identity. Do not expose local authentication material in commands,
  logs, docs, or commits.
- Use `docs/DELIVERY_ROADMAP.md` for current sequencing,
  `docs/CAPABILITY_MATURITY_MATRIX.md` for capability claims,
  `docs/PRODUCT_PHASES_AND_RELEASE_GATES.md` for promotion evidence, and
  `docs/RISK_REGISTER.md` for active risk. Treat long work-order, review, and
  legacy-audit documents as supporting context rather than competing queues.
- When a change affects an externally visible product or management claim,
  update the capability maturity evidence and the investor claim/evidence
  register in the same slice or explicitly assign that follow-up.
- Do not edit generated or vendored output:
  - `code/web/bpane-client/dist/`
  - `node_modules/`
  - `test-results/`
- Keep this file aligned with the live code when browser support, session-sharing behavior, media codecs, or runtime topology changes.
- Keep `docs/CURRENT_CONTEXT.md` aligned when the current product boundary,
  Phase 0 contract, issue order, or default operator surface changes.
- Prefer narrow, subsystem-specific validation plus any impacted cross-cutting checks.

## When adding or changing features

- Update the support matrix if the change affects:
  - browser support
  - host platform support
  - session-sharing limits
  - default media behavior
- Update the architecture map if subsystem ownership moves.
- Only document commands that are actually runnable in this repo.
