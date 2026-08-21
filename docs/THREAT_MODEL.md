# BrowserPane Threat Model

Status: Production-baseline input; not a certification or production-readiness
claim.

Last reviewed: 2026-08-14 against the gateway OpenMetrics checkpoint, the
authenticated runtime-broker topology, and the qualified single-node package.

Owner: [#223](https://github.com/ITmedes/browserpane/issues/223), under the
broader security roadmap in [#72](https://github.com/ITmedes/browserpane/issues/72).

## Purpose And Scope

This document describes the current BrowserPane trust model, implemented
controls, required deployment controls, and residual risks. It covers the
owner-scoped control plane, browser data plane, operator application, MCP and
workflow integrations, runtime broker, browser and worker containers, storage,
egress, callbacks, and telemetry.

The model is evidence-led:

- `Implemented` means a current code/manifest control and test are named.
- `Required` means deployment or operator action is necessary and BrowserPane
  does not currently enforce it end to end.
- `Residual` means the risk remains and has a canonical issue owner.

## Profiles And Assumptions

### local development

`deploy/compose.yml` is the canonical local development profile. It publishes
dependencies to localhost and uses development credentials, Keycloak dev mode,
Vault dev mode, local certificates, and a direct Docker compatibility path. It
is not a production deployment.

### production-like broker validation

`deploy/compose.yml` plus `deploy/compose.runtime-broker.yml` proves the local
Docker trust boundary. The gateway loses Docker network/socket/proxy access and
uses a dedicated OIDC service identity to call the runtime broker. The broker
validates typed browser, worker, and storage operations before using the private
Docker proxy. This profile is production-like broker validation, not complete
production packaging.

### hardened single-node baseline

`deploy/single-node/compose.yml` packages the broker topology for one dedicated
Linux Docker host. It requires immutable images, protected secret files,
external OIDC/Postgres/Vault, trusted ingress, and operator-owned host, backup,
monitoring, and network controls. Repository qualification proves the packaged
boundary and core browser/workflow/recording behavior, not target-specific
production acceptance, HA, managed-cloud support, scale, or compliance. Issue
#66 and the focused production owners retain those remaining gates.

Assumptions common to all profiles:

- Chromium, visited websites, downloaded content, uploaded files, workflow
  source, extension packages, and user-provided metadata are untrusted.
- The external identity provider, certificate authority, database, secret
  provider, artifact storage, egress enforcement point, and orchestrator are
  separately administered dependencies.
- Host or orchestrator compromise is outside the protection offered by a
  container boundary.
- BrowserPane can reduce component authority and exposure; it cannot make a
  compromised gateway, broker, runtime, or dependency harmless.

## Assets And Data Classes

| Class | Examples | Security objective |
| --- | --- | --- |
| Authentication material | Owner OIDC tokens, external client-credentials tokens, connect tickets, automation tokens, admin-event tickets, broker/worker credentials | Confidentiality, short lifetime, audience/purpose binding, replay resistance |
| Secret material | Vault tokens, credential-binding values, proxy credentials, webhook signing secrets, CA private material | Never returned through owner APIs, logs, metrics, or artifacts |
| Control-plane data | owners, projects, policies, templates, sessions, workflows, grants, run state | Authorization, integrity, tenant separation, auditability |
| Browser/session data | rendered frames, input, clipboard, microphone, camera, cookies, profile state | Session isolation, capability enforcement, bounded retention |
| Files and evidence | uploads, downloads, workspace files, produced files, recordings, exports | Path isolation, integrity, retention, authorized download |
| Workflow code | git source, pinned commits, entrypoints, dependencies | Immutable publication, source containment, runtime isolation |
| Network evidence | egress profile, proxy metadata, byte counters, diagnostics | Sanitization; no credentials, payloads, decrypted traffic, or requested URLs in BrowserPane telemetry |
| Operational evidence | logs, metrics, readiness, future traces, support bundles | Availability and diagnosis without sensitive/high-cardinality leakage |
| Availability resources | runtime slots, worker slots, queues, Postgres pool, CPU, memory, storage | Bounded admission, observability, recovery, tested capacity |

## Actors And Attacker Model

| Actor | Intended authority | Threat considered |
| --- | --- | --- |
| Owner/operator | Manage owner-visible resources and interactive sessions | Stolen token, malicious/compromised browser, accidental unsafe configuration |
| Viewer | Observe a shared session subject to session policy | Input or data-exfiltration privilege escalation |
| Automation principal | Operate one delegated session or workflow scope | Cross-session access, token replay, over-broad service identity |
| External process caller | Invoke, poll, cancel, or read artifacts for one granted project Workflow Endpoint | Interactive-token substitution, unregistered/disabled identity, cross-project access, grant/scope bypass, unsafe retry after uncertain effects |
| Runtime-broker service principal | Submit policy-valid runtime operations | Credential theft, replay, gateway compromise using allowed broker authority |
| Browser runtime | Execute untrusted websites with approved session capabilities | Browser exploit, container escape, network/file/credential exfiltration |
| Workflow/recording worker | Execute pinned workflow or record one session | Malicious source, wrong-session access, artifact/path abuse, output flooding |
| Malicious website | Control browser content and network responses | Drive-by exploit, clipboard/file/camera/microphone abuse, challenge or phishing behavior |
| Malicious uploaded/archive content | Enter file and browser-context boundaries | Path traversal, symlink/hardlink escape, decompression/resource exhaustion |
| External callback receiver | Receive signed workflow events | Redirect/SSRF target, event forgery, replay, availability failure |
| Dependency/operator | Administer IdP, Postgres, Vault, artifact store, proxy, CA, orchestrator | Misconfiguration, compromise, outage, privileged insider access |
| Network attacker | Observe or alter traffic outside trusted private networks | Credential/content disclosure, request manipulation, service impersonation |

## Trust Boundaries And Data Flows

| Boundary | Data/authority crossing | Current control and evidence | Required or residual |
| --- | --- | --- | --- |
| Browser to web/admin-new | Static code, OIDC transaction, verified identity display | Shared `oauth4webapi` adapter, in-memory token set, PKCE/nonce/state tests, hash CSP, nginx header contract | HTTPS public origin and trusted IdP configuration are required by #66 |
| Browser/CLI to owner API | Owner bearer, resource requests and responses | Gateway JWT issuer/audience validation; owner-scoped store/API tests; OpenAPI classification | Complete organization/project grants remain #176 |
| Browser to admin events | Short-lived first-frame credential, sanitized snapshots | Purpose-scoped admin-event token and wrong-purpose/expiry tests | Event-stream capacity and generalized security-event export remain #28/#164 |
| Browser to WebTransport | Short-lived connect ticket, frames, input, media, files | Purpose-scoped/expired ticket tests; session capability and viewer enforcement | Public origin/TLS packaging and protocol conformance remain #66/#175 |
| Automation to session API/CDP | Session automation token and delegated authority | Session-bound token, wrong-purpose and wrong-session API/Compose tests | Direct automation productization and broader grants remain #69/#176 |
| External process to Workflow Endpoint | OIDC client-credentials token, endpoint input/correlation/idempotency key, restricted invocation/outcome/artifact projection | Issuer/client resolution to active service principal; project membership plus exact endpoint operation grant and declared scope; pre-runtime schema validation; endpoint/caller fingerprint; original-caller reads; RFC 9457 failures; typed bounded outcomes and artifact metadata | TLS/private ingress and IdP client-secret lifecycle are deployment-owned; generalized RBAC remains #176 and callbacks/revisions/rate limits remain #240 |
| MCP client to bridge | MCP requests and selected/delegated session | Control mutation uses internal bearer through authenticated gateway proxy; session endpoint smoke | Public transport auth and exact-origin CORS are residual; host-exposed MCP is local/trusted-network only |
| Gateway to Postgres | Control resources, ownership, lifecycle, metadata | Parameterized store implementations, shared store contract, readiness check | TLS, least-privilege DB identity, backup/restore, HA are #66/#73/#74 |
| Gateway to Vault | Credential-binding lookup and secret value | Secret-provider boundary and opaque binding metadata | Local Compose passes a dev root token by argument; production-safe identity/secret injection remains #66/#70 |
| Gateway to runtime broker | Typed operations and dedicated service credential | OIDC client credentials from read-only file; audience/client checks; internal broker API/auth networks; replay ledger | Private TLS/mTLS or equivalent service-mesh protection is required for non-local deployments |
| Runtime broker to Docker proxy | Validated browser/worker/storage operation translated to Docker API | Typed contract, immutable configured images, allowlisted inputs, internal network, digest-pinned proxy, API-family deny checks | Broker compromise still grants the proxy’s allowed container/volume authority |
| Broker to browser/worker runtime | Image, command, mounts, environment, resources, labels | Policy validation; no privileged/host namespaces/devices; no added capabilities; `no-new-privileges`; Docker default seccomp in broker mode; resource caps; dynamic worker credentials delivered over bounded one-shot stdin rather than inspectable env/command/files | Browser/worker writable roots and Docker-host co-location are residual deployment risks |
| Browser runtime to internet | Website requests through direct or configured egress | Project/profile scoping, proxy auth bindings, sanitized diagnostics/usage, explicit TLS-intercept mode | Proxy/SWG owns URL/content logs; network enforcement and data policy remain #66/#76/#79/#80 |
| Workflow source to gateway/worker | Git URL/ref, immutable commit, source archive, entrypoint | Scheme/path/helper validation, trusted local-root exception, symlink containment, immutable publish ref | Dependency execution inside a worker remains untrusted code and needs runtime isolation/supply-chain controls |
| Files/contexts to stores/runtimes | Uploads, workspace refs, session bindings, context archives | Opaque refs, relative mount paths, archive count/size/type/path limits, offloaded parsing, project policy | Remote object-store adapters, malware/DLP, residency, encryption remain #21/#76/#80 |
| Recorder to gateway/artifact store | Worker credential, staged WebM, completion metadata | Purpose-scoped worker token, exact session/recording staging path, regular-file and measured-byte validation | Durable object storage and backup policy remain deployment-owned |
| Gateway to callback receiver | Signed event, retry and delivery state | HTTPS/public-address default, DNS classification/pinning, no redirects/system proxy, exact-origin local exception, HMAC and retry tests | Receiver replay policy, key rotation, private connectivity, event export remain #28/#66/#70 |
| Gateway/broker to telemetry collector | Aggregate RED/capacity metrics, sanitized logs, and bounded browser-runtime traces | Fixed metric/span dimensions, W3C propagation, no raw path/session labels, no baggage, private-network guidance | Operator collector TLS/auth/access/retention and broader worker/store/event traces, SLOs, alerts, and capacity evidence remain #178/#66 |

## Threat And Control Matrix

| ID | Threat | Status and current control | Residual/owner |
| --- | --- | --- | --- |
| T-01 | Token theft, purpose confusion, expiry bypass, replay | Implemented purpose domains, constant-time HMAC validation, expiry tests, broker JWT audience/client/key checks, bounded broker idempotency ledger | Public ingress/TLS and lifecycle policy: #66/#70 |
| T-02 | Cross-owner, cross-project, or cross-session access | Owner-scoped APIs/stores and wrong-session automation/file tests are implemented | Descriptive mappings are not a complete grant model: #176/#177 |
| T-03 | Viewer escalates to controller or media/file authority | Exclusive-owner restricted viewers are filtered and enforced read-only in gateway and client | Multi-party policy/audit expansion: #79/#28 |
| T-04 | Compromised gateway gains Docker-host control | Broker profile removes Docker endpoint/network/socket/proxy from gateway; typed broker policy is tested | Gateway still owns control-plane data and allowed broker requests; isolate broker/host in #66 |
| T-05 | Compromised broker abuses Docker | Internal-only broker/proxy networks, service auth, policy, immutable images, constrained proxy API, no host-published ports | Allowed container/volume APIs remain powerful; separate host/orchestrator policy required by #66/#72 |
| T-06 | Runtime container escape or host-device abuse | Broker launches are unprivileged, no host namespaces/devices/cap additions, `no-new-privileges`, default seccomp, bounded resources | Browser/worker writable roots and kernel/browser sandbox assurance need deployment validation: #66/#72 |
| T-07 | Malicious website, workflow, extension, upload, or context archive | Runtime isolation, approved extension metadata, source validation/pinning, archive and file path limits | Malware/DLP and complete supply-chain governance: #75/#80 |
| T-08 | Secret leakage through APIs, process configuration, logs, telemetry, or debug output | Opaque credential bindings, redacted debug types, bounded telemetry, protected deployment files, and one-shot stdin delivery for broker-launched worker credentials | Local Vault root token/process args are dev-only; production rotation/workload identity lifecycle: #66/#70 |
| T-09 | SSRF or unsafe outbound destination | Callback URL/DNS/IP/redirect/proxy policy is implemented; browser egress can be profile-bound | Browser browsing is intentionally outbound; centralized policy/DLP: #79/#80 |
| T-10 | Path traversal, symlink/hardlink escape, or arbitrary artifact finalization | Context import limits/type rejection, source preview containment, recording staging boundary, relative session file paths | General artifact API/storage adapters: #21 |
| T-11 | Mutable or compromised build/runtime dependency | Proxy image digest and broker runtime-image immutability are enforced in the broker overlay; dependency scans run in CI | SBOM, signing, provenance, release and IP governance: #75/#180 |
| T-12 | Resource exhaustion or saturation hides as generic failure | Runtime/worker admission, quotas, body/archive/output limits, readiness, and aggregate metrics exist | Reproducible load envelopes, queue/subsystem metrics and SLOs: #164/#168/#169/#178 |
| T-13 | Sensitive or high-cardinality telemetry | Gateway metrics and the gateway-to-broker browser-runtime trace checkpoint use fixed dimensions; trace export excludes baggage, resource ids, URLs, credentials, browser content and raw errors; egress usage excludes URLs, headers, payloads, credentials, decrypted traffic and CA material | Operator collector isolation/retention and broader cross-process trace/log policy: #178/#66 |
| T-14 | MCP delegation or transport takeover | Bridge-global control mutation is internal-bearer protected and gateway-authorized; session-specific routing exists | MCP transports have no complete public inbound auth/origin boundary: #69/#72 |
| T-15 | Admin XSS, clickjacking, token persistence, or stale auth | Shared verified OIDC core, in-memory tokens, CSP, frame denial, referrer/content/permission headers and expired-auth smokes | Infrastructure TLS/origin and future cookie CSRF rules: #66/#72 |
| T-16 | Forged, redirected, replayed, or unavailable callback delivery | Signed events, destination policy, persisted retries/diagnostics and ordering tests | Key rotation, receiver replay window and generalized event export: #28/#70 |
| T-17 | Data loss, incomplete cleanup, or unrecoverable state | Retention workers, persisted assignments, restart reconciliation, lifecycle readiness | Backup/restore and HA drills: #73/#74 |
| T-18 | Protocol downgrade, parser ambiguity, or unsupported client behavior | Shared Rust/TypeScript protocol implementation and frame tests exist | Version negotiation, vectors, fuzzing and compatibility matrix: #175 |
| T-19 | External Workflow Endpoint caller bypasses project/grant scope, duplicates a browser side effect, or leaks process credentials/data | Active registered service-principal and exact endpoint-operation checks precede runtime creation; Draft 2020-12 input/output enforcement; endpoint/caller fingerprint conflicts; restricted projections; bounded outcome/artifact evidence; uncertain side effects disable retry guidance | Public ingress/TLS, client-secret rotation, external compensation, and broader integration lifecycle remain deployment/#70/#176/#240 |

## Negative-Test Evidence

This inventory names the rejection evidence expected to remain green. The
commands intentionally select behavior rather than implementation line numbers.

| Boundary | Required rejection | Current evidence | Focused command |
| --- | --- | --- | --- |
| Owner API authentication | Missing bearer cannot use versioned session or identity resources | `rejects_v1_session_routes_without_bearer_auth`; Compose `compose_identity_access_review_api_surface` | `cargo test -p bpane-gateway rejects_v1_session_routes_without_bearer_auth` |
| Owner isolation | A foreign principal cannot list, read, mutate, delegate, or delete another owner's session | `scopes_session_resources_to_the_authenticated_owner`; Compose `compose_session_ownership_boundaries_api_surface` | `cargo test -p bpane-gateway scopes_session_resources_to_the_authenticated_owner` |
| Session automation | A session token cannot address another session or call owner-only delete; file content is session-bound | `compose_automation_access_boundaries_api_surface`; `lists_downloads_and_scopes_runtime_session_files` | `cargo test -p bpane-gateway lists_downloads_and_scopes_runtime_session_files` |
| Credential domains | Connect, automation, admin-event, recording-worker, and other token purposes reject substitution, tampering, and expiry | `all_credentials_reject_cross_purpose_replay`; token-codec and per-manager expiry tests | `cargo test -p bpane-gateway all_credentials_reject_cross_purpose_replay` |
| Admin browser auth | Old/future/reused login transactions, wrong state/provider/key, failed refresh, missing tokens, and malformed event auth fail closed | `code/web/bpane-admin-auth/src/*.test.ts`; admin-new authenticated API and shell tests | `npm test --prefix code/web/bpane-admin-auth` |
| Admin browser defenses | Missing CSP, frame, MIME, referrer, or permissions policy fails the static contract | `scripts/security/admin-security-header-contract.test.mjs`; `scripts/ci/admin-security-headers-contract.test.mjs` | `node --test scripts/security/admin-security-header-contract.test.mjs scripts/ci/admin-security-headers-contract.test.mjs` |
| Broker authentication | Missing/expired token, wrong audience/client/key, malformed claims, and symmetric algorithms are rejected | `health_routes_are_public_but_operations_require_authentication`; `maps_expired_wrong_audience_client_and_key_failures`; auth unit tests | `cargo test -p bpane-runtime-broker maps_expired_wrong_audience_client_and_key_failures` |
| Broker replay/idempotency | A reused request id with changed body/key or principal is denied while exact retries are stable | `exact_retry_is_cached_and_conflicting_reuse_is_denied`; ledger conflict/capacity tests | `cargo test -p bpane-runtime-broker exact_retry_is_cached_and_conflicting_reuse_is_denied` |
| Callback delivery | Unsafe URL forms, non-public/mixed DNS answers, redirects, implicit proxies, and unapproved local targets are denied | `workflow_event_delivery::destination_policy::tests` and delivery client tests | `cargo test -p bpane-gateway workflow_event_delivery` |
| Workflow Endpoint machine access | Missing/interactive, unregistered, disabled, cross-project, insufficient-scope, wrong-operation, and foreign-caller tokens are denied before runtime side effects; changed idempotency payload conflicts | `api::tests::workflow_endpoints`; in-memory/Postgres shared store contract; fake-BPM Compose smoke | `cargo test -p bpane-gateway workflow_endpoints` plus `npm run smoke:workflow-endpoint-compose -- --headless` |
| Browser-context archives | Oversized archives, expansion/entry/path limits, traversal, links, devices, and FIFOs are rejected | `api::browser_context_archive::tests`; authenticated import capacity test | `cargo test -p bpane-gateway browser_context_archive` |
| Recording finalization | Outside, mismatched, missing, directory, symlink, parent-symlink, and non-regular staging sources are rejected | `recording::artifact_store::tests`; worker/session/recording binding API tests | `cargo test -p bpane-gateway recording::artifact_store` |
| Runtime policy | Mutable images, arbitrary environment, escaping mounts/socket paths, unsafe endpoints/networks, host privileges, weakened broker confinement, and direct gateway Docker access are rejected | runtime-broker browser/worker/storage tests and structured negative Compose fixtures | `cargo test -p bpane-runtime-broker` plus `node --test scripts/validate-runtime-broker-*.test.mjs` |

The owner and automation Compose cases are the end-to-end proof for the first
three rows and require the supported local stack:

```bash
cargo test -p bpane-gateway --test compose_api_surface \
  compose_identity_access_review_api_surface -- --ignored --test-threads=1
cargo test -p bpane-gateway --test compose_api_surface \
  compose_session_ownership_boundaries_api_surface -- --ignored --test-threads=1
cargo test -p bpane-gateway --test compose_api_surface \
  compose_automation_access_boundaries_api_surface -- --ignored --test-threads=1
```

Audit result for #223: no missing negative test was found inside the bounded
credential, authorization, callback, archive, artifact, or broker-policy
acceptance scope. Public MCP transport inbound authentication and exact-origin
policy remain a real residual gap owned by #69/#72; they are not reclassified
as implemented evidence by this table.

## Security Invariants

Changes must preserve these invariants:

1. Owner, connect, automation, admin-event, recording-worker, and broker service
   credentials are not interchangeable.
2. Raw bearer credentials, secret values, requested URLs, browser content,
   decrypted traffic, and CA private material do not enter logs or metrics.
3. The broker and single-node profiles give the gateway no Docker socket,
   endpoint, proxy dependency, or Docker-control network membership.
4. Runtime broker and Docker proxy do not publish host ports, and only the proxy
   mounts the Docker socket.
5. Broker operations validate image, command, environment, mounts, networks,
   resources, ownership, idempotency, and security posture before execution.
6. Browser and worker containers are not privileged and do not receive host
   namespaces, host devices, or caller-selected Linux capabilities.
7. Owner-visible resources do not expose raw filesystem paths or secret values.
8. Callback delivery denies redirects, implicit proxies, and unsafe destination
   addresses unless an exact local/deployment exception is configured.
9. Archive and artifact boundaries reject traversal, links, special files,
   wrong-resource paths, and unbounded expansion.
10. Health, readiness, metrics, and diagnostic routes do not start a stopped
    session or mutate lifecycle state.
11. Local-development exceptions remain explicitly named and cannot be cited as
    production controls.
12. Broker-launched dynamic worker credentials remain absent from Docker
    environment, command, inspectable worker files, and general logs.

The executable baseline is `node scripts/check-production-security-baseline.mjs`.

## Residual Risks

| Risk | Current position | Owner |
| --- | --- | --- |
| Bounded single-node package lacks target network-policy and production acceptance evidence | Repository qualification only | #66 |
| Incomplete MCP public transport auth/origin control | Trusted local/private exposure only | #69/#72 |
| Organization/project mappings not fully enforced as grants | Owner boundary works; enterprise role model incomplete | #176/#177 |
| Gateway/Vault production service identity and secret rotation incomplete | Dev token works; production credential lifecycle unresolved | #66/#70 |
| Broker/Docker co-location and runtime sandbox envelope not qualified across targets | Local Docker evidence only | #66/#72 |
| No formal protocol version/conformance/fuzz baseline | Current clients share implementation | #175 |
| No complete trace/SLO/alert/load envelope | Gateway metrics plus bounded gateway-to-broker browser-runtime trace prototype only | #178 |
| Backup, restore, HA, and zero-downtime behavior not qualified | Restart reconciliation is not DR/HA | #73/#74 |
| SBOM, signing, provenance, vulnerability intake, contribution and IP policy incomplete | CI dependency checks are not release governance | #75/#180 |
| Residency, BYOK, central policy and DLP incomplete | Project policy and egress controls are partial | #76/#79/#80 |

Review this model whenever a public boundary, credential type, runtime adapter,
storage adapter, callback type, deployment profile, or sensitive data class is
added or materially changed.
