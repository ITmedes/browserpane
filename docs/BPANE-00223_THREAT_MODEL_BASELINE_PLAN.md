# BPANE-00223 Threat Model Baseline Plan

Issue: [#223 Establish an evidence-linked threat model and hardening baseline](https://github.com/ITmedes/browserpane/issues/223)

Parent: [#72 Enterprise security hardening baseline and threat model](https://github.com/ITmedes/browserpane/issues/72)

Status: in progress

## Business Case

BrowserPane has accumulated meaningful security controls across credential
domains, browser authentication, callback delivery, archive processing,
recording finalization, runtime brokerage, storage, and telemetry. Those
controls are currently distributed across code, manifests, tests, architecture
documents, and completed implementation plans. A reviewer cannot yet follow one
current threat model from an external request to a browser or worker runtime and
determine which controls are enforced by BrowserPane, which are deployment
requirements, and which remain residual risks.

This checkpoint creates that auditable baseline before Kubernetes, managed-cloud,
or public deployment packaging expands the number of supported trust boundaries.
It does not turn local Compose into a production deployment and does not use a
document as a substitute for an unimplemented control.

## Example Use Case

A security reviewer evaluates a proposed BrowserPane deployment for an approved
business workflow that uses credentials, downloads files, and retains recording
evidence. Starting from the external caller, the reviewer can trace data and
authority through the web ingress, gateway, runtime broker, browser and workflow
containers, Vault, Postgres, artifact stores, callback delivery, and telemetry.
For each boundary, the reviewer sees:

- the assets and data classes that cross it,
- the relevant attacker or failure mode,
- the implemented control and executable evidence,
- the required infrastructure or operator decision,
- the residual risk and canonical follow-up issue.

The reviewer can then reject an unsafe topology, accept a bounded Pilot profile,
or request a specific control without relying on an unsupported general
"enterprise-ready" claim.

## Current Findings

1. Purpose-scoped gateway credentials, shared browser OIDC handling, webhook
   destination policy, bounded browser-context imports, trusted recording
   finalization, and broker service credentials have strong focused evidence.
2. The broker topology removes Docker access from the gateway and validates
   browser, worker, and storage operations, but the overlay remains a local
   production-like test fixture rather than deployment packaging.
3. `deploy/compose.yml` is explicitly a local development stack and includes
   deliberate exceptions such as development credentials, published dependency
   ports, Vault dev mode, Keycloak dev mode, and an unconfined direct-runtime
   compatibility path. These must never be presented as production defaults.
4. Admin response headers have a machine-checked baseline. Existing runtime
   validators cover network membership, immutable images, file-backed service
   secrets, gateway Docker isolation, and Docker API denial, but the integrated
   production-security evidence is not discoverable as one validation stage.
5. Negative tests exist for wrong-purpose/expired credentials, cross-session
   automation and file access, broker audience/client/replay failures, webhook
   origin/address policy, and runtime privilege policy. The final threat matrix
   must cite these tests and identify genuine gaps rather than duplicating them.
6. MCP host exposure and wildcard CORS remain a known hardening gap when an MCP
   transport is exposed outside a trusted local network. The threat model must
   keep that visible and assign implementation ownership without silently
   treating the local fixture as a supported public endpoint.
7. Metrics are aggregate and sanitized, but the unauthenticated scrape endpoint
   depends on private collector-network exposure owned by deployment packaging.

## Scope

### 1. Integrated threat model

Add `docs/THREAT_MODEL.md` with:

- supported and explicitly unsupported deployment assumptions,
- assets and data classification,
- human and machine actors,
- attacker capabilities and compromised-component assumptions,
- trust-boundary and data-flow inventory,
- threats grouped by boundary rather than by historical implementation slice,
- implemented controls with code, manifest, API, and test evidence,
- residual risks and canonical owners,
- security invariants that future changes must preserve.

The model must cover:

- web ingress, admin-new, shared OIDC, and admin events,
- owner API, worker/internal API, health/readiness, metrics, and WebTransport,
- MCP bridge and direct session automation,
- runtime broker, Docker proxy, browser runtime, and worker runtimes,
- workflow source, extensions, browser contexts, and uploaded files,
- credential bindings and Vault,
- Postgres, workspace/session files, recordings, and artifact stores,
- egress proxying, TLS observation, callbacks, and sanitized usage reporting,
- logs, metrics, future traces, and diagnostic evidence.

### 2. Production-hardening baseline

Add `docs/PRODUCTION_SECURITY_BASELINE.md` that classifies every requirement as:

- application-enforced,
- required deployment configuration,
- infrastructure/operator responsibility,
- local-development exception,
- deferred with a canonical issue.

The checklist must cover TLS/origins, identity, service credentials, secret
files, network isolation, runtime security, storage, egress, retention,
telemetry, callbacks, dependency exposure, backup/restore, upgrades, and
incident response. It must link #66 for packaging instead of defining an
unvalidated production manifest in this slice.

### 3. Executable security contract

Add a focused Node validation module and wrapper that:

- parse the structured base Compose and runtime-broker overlay,
- compose the existing Docker-proxy and broker-boundary validators,
- enforce selected high-risk broker/proxy service hardening invariants not
  already covered, including no host-published broker/proxy ports,
  `no-new-privileges`, capability dropping, read-only broker root, bounded
  writable temporary storage, read-only secret/config mounts, and gateway
  Docker isolation,
- reuse or invoke the existing admin security-header contract,
- verify the local stack and security baseline retain explicit development-only
  classification,
- fail with specific control-oriented messages,
- include negative fixtures for every new invariant.

Register the contract in the canonical fast validation profile. Avoid parsing
YAML with regular expressions and avoid copying existing validator logic.

### 4. Negative-test evidence audit

Build a compact evidence table for:

- unauthenticated API access,
- owner/cross-principal visibility,
- wrong-session automation,
- wrong-purpose and expired credentials,
- broker audience/client/key and idempotency replay failures,
- unsafe callback destinations and redirects,
- unsafe recording/import/archive paths,
- runtime privilege, mount, image, environment, and network escalation.

Add new code-level tests only where the audit discovers a concrete acceptance
gap within this checkpoint. Record wider feature gaps against their current
issues.

### 5. Documentation and governance synchronization

Update the durable entry points so reviewers can find the baseline:

- `README.md` and `ARCH.md`,
- `docs/README.md`,
- `docs/DELIVERY_ROADMAP.md`,
- `docs/OPEN_ISSUES_CONTEXT.md`,
- `docs/CAPABILITY_MATURITY_MATRIX.md`,
- `docs/RISK_REGISTER.md`,
- `docs/VALIDATION_MATRIX.md`,
- `AGENTS.md` only if architecture ownership or a runnable validation command
  changes.

OpenAPI changes are required only if implementation discovers or changes an API
contract. Pure threat-model classification must not modify the frozen API.

## Architecture Decisions

### Evidence before claims

Every implemented control must link to current evidence. A requirement without
evidence is marked required or deferred, not implemented. Historical plan status
alone is not evidence when the current code or manifest disagrees.

### Profiles are named explicitly

Use these terms consistently:

- `local development`: canonical Compose with deliberate test conveniences,
- `production-like broker validation`: local Compose plus the authenticated
  broker overlay used to prove the Docker trust boundary,
- `production deployment`: not yet a supported package; owned by #66.

### Component compromise is in scope

The model assumes malicious websites and uploaded content are untrusted. It also
considers a compromised browser/worker runtime and a compromised gateway. The
broker boundary limits gateway Docker authority; it does not make a compromised
gateway harmless because the gateway still owns control-plane data and broker
requests allowed by its service identity.

### No sensitive evidence generation

Validation output may identify control names, component names, and fixed test
fixtures. It must not emit owner/session identifiers, bearer material, secret
file content, requested URLs, browser content, decrypted traffic, or raw CA
material.

## Implementation Steps

1. Create the threat-boundary, data-flow, threat/control, and negative-evidence
   inventories from current code and manifests.
2. Implement the structured production-security contract and negative fixtures;
   register it in the fast validation profile.
3. Write the threat model and production-hardening baseline against the verified
   inventory and contract.
4. Add any narrowly justified negative tests discovered by the audit.
5. Synchronize durable docs, risk/maturity/validation state, and GitHub issue
   evidence.
6. Run focused, fast-profile, and live broker-topology validation before PR.

## Acceptance Criteria

- The threat model is current, navigable, and covers every named boundary in
  #223 without treating local Compose as production.
- Every material threat has implemented evidence, a required deployment
  control, or a canonical residual-risk owner.
- The production-hardening checklist separates BrowserPane, operator, and
  infrastructure responsibilities.
- The new contract composes existing structured validators and catches each new
  hardening regression through a negative fixture.
- Key credential, authorization, callback, archive, artifact, and runtime-policy
  negative paths are cited and remain green.
- README, architecture, roadmap, maturity, risk, validation, and issue maps agree
  with the implementation state.
- No API, production-readiness, compliance, HA, or capacity claim exceeds the
  available evidence.

## Post-Implementation Smoke Sequence

1. Run the new production-security contract and all of its negative fixtures.
2. Run repository document, link/path, YAML, and validation-tool tests.
3. Run the existing Docker runtime-boundary and runtime-broker foundation/browser
   overlay contracts.
4. Run gateway credential-domain, API authorization, webhook, archive,
   recording-finalization, and automation-boundary tests.
5. Run runtime-broker authentication, ledger/replay, browser policy, worker
   policy, and storage policy tests.
6. Run shared admin auth tests and the admin security-header contract.
7. Run the canonical fast validation profile.
8. Start canonical Compose with the broker overlay and run live broker isolation:
   prove gateway Docker denial, authenticated browser launch, session API,
   admin-new session smoke, MCP session endpoint, workflow, and recording paths.
9. Inspect validation and service logs for the fixed test secret/session markers
   and confirm they are absent.
10. Run `git diff --check` and verify only intentional source/document changes
    are committed.

## Out Of Scope

- Formal penetration testing or compliance certification.
- Kubernetes, Fargate, Cloud Run, or managed-cloud deployment packaging (#66).
- Complete organization/project authorization and provisioning (#176/#177).
- Vulnerability intake, contribution, license, and IP governance (#180).
- HA, DR, backup/restore, residency, encryption, BYOK, DLP, and central policy
  implementation (#73/#74/#76/#79/#80).
- Full OpenTelemetry, SLO, alert, dashboard, and capacity work (#178).
- Public MCP transport productization or generalized resource events (#69/#28).
