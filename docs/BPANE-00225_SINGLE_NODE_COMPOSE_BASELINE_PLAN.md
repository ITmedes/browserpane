# BPANE-00225 Single-Node Compose Baseline Plan

Issue: [#225 Package a hardened single-node Compose deployment profile](https://github.com/ITmedes/browserpane/issues/225)

Parent: [#66 Support three deployment options: Compose, Kubernetes, and AWS Fargate](https://github.com/ITmedes/browserpane/issues/66)

Status: implementation complete and locally qualified; ready for review

## Business Case

BrowserPane has a broker-authorized Docker-host topology and an executable
security baseline, but the only runnable Compose stack is intentionally a local
development environment. It includes demo identity, fixed credentials,
development certificates, public dependency ports, source mounts, and direct
runtime compatibility. Reusing that stack on an externally reachable host
would make the deployment difficult to review and easy to misconfigure.

This slice packages one bounded operating target: a single Linux host running
BrowserPane through Docker Compose, behind organization-owned HTTPS/OIDC and
with browser and worker runtimes controlled by the authenticated runtime broker.
It creates the first deployment package that can be assessed against
`docs/THREAT_MODEL.md` and `docs/PRODUCTION_SECURITY_BASELINE.md`. It does not
claim HA, Kubernetes, Fargate, formal compliance, or universal production
readiness.

## Example Use Case

An organization provisions one dedicated Linux VM and already owns DNS, a
trusted certificate, OIDC, Postgres, Vault-compatible credential storage, an
HTTPS reverse proxy, host backup tooling, and firewall policy. The operator:

1. pins BrowserPane component and runtime images by digest,
2. supplies public OIDC and WebTransport URLs plus internal dependency URLs,
3. mounts database, Vault, broker, and worker credentials from protected files,
4. runs a preflight that rejects development defaults and unsafe exposure,
5. starts the single-node profile,
6. reaches admin-new through the HTTPS reverse proxy and the browser stream
   through the trusted WebTransport endpoint,
7. creates sessions and runs MCP-independent workflows/recordings through the
   broker, and
8. restarts the control-plane services without losing promised persisted
   resources or creating duplicate runtimes.

The deployment remains explicitly bounded by one Docker host and the operator's
external identity, storage, ingress, monitoring, backup, and incident controls.

## Current Implementation Findings

1. `deploy/compose.yml` is correctly classified as local development and must
   remain the deterministic developer/smoke stack.
2. `deploy/compose.runtime-broker.yml` proves broker policy and gateway Docker
   denial, but it is an overlay on the local stack. It inherits local Keycloak,
   Vault dev mode, demo credentials, source mounts, development certificates,
   and published dependency ports.
3. The gateway supports a file-backed runtime-broker client secret, while the
   Postgres URL and Vault token are string-only configuration. A production
   profile cannot safely provide those values without adding narrow file-input
   support.
4. Direct workflow/recording worker client secrets are string options, but the
   broker path already owns a file-backed worker OIDC secret and injects
   purpose-scoped worker credentials. The new profile must use that path rather
   than duplicate direct worker secret handling.
5. The current web image contains development fixtures and generates local
   certificate hash/SPKI helper files. Those are useful locally but should not
   be exposed by the deployment image/profile.
6. Runtime-broker worker policy and browser environment files contain
   development-specific network names, token URLs, page URLs, and recording
   certificate assumptions. The profile needs deploy-time non-secret policy
   rendering with strict validation.
7. Local filesystem artifact stores already persist opaque refs behind defined
   gateway boundaries. They are acceptable only for this single-host profile
   with explicit durability, encryption, backup, and non-HA limitations.
8. `/admin-new/` is the default operator console. `/admin/` is compatibility
   fallback only; deployment guidance must not present both as co-equal apps.

## Target Topology

### Included services

- `web`: production web image serving admin-new, compatibility admin, API
  proxying, and documented static API artifacts without development fixtures.
- `gateway`: owner/control API plus WebTransport, configured for `broker_pool`.
- `runtime-broker`: typed browser, workflow, recording, context, and
  session-data operations.
- `docker-proxy`: internal, digest-pinned, API-allowlisted Docker boundary
  reachable only by the broker.

Browser, workflow, recording, and storage-helper containers are broker-launched
jobs, not persistent Compose services.

### External dependencies

- OIDC issuer/JWKS/token service,
- Postgres,
- Vault KV v2-compatible credential provider when credential bindings are
  enabled,
- trusted certificate/DNS and HTTPS reverse proxy,
- backup, monitoring, log collection, host hardening, and firewall controls.

A local validation fixture may provide test dependencies, but the production
profile must not define or inherit Keycloak dev mode, Vault dev mode, demo
passwords, or a repository source checkout.

### Network and listener policy

- `web` publishes a configurable loopback/default management listener for an
  operator-owned reverse proxy.
- `gateway` publishes only the required WebTransport TCP/UDP listener.
- gateway API, readiness, and metrics remain private and are selectively
  proxied or collected on trusted networks.
- broker, Docker proxy, database, credential provider, CDP, worker APIs, and MCP
  have no public host ports.
- gateway and Docker proxy never share a network; gateway has no Docker socket,
  Docker endpoint, or Docker-control dependency.

MCP is excluded from the default profile until #69/#72 define accepted inbound
transport authentication and exact-origin exposure. It can remain an explicit
future private-only profile.

## Architecture Decisions

### Independent manifest, not a local overlay

Add a dedicated manifest under `deploy/single-node/`. It must not extend
`deploy/compose.yml`, because secure review should not depend on subtracting
unknown future development conveniences. Limited duplication is acceptable at
this trust boundary and is checked by contracts.

### Immutable images are inputs

The profile consumes required digest-pinned image references. It does not build
from the checkout during startup and does not silently fall back to mutable
tags. A separate local fixture can resolve freshly built images to immutable
Docker image IDs, following the existing broker-overlay launcher pattern.

### Secrets stay out of Compose values and process arguments

Add a reusable gateway secret-file loader and narrowly scoped options for:

- Postgres connection URL,
- Vault token,
- MCP bridge control token only if the component is configured later.

Configuration must reject simultaneous inline/file values and empty,
non-regular, symlinked, or over-permissive secret files where the process can
validate them. The single-node profile uses files only. Existing inline options
remain compatible for local development in this slice.

The broker gateway/worker service secrets continue to use their existing file
contracts. Secret content must never appear in rendered Compose, `docker
inspect` environment/command output, validation output, or logs.

### External identity and dependency URLs are split by audience

Public browser-facing issuer and gateway URLs are distinct from private
JWKS/token/dependency URLs. Validation checks HTTPS for public URLs and rejects
the local demo issuer, localhost public URLs, and development client metadata.
Internal URLs may use operator-approved private networking, but their trust and
TLS policy must be explicit in the runbook.

### Production web target excludes development fixtures

Create a production image target or Dockerfile that preserves:

- admin-new at `/admin-new/` and the root redirect,
- compatibility admin at `/admin/`,
- shared browser security headers,
- auth configuration generation,
- owner API/admin-event proxying,
- documented OpenAPI companion artifacts.

It excludes test/benchmark/recording fixture pages, example credentials, and
the local certificate hash/fingerprint endpoints and entrypoint helper.

### Local filesystem durability is single-host only

Named volumes or explicit managed host paths persist Postgres-independent file
workspaces, recording artifacts/staging, browser contexts/profiles, and session
data. The profile documents ownership, encryption, capacity, backup, and
restore prerequisites. It does not claim movable workloads or HA; object/shared
store adapters remain #21/#76.

## Implementation Slices

### Slice 1: Configuration and secret-file primitives

1. Add gateway secret-file loading with focused unit tests for precedence,
   empty files, symlinks, non-files, permissions, trimming, and redacted errors.
2. Add Postgres URL and Vault token file options while preserving inline local
   compatibility and rejecting ambiguous configuration.
3. Ensure `Debug`, startup errors, and dependency errors never expose loaded
   values.
4. Update Rust CLI/help and architecture documentation only for runnable
   behavior introduced here.

Commit target: `feat(gateway): support deployment secret files`.

### Slice 2: Single-node manifest and production web image

1. Add the independent Compose manifest and documented environment template.
2. Require immutable web, gateway, broker, browser, workflow, recording, and
   storage-helper image references.
3. Add non-secret broker browser/worker policy templates and a deterministic
   renderer that fails closed on missing values.
4. Add the production web target and routing/security-header checks.
5. Define private networks, bounded service security, read-only mounts/root
   filesystems where supported, health checks, restart policy, and durable
   volumes.
6. Exclude host/source mounts, development identity/secret/certificate helpers,
   direct Docker mode, and public internal listeners.

Commit target: `feat(deploy): add single-node compose profile`.

### Slice 3: Structured preflight and negative fixtures

1. Parse rendered Compose through the existing structured YAML tooling.
2. Compose the production-security, broker-service, broker-browser/worker,
   Docker-proxy, and admin-header contracts instead of copying them.
3. Validate required URLs, files, secret permissions, immutable images,
   published listeners, private networks, volumes, runtime bounds, restart
   behavior, and absence of development markers.
4. Add one negative fixture per new invariant with control-oriented errors.
5. Register the static preflight in the canonical fast profile.

Commit target: `test(deploy): enforce single-node profile contract`.

### Slice 4: Local deployment fixture and live qualification

1. Add a fixture overlay/harness that supplies local OIDC/Postgres/Vault and
   trusted test certificates without modifying the production manifest.
2. Build profile images, resolve them to immutable IDs, render non-secret
   policy, create temporary secret files with safe modes, and clean them on
   exit.
3. Start the profile and prove dependency-aware readiness.
4. Exercise admin-new session lifecycle, MCP-independent workflow execution,
   recording finalization/export, broker storage, restart reconciliation, and
   gateway Docker denial.
5. Verify promised named-volume persistence and absence of duplicate runtimes.
6. Scan rendered config, inspect data, and logs for fixed secret markers and
   forbidden development values.

Commit target: `test(deploy): qualify single-node compose profile`.

### Slice 5: Operator documentation and roadmap state

1. Add a single-node configuration/startup/diagnostics runbook.
2. Document external ingress/WebTransport, OIDC, Postgres, Vault, backup,
   upgrade/rollback, host hardening, monitoring, capacity, and unsupported
   boundaries.
3. Update README, ARCH, AGENTS, deployment/security docs, validation matrix,
   capability matrix, risk register, roadmap, and issue map where behavior or
   execution state changes.
4. Keep #66 open for Kubernetes/Fargate and #73/#74/#75/#178/#180 open for
   their respective production gates.

Commit target: `docs(deploy): document single-node operating profile`.

## Error and Failure Cases

- missing required configuration or secret file,
- secret file is empty, a symlink, not regular, unreadable, or too permissive,
- inline and file values are both supplied,
- mutable/missing image reference,
- localhost or HTTP public URL,
- local demo issuer/client/password or development certificate helper,
- direct gateway Docker endpoint/socket/network membership,
- broker/proxy public port, privilege, capability, namespace, mount, or writable
  root regression,
- public database/Vault/MCP/metrics/API listener,
- missing/broad runtime and worker admission limits,
- source checkout or trusted local workflow root mounted,
- non-durable artifact/session/context path,
- external dependency unavailable or wrong issuer/audience,
- gateway/broker restart with stale or duplicate runtime assignment,
- browser, workflow, recording, or storage cleanup failure,
- secret, bearer, private-key, requested-URL, or browser-content leakage in
  config, inspect data, diagnostics, or logs.

## Test Strategy

### Unit and contract

- Rust secret-file loader and config resolution tests,
- production web routing/header/static-surface tests,
- policy renderer tests,
- Compose/preflight positive and negative fixtures,
- repository/stage-catalog/document contract tests.

### Integration

- gateway startup with file-backed Postgres/Vault/broker credentials,
- wrong/empty/ambiguous secret configurations fail before listeners become
  ready,
- rendered profile topology and volume/listener assertions,
- external OIDC/JWKS/token split using fixture dependencies,
- gateway-to-broker and broker-to-Docker/worker service authentication.

### Smoke and E2E

1. Run the static profile preflight and all negative fixtures.
2. Render the profile using fixture values and verify only web plus
   WebTransport host listeners.
3. Start the local deployment fixture and wait for `/readyz`.
4. Authenticate to admin-new and create/connect/disconnect/reconnect/release/
   stop/kill sessions.
5. Prove two-session isolation and broker-only runtime launch.
6. Execute a pinned test workflow and download its produced file.
7. Capture a nonempty recording segment and download playback export.
8. Exercise context/session-file storage operations and verify digests.
9. Restart web/gateway/broker and prove resource persistence, runtime
   reconciliation, and no duplicate containers.
10. Prove gateway Docker proxy/socket denial and internal listener isolation.
11. Scan config, inspect output, diagnostics, and logs for fixed sensitive and
    development markers.
12. Run affected Compose API suites, admin-new smokes, canonical fast
    validation, and `git diff --check`.

## Migration and Rollback

- Existing `deploy/compose.yml` behavior and local smoke commands remain
  unchanged.
- The new profile uses a distinct Compose project name, networks, volumes, and
  configuration directory by default to prevent accidental data crossover.
- Adoption requires an explicit export/restore decision; this slice does not
  silently reuse development volumes.
- Rollback stops the new profile and restores the previous pinned image set and
  compatible database backup. Database migrations are forward-compatible only
  when the release runbook explicitly proves that claim.
- If live qualification cannot prove a required invariant, the profile remains
  `experimental` and the failed item stays open on #225; documentation must not
  call it supported.

## Acceptance Criteria

- #225 and this plan remain aligned on scope, use case, errors, and smoke.
- The profile is independent of local Compose and contains no inherited demo or
  source-development behavior.
- Every sensitive deployment value has a file/platform-backed path and no
  sensitive value appears in process arguments or rendered Compose.
- Public and internal endpoints are explicit and machine-validated.
- The gateway uses only `broker_pool` and has no Docker authority.
- Browser, worker, storage, lifecycle, admin-new, and restart behavior pass the
  local deployment fixture.
- Static and live negative evidence catches each new high-risk invariant.
- Documentation states the exact single-node support boundary and external
  operator responsibilities.
- No claim exceeds the evidence; Kubernetes, Fargate, HA, DR, shared storage,
  public MCP, and compliance remain open.

## Implementation Evidence

All five implementation slices are complete on
`feature/BPANE-00225-single-node-compose-baseline`:

- `deploy/single-node/compose.yml` is independent of local Compose and runs the
  four-service, broker-only control topology against external OIDC, Postgres,
  Vault, ingress, and registry inputs.
- `deploy/single-node/.env.example`, the non-secret policy renderer, structured
  preflight, and negative fixtures reject mutable images, development defaults,
  unsafe listeners, missing limits, invalid secret files, and Docker-boundary
  regressions.
- The production web image serves admin-new, the compatibility fallback, auth
  configuration, owner API/event proxying, and API companion artifacts without
  development fixtures or local certificate helpers.
- Gateway deployment credentials use protected files. Broker-launched workflow
  and recording credentials are delivered through bounded one-shot stdin and
  do not appear in Docker environment, command, or filesystem inspection.
- The live fixture proved two distinct browser runtimes, workflow execution, a
  retained produced file across gateway restart, broker-only Docker authority,
  secret redaction, a 272,626-byte recording artifact, and a 278,504-byte
  playback export.
- `docs/SINGLE_NODE_DEPLOYMENT.md` documents configuration, startup, ingress,
  diagnostics, backup/restore, upgrade/rollback, decommissioning, qualification,
  operator responsibilities, and unsupported boundaries.

This is repository qualification evidence, not target infrastructure, load,
HA, DR, compliance, or formal production acceptance evidence.

## Out of Scope

- Kubernetes/EKS and ECS/Fargate manifests or runtime adapters.
- Multi-node scheduling, HA, zero-downtime upgrades, or cross-host failover.
- Production Postgres/Vault/IdP installation or managed-service selection.
- Full backup/restore, DR, SBOM/signing, SLO/load, residency/BYOK, and DLP
  qualification.
- Object-store/shared-filesystem adapters.
- Public MCP transport exposure.
- Formal penetration testing or compliance certification.
