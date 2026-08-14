# BrowserPane Production Security Baseline

Status: Required controls for deployment acceptance. The repository provides
local development, production-like broker validation, and a hardened but
bounded single-node package. The latter is not complete production, HA,
managed-cloud, compliance, or target-acceptance evidence.

Threat model: `docs/THREAT_MODEL.md`

Owners: focused baseline [#223](https://github.com/ITmedes/browserpane/issues/223),
deployment packaging [#66](https://github.com/ITmedes/browserpane/issues/66),
and broader security roadmap [#72](https://github.com/ITmedes/browserpane/issues/72).

## Responsibility Model

| Responsibility | BrowserPane application | Deployment/infrastructure operator | External dependency owner |
| --- | --- | --- | --- |
| Authentication and authorization | Validate token issuer/audience/purpose/expiry; scope owner/session resources | Configure trusted issuers, clients, redirect origins, service identities and future grants | Operate IdP, MFA, account lifecycle and signing keys |
| Public transport | Provide HTTP/WebTransport services and security-header contract | Terminate trusted TLS, restrict origins/routes, enforce network policy and rate limits | Operate DNS, CA, ingress/load balancer |
| Runtime isolation | Validate typed runtime operations and container policy | Isolate broker/orchestrator, hosts, networks and kernel; select approved immutable images | Maintain container runtime, host OS and orchestrator |
| Secrets | Store opaque bindings and resolve through provider boundary | Inject short-lived/file-backed identities, rotate/revoke, restrict provider policy | Operate Vault/KMS/secret manager |
| Data/storage | Enforce owner/project/path/resource contracts and retention metadata | Supply encrypted durable stores, backup/restore, lifecycle and access policy | Operate database/object storage/KMS |
| Egress | Bind sessions to approved profiles and expose sanitized evidence | Enforce proxy/SWG/firewall/DNS policy and approved TLS inspection | Operate proxy, CA and log sink |
| Telemetry | Emit bounded sanitized health/readiness/metrics evidence | Isolate collectors, set retention/access, define alerts/runbooks | Operate telemetry backend |
| Incident/release governance | Preserve evidence and deterministic resource state | Patch, scan, sign, deploy, monitor, respond and recover | Operate source/build registries and notification channels |

## Required Production Controls

### Public ingress and browser security

- [ ] Publish only HTTPS origins with trusted certificates; do not use the local
  SPKI bypass or development certificate flow.
- [ ] Route admin, owner API, metrics, MCP, WebTransport, and compatibility
  surfaces explicitly. Do not expose internal worker, broker, Docker proxy,
  Postgres, Vault, Keycloak admin, CDP, or browser-agent endpoints publicly.
- [ ] Restrict browser origins and CORS per surface. MCP transport exposure is
  not production-supported until inbound authentication and exact-origin policy
  are implemented.
- [ ] Preserve the admin CSP, frame denial, MIME sniffing, referrer and
  permissions policies validated by
  `scripts/ci/admin-security-headers-contract.test.mjs`.
- [ ] Add infrastructure request/body/rate limits without weakening gateway
  limits or WebTransport behavior.

### Human and service identity

- [ ] Configure a production OIDC issuer, expected audience, HTTPS JWKS/token
  endpoints, approved redirect URIs, and suitable MFA/session policy.
- [ ] Use separate confidential service clients for gateway-to-broker and
  worker-to-gateway access. Keep allowed client IDs narrow.
- [ ] Inject service secrets from files, workload identity, or a platform secret
  interface. Do not place secrets in images, source, command lines, URLs, or
  general environment dumps.
- [ ] Define rotation, revocation, emergency access, deprovisioning and stale
  assignment review. #70, #176 and #177 own missing product controls.
- [ ] Keep connect, automation, admin-event, recording-worker and broker service
  credential purposes separate and short-lived.

### Network and runtime boundary

- [ ] Use the authenticated broker runtime path or a policy-equivalent
  orchestrator adapter. The direct `docker_pool` proxy path is local
  compatibility, not the production target.
- [ ] Place gateway-to-broker, broker-to-identity and broker-to-orchestrator
  traffic on explicit private networks with encrypted/authenticated transport
  appropriate to the deployment.
- [ ] Do not expose the Docker socket. If a Docker proxy is used, pin its image,
  limit API families, keep it internal, and permit only the broker as caller.
- [ ] Run broker and proxy without host-published ports, privileged mode, added
  capabilities, host PID/IPC, or devices. Keep broker root read-only and writable
  temporary storage bounded.
- [ ] Run browser and worker containers without privileged mode, host
  namespaces/devices, caller-selected capabilities, or mutable image references.
  Validate seccomp/AppArmor/SELinux and browser sandbox behavior on each target.
- [ ] Enforce CPU, memory, PID, shared-memory, timeout, output, concurrency and
  startup limits. Record tested envelopes rather than inferred scale.
- [ ] Separate customer/runtime workloads from control-plane dependencies and
  define host/orchestrator patching and isolation ownership.

### Secrets and credentials

- [ ] Use a production secret provider with least-privilege workload identity.
  The Compose Vault dev root token is prohibited.
- [ ] Restrict each credential binding to the intended project, workflow,
  destination/origin and runtime purpose.
- [ ] Keep proxy credentials, CA private material and decrypted traffic in the
  egress enforcement system, not BrowserPane control resources or telemetry.
- [ ] Define secret rotation, access review, audit and retention. Treat local
  repository fixture secrets as test data. For the single-node package, keep
  operator-owned secret files outside the repository, restrict their modes,
  rotate them through an approved procedure, and prefer workload identity where
  the target supports it.

### Data, artifacts and retention

- [ ] Use production Postgres with encrypted transport, least-privilege role,
  durable storage, monitored pool capacity, migration controls and tested
  backup/restore.
- [ ] Replace local filesystem artifact stores when durability, HA, residency or
  multi-node execution requires it. Preserve opaque artifact references and path
  validation in every adapter.
- [ ] Encrypt persistent data and backups, define key ownership/rotation, and map
  storage location to residency policy.
- [ ] Configure and verify retention for browser contexts, workspace/session
  files, workflow logs/outputs, recordings, callbacks and operational evidence.
- [ ] Define malware/DLP handling before untrusted files or recordings cross the
  organization boundary.

### Egress, callbacks and integrations

- [ ] Bind production sessions to approved egress policy and deny unintended
  direct internet paths at the network layer.
- [ ] Keep TLS interception explicit, use approved CA material and sensitive-log
  sinks, and communicate the policy to affected operators/users.
- [ ] Preserve callback DNS/IP validation, no-redirect/no-system-proxy behavior,
  exact-origin exception review, signing and bounded retries.
- [ ] Rotate callback signing keys and require receivers to enforce authenticity,
  replay windows, payload limits and idempotency.
- [ ] Keep generalized MCP, Human Handoff and external workflow endpoints private
  until their auth, grant, callback and deployment profiles are accepted.

### Operations, telemetry and recovery

- [ ] Keep `/healthz` for liveness and `/readyz` for dependency/admission state;
  configure orchestrator probes and bounded drain behavior.
- [ ] Expose `/metrics` only to trusted collectors. Preserve bounded labels and
  exclude owner/session IDs, credentials, requested URLs and browser content.
- [ ] Define logs/metrics/traces access, encryption, retention and redaction;
  complete #178 before claiming end-to-end SLO or capacity evidence.
- [ ] Alert and runbook dependency loss, runtime/worker saturation, callback
  failure, artifact failure, auth failure and abnormal lifecycle transitions.
- [ ] Test backup/restore, broker/gateway/runtime restart, key rotation,
  dependency failure, rollback and incident evidence collection.
- [ ] Pin and scan build/runtime dependencies; complete SBOM, signing,
  provenance, vulnerability intake, contribution and IP governance in #75/#180.

## Local Development Exceptions

The following are intentional local development conveniences and are prohibited
as production defaults:

| Exception in `deploy/compose.yml` | Why it exists locally | Production replacement/owner |
| --- | --- | --- |
| Keycloak `start-dev`, imported demo realm and demo/admin passwords | Reproducible login smoke | Managed/hardened IdP and secret injection: #66/#72 |
| Vault dev mode and fixed root token passed to gateway | Credential-binding fixture | Workload identity and least-privilege secret policy: #66/#70 |
| Published Postgres, Vault, Keycloak, MCP, gateway and web ports | Local inspection and tests | Explicit ingress/private networks; no dependency exposure: #66 |
| Locally generated certificate and SPKI bypass | WebTransport development trust | Trusted certificate chain and managed rotation: #66 |
| Direct `docker_pool` compatibility and gateway Docker proxy access | Fast local runtime development | Authenticated broker or policy-equivalent adapter: #66/#72 |
| `seccomp=unconfined` for direct host/runtime compatibility | Local Chromium/container compatibility | Qualified target-specific sandbox policy in broker/orchestrator path: #66/#72 |
| Bind-mounted source tree and local workflow roots | Local git-backed workflow smoke | Immutable published source/package and isolated worker materialization: #47/#66 |
| Local filesystem workspace/recording stores | Single-host test evidence | Durable encrypted object/artifact adapters: #21/#66/#76 |
| Wildcard MCP CORS and unauthenticated MCP transport | Local trusted-client compatibility | Inbound transport auth, exact origins and private exposure: #69/#72 |
| File-backed static broker/worker client secrets in repository fixtures | Repeatable local OIDC service auth | Platform secret/workload identity and rotation: #66/#70 |
| Unauthenticated `/metrics` on local API port | Local Prometheus/curl scrape | Collector-only network/listener policy: #66/#178 |

The production-like broker validation overlay removes gateway Docker reachability
and validates broker policy. It does not remove every local exception above.
The independent `deploy/single-node/compose.yml` profile removes the listed
demo identity, dev Vault, source-mount, local-certificate-helper, and public
dependency defaults. Its remaining external controls and acceptance work are
documented in `docs/SINGLE_NODE_DEPLOYMENT.md` and must be supplied and tested
by the operator.

## Deployment Gate Checklist

### Architecture and ownership

- [ ] Target profile, data classification, threat model and residual-risk owners
  are reviewed.
- [ ] Public, private, internal and worker-only endpoints are inventoried and
  enforced by ingress/network policy.
- [ ] BrowserPane, infrastructure, IdP, secret, storage, egress and incident
  responsibilities are assigned.
- [ ] Unsupported capabilities and local-development exceptions are excluded or
  explicitly accepted with owner and expiry.

### Identity, runtime and data

- [ ] Human/service identities, grants, secret injection and rotation pass a
  negative-access review.
- [ ] Broker/orchestrator policy, immutable images, sandbox, resources and host
  isolation pass static and live validation.
- [ ] Postgres/artifact encryption, retention, backup and restore are tested on
  the actual target.
- [ ] Egress, callbacks, files, recordings and telemetry comply with the agreed
  data policy.

### Validation and operations

- [ ] `node scripts/check-production-security-baseline.mjs` passes.
- [ ] `node scripts/check-single-node-deployment.mjs --env-file <path>` passes
  against the exact deployment configuration.
- [ ] Canonical fast and affected Compose validation profiles pass on the release
  commit.
- [ ] Failure injection distinguishes identity, database, secret, runtime,
  storage, callback and saturation failures.
- [ ] Capacity/load evidence supports the configured limits and expected Pilot
  workload.
- [ ] Rollback, restore, key rotation, incident escalation and evidence export
  have named runbooks and owners.
- [ ] Release artifacts satisfy dependency, SBOM, signing/provenance and license
  governance selected for the target gate.

Passing this checklist is necessary but not sufficient for formal compliance,
security certification, HA, residency, or enterprise-readiness claims.
