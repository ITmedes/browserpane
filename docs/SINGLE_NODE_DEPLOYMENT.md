# BrowserPane Single-Node Deployment

Status: Hardened, bounded deployment baseline for one Linux Docker host. This
profile is suitable for controlled evaluation and pilot qualification when the
operator supplies the external controls below. It is not an HA, multi-node,
Kubernetes, Fargate, or complete production-readiness claim.

Issue: [#225](https://github.com/ITmedes/browserpane/issues/225), under the
broader deployment owner [#66](https://github.com/ITmedes/browserpane/issues/66).

## Supported Boundary

The independent manifest at `deploy/single-node/compose.yml` runs four
long-lived services:

- `web`: admin-new, compatibility admin, authenticated owner API proxy, and
  static API companion artifacts;
- `gateway`: owner/control API and WebTransport in `broker_pool` mode;
- `runtime-broker`: authenticated, policy-validating runtime operations; and
- `docker-proxy`: private, API-allowlisted Docker access for the broker only.

Browser, workflow, recording, and storage-helper containers are short-lived or
session-scoped broker-launched workloads. The gateway has no Docker socket,
Docker endpoint, proxy dependency, or Docker-control network membership.

The operator must provide:

- a dedicated, patched Linux Docker host with encrypted storage and firewall
  policy;
- organization-owned HTTPS ingress and DNS for the web console;
- a trusted TCP and UDP WebTransport endpoint and certificate;
- production OIDC, Postgres, and Vault KV v2-compatible services;
- an immutable image registry, log/metric collection, capacity monitoring,
  backup/restore, release, incident, and credential-rotation procedures.

The profile does not expose MCP. Public MCP remains unsupported until its
inbound authentication and exact-origin policy are accepted. Local filesystem
artifact volumes are single-host storage, not shared or HA storage.

## Configuration

1. Place this repository or a versioned deployment bundle on the host. Do not
   mount the source checkout into running services.
2. Create an operator-owned environment file from
   `deploy/single-node/.env.example` outside the repository.
3. Replace every image placeholder with an immutable registry digest. Mutable
   tags are rejected.
4. Set the public URLs, internal dependency URLs, client IDs, runtime limits,
   and secret-file paths.
   Keep `BPANE_PROTOCOL_HANDSHAKE_TIMEOUT_MS` between 100 and 10,000. The
   initial gateway-first rollout explicitly sets
   `BPANE_PROTOCOL_LEGACY_COMPATIBILITY=true`; disabling it rejects the checked
   current pre-negotiation browser and is reversible without a data migration.
5. Create the four required secret files with owner-only permissions:

```bash
install -d -m 0700 /etc/browserpane/secrets /etc/browserpane/tls
install -m 0600 /dev/null /etc/browserpane/secrets/database-url
install -m 0600 /dev/null /etc/browserpane/secrets/vault-token
install -m 0600 /dev/null /etc/browserpane/secrets/broker-gateway-client-secret
install -m 0600 /dev/null /etc/browserpane/secrets/worker-oidc-client-secret
```

Write exactly one value to each file without placing it in shell history. The
database URL and Vault token are consumed by the gateway. The broker client
secret authenticates gateway-to-broker calls. The worker OIDC secret is read by
the broker and delivered to each worker through a bounded one-shot stdin
payload; worker credentials do not appear in Docker environment or command
inspection and are not persisted to a worker filesystem.

Install the trusted certificate chain and private key at the configured TLS
paths. The certificate must cover `BPANE_PUBLIC_GATEWAY_URL`. The recorder's
`BPANE_RECORDING_CONNECT_GATEWAY_URL` must resolve from the runtime network and
must use a hostname trusted by recorder Chromium. Use an internal SAN or a
runtime-reachable public name; do not reintroduce the development SPKI bypass.

## Preflight And Start

The preflight parses the environment file, validates secret types and modes,
renders only non-secret browser/worker policy, renders Compose as structured
JSON, and rejects mutable images, development defaults, unsafe listeners,
missing limits, and trust-boundary regressions.

```bash
node scripts/check-single-node-deployment.mjs \
  --env-file /etc/browserpane/browserpane.env
```

Start with the same environment file and an explicit project name matching
`BPANE_DEPLOYMENT_NAME`:

```bash
docker compose \
  --project-name browserpane \
  --env-file /etc/browserpane/browserpane.env \
  -f deploy/single-node/compose.yml \
  up -d
```

The examples use the default deployment name `browserpane`. If
`BPANE_DEPLOYMENT_NAME` differs, use the same value for `--project-name` in
every lifecycle command.

Do not add Keycloak dev mode, Vault dev mode, demo credentials, repository
mounts, local certificate helpers, or public dependency ports to this manifest.

## Ingress And Network Policy

- Keep `BPANE_WEB_BIND_ADDRESS=127.0.0.1` and publish the web console through an
  organization-owned HTTPS reverse proxy. Preserve BrowserPane's CSP, frame,
  MIME, referrer, and permissions headers.
- Publish the configured gateway port for both TCP and UDP. WebTransport uses
  QUIC/UDP; opening only TCP produces an apparently healthy web console with a
  failed browser connection.
- Restrict the web and WebTransport listeners to intended client networks.
- Do not publish gateway HTTP `:8932`, broker `:8940`, Docker proxy `:2375`,
  Postgres, Vault, CDP, worker, or browser-agent endpoints.
- Keep the gateway/broker and broker/Docker paths private. Use encrypted and
  authenticated service transport when traffic leaves one trusted host.
- Keep `/metrics` on a trusted collector path. It is resource-free and
  unauthenticated by design.

The primary console is `/admin-new/`. `/admin/` is a compatibility fallback,
not a second operating model.

## Readiness And Diagnostics

Inspect service state without printing the environment file or secret values:

```bash
docker compose \
  --project-name browserpane \
  --env-file /etc/browserpane/browserpane.env \
  -f deploy/single-node/compose.yml \
  ps
```

Expected readiness chain:

1. Docker proxy accepts its allowlisted health probe.
2. Runtime broker validates Docker and identity dependencies.
3. Gateway validates Postgres, broker, Vault, and local artifact stores.
4. Web starts after the gateway is healthy.

Use bounded log tails and keep the log backend access-controlled:

```bash
docker compose \
  --project-name browserpane \
  --env-file /etc/browserpane/browserpane.env \
  -f deploy/single-node/compose.yml \
  logs --since 10m --tail 200 gateway runtime-broker web docker-proxy
```

When diagnosing a failed session or job, correlate the owner resource ID with
the broker-owned `browserpane.runtime.resource_id` Docker label. Do not collect
browser content, bearer values, secret files, raw Docker inspection, requested
URLs, or decrypted egress traffic in a general support bundle.

Alert at minimum on:

- service readiness loss and restart loops;
- runtime active/starting counts approaching configured limits;
- worker admission failures, abnormal exits, and reconciliation failures;
- Postgres/Vault latency or unavailability;
- host CPU, memory, PID, inode, filesystem, and volume capacity;
- recording/workspace retention failures and backup age.
- protocol negotiation rejections and continued legacy selections; failure
  metrics use fixed reasons and contain no resource or peer-controlled labels.

The current metrics are a foundation, not a complete SLO or capacity envelope.
Set conservative admission limits and qualify the intended workload before
increasing them.

### Optional trace export

The four-service profile does not bundle a telemetry collector. To enable the
bounded gateway-to-runtime-broker browser lifecycle trace checkpoint, place an
operator-owned OTLP gRPC collector on a private service path and configure the
same credential-free endpoint for gateway and runtime broker:

```env
OTEL_TRACES_EXPORTER=otlp
OTEL_EXPORTER_OTLP_ENDPOINT=https://otel-collector.internal:4317
OTEL_EXPORTER_OTLP_PROTOCOL=grpc
OTEL_TRACES_SAMPLER=parentbased_always_on
```

The operator owns collector TLS/authentication, network policy, sampling,
redaction, storage, access, retention, and deletion. Do not place credentials,
resource identifiers, personal data, or browser data in `tracestate`. Collector
loss does not withdraw BrowserPane readiness and may drop queued spans. See
[`PLATFORM_TELEMETRY.md`](PLATFORM_TELEMETRY.md) for the exact evidence and data
boundary.

## Data, Backup, And Restore

Postgres owns durable control-plane resources and runtime assignments. Back it
up with the database operator's transactionally consistent tooling and verify
restores on an isolated environment.

The single-node profile keeps these durable Docker volumes:

- `<deployment>-recording-artifacts`;
- `<deployment>-file-workspaces`;
- dynamically created `<deployment>-session-data-*` volumes; and
- dynamically created `<deployment>-browser-context-*` volumes.

`recording-staging` is an in-flight handoff area and `runtime-sockets` is
ephemeral coordination state. A backup taken while writes are active is not a
consistent application backup. Quiesce new work, allow active workers and
recordings to finalize, withdraw or stop the web/gateway, then snapshot the
durable volumes with an approved digest-pinned backup helper or host volume
tool. Encrypt, checksum, retain, and access-control all backup material.

A restore must use a compatible image/database version and restore Postgres and
the durable artifact/context/session volumes as one tested recovery point. Start
the broker before the gateway and verify assignment reconciliation, artifact
downloads, context reuse, and absence of duplicate runtime containers before
reopening ingress.

Never use `docker compose down -v` for an environment whose data must survive.

## Upgrade And Rollback

1. Record the current image digests, environment-file checksum, database
   migration version, volume inventory, and last successful backup.
2. Run the repository preflight against the proposed environment and images.
3. Qualify the release in an isolated environment with representative sessions,
   workflow produced files, recordings, restart, and failure cases.
4. Drain new work and create a recovery point.
5. Change only reviewed immutable image digests and perform the documented
   migration/start sequence.
6. Verify readiness, authentication, session creation/connect, workflow,
   recording, artifact download, runtime limits, and telemetry before reopening
   traffic.

Rollback means restoring the prior compatible image set and, when a migration
is not backward compatible, its matching database and volume recovery point.
Restart reconciliation is not a substitute for rollback or disaster recovery.

## Stop And Decommission

Stop services while preserving data:

```bash
docker compose \
  --project-name browserpane \
  --env-file /etc/browserpane/browserpane.env \
  -f deploy/single-node/compose.yml \
  stop
```

Before decommissioning, export required evidence, revoke OIDC clients and Vault
policy, remove DNS/firewall routes, archive or destroy data according to
retention policy, and verify deletion of dynamic session/context volumes.

## Repository Qualification Fixture

The local fixture is validation evidence, not the deployment procedure above:

```bash
./scripts/start-single-node-fixture.sh
node scripts/qualify-single-node-deployment.mjs
node scripts/smoke-runtime-tracing.mjs
./scripts/stop-single-node-fixture.sh
```

It builds local images, resolves immutable image IDs, supplies isolated fixture
OIDC/Postgres/Vault services, and proves workflow execution, two-session runtime
isolation, produced-file retention across control-plane restart, always-on
recording worker launch, broker-only Docker authority, and inspect/log secret
redaction. Its fixture overlay also starts a private file-export collector for
the tracing smoke; that collector is test evidence and not part of the
four-service deployment. The qualification requires the current named branch
and commit to be pushed because the test workflow resolves its source through
Git.

## Explicitly Unsupported

- multi-host scheduling, HA, automatic failover, or zero-downtime upgrades;
- Kubernetes/EKS/GKE/AKS or ECS/Fargate/managed-runtime adapters;
- shared/object artifact stores and cross-host reusable contexts;
- public MCP ingress;
- tested production-scale envelopes or complete SLO/alert coverage;
- supplied production IdP, Postgres, Vault, ingress, backup, KMS, malware/DLP,
  residency, or incident services;
- formal compliance certification or penetration-test assurance.

These remain owned by #66, #69/#72, #73-#80, #168/#178, and related focused
follow-up issues. Do not infer them from a passing single-node fixture.
