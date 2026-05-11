# BrowserPane Open Product Readiness

Date: 2026-05-11

Status: active backlog after PR #83. This document intentionally contains only
work that is not handled by the first admin hardening slice.

## Scope

PR #83 completes the consolidated #65 slice: local file policy, session file
visibility, session-bound MCP routing, remote deployment notes, and the first
smoke-covered admin operations surface.

This file replaces the old local `*_PLAN.md` planning notes. Those notes mixed
completed work, historical analysis, and future work. Keep this document
focused on the remaining gaps; completed validation logs and commit lists belong
in PRs, issues, and release notes.

## Active Issue Map

- Umbrella roadmap: #6, #7, #63.
- Admin productization: #82.
- Automation and workflow APIs: #69, #68, #47.
- Artifacts, logs, events, and support evidence: #43, #21, #20, #28, #29,
  #30, #81, #42.
- Identity, governance, and policy: #52, #70, #72, #76, #77, #78, #79, #80.
- Deployment and operations: #66, #73, #74, #75, #34.
- Session resource model, network, and egress: #15, #19, #22, #24, #25, #26,
  #27, #31, #33.
- Handoff and challenge handling: #67, #71.
- Developer and experimental surface: #16, #35, #36, #37, #38, #39, #40, #41.

Closed or PR-covered issues should not reappear here unless a new gap is found.

## P0: Admin Console Productization

Issue focus: #82, #63.

The admin app now has a useful single-page operations surface. It still needs a
durable resource model and navigation structure before it is production-ready.

Remaining work:

- Add route-level pages for sessions, workflow definitions, workflow runs,
  recordings, automation tasks, file workspaces, credentials, extensions,
  operations, and audit.
- Add search, filtering, sorting, pagination, stale-state indicators, and deep
  links for high-volume resource lists.
- Build session detail pages that show lifecycle state, runtime assignment,
  owner mode, labels, timestamps, connection summaries, stop blockers, linked
  recordings, linked workflow runs, files, automation tasks, and diagnostics.
- Build workflow run detail pages with state, logs, events, produced files,
  workspace inputs, credentials used by binding id, recordings, linked session,
  and intervention controls.
- Keep the live browser as an operator tool, but make historical inspection
  possible without connecting to a running browser.

Acceptance:

- Operators can find and inspect a session, run, recording, file, credential, or
  extension through a stable URL.
- Reloading the admin app does not erase the operator's ability to reconstruct
  what happened.
- Destructive controls explain disabled states and capture an operator reason
  where appropriate.

## P0: Durable Observability, Events, And Support Evidence

Issue focus: #20, #28, #29, #30, #81, #42.

Gateway snapshot events and local UI diagnostics are useful, but they are not a
complete support timeline.

Remaining work:

- Add persisted session and workflow timelines with severity, source, actor,
  resource id, correlation metadata, and timestamps.
- Capture or expose browser console, page error, network metadata, tab/page
  inspection state, transport state, recording events, workflow events, file
  events, MCP delegation changes, policy checks, and operator actions.
- Add redacted support bundle export for sessions and workflow runs.
- Add compact diagnostic summaries that are safe to copy into issues or support
  tickets.
- Make redaction rules explicit and testable for secrets, credentials,
  clipboard content, URLs, headers, and uploaded/downloaded file metadata.

Acceptance:

- Support can receive one redacted artifact and understand the lifecycle without
  database, Docker, or browser inspection.
- Sensitive values are excluded by default.
- Event and log retention behavior is visible to operators.

## P0: Automation And Workflow API Productization

Issue focus: #69, #68, #47.

Session-scoped MCP endpoints are now the recommended model, but the broader
automation surface still needs product contracts.

Remaining work:

- Productize session-scoped automation connection APIs with clear auth,
  discovery, lifecycle, health, and client compatibility behavior.
- Add quick browser action APIs for screenshots, PDFs, snapshots, extraction,
  and other common one-shot actions without requiring a long-lived MCP client.
- Productize workflow publishing and supported execution interfaces, including
  source versioning, immutable refs, credential policy, workspace inputs,
  extension allowances, and run compatibility.
- Define whether automation clients need signed URLs, owner-issued nonces,
  service-principal scopes, or all of the above.
- Add wrong-session, expired-token, revoked-delegate, and concurrent-client
  regression coverage for supported automation entrypoints.

Acceptance:

- An external automation client can discover, connect to, and operate on exactly
  one intended session without relying on a mutable global bridge target.
- One-shot browser actions and workflow runs have stable request/response
  contracts and auditable outcomes.

## P0: Operations, Deployment, And Resilience

Issue focus: #34, #66, #73, #74, #75.

The compose stack is good for development and smoke coverage. Production needs
explicit deployment and operating models.

Remaining work:

- Support and document Compose, Kubernetes, and AWS Fargate deployment paths.
- Add runtime capacity, queue/admission, active session, active workflow,
  active recorder, join latency, start latency, retention, and artifact-store
  health surfaces.
- Add backup, restore, and disaster recovery runbooks for Postgres, artifact
  storage, profiles, workflow source metadata, and secrets.
- Add high-availability and zero-downtime operation support for gateway,
  workers, event delivery, and runtime pools.
- Add release governance and supply-chain controls for container images,
  dependencies, generated artifacts, signing, provenance, and rollback.

Acceptance:

- Operators can see whether the system has capacity and where failures are
  accumulating.
- Deployment docs describe the supported runtime topology and the backup/restore
  boundary for every persistent resource.

## P1: Governance, Identity, Policy, And Compliance

Issue focus: #52, #70, #72, #76, #77, #78, #79, #80.

Remaining work:

- Add external identity and service-principal lifecycle management.
- Add API key, audit log, retention policy, and access review controls.
- Add a central enterprise policy engine for session creation, automation,
  file/artifact access, recording, clipboard, downloads, extension use,
  credential use, network egress, and sharing.
- Add security event export, alerting, and SIEM integration.
- Add DLP and content inspection hooks for files and artifacts.
- Add data residency, encryption, and BYOK controls.
- Add an enterprise hardening baseline and threat model covering local compose,
  remote self-hosted, Kubernetes, and managed-cloud deployment assumptions.

Acceptance:

- Operators can explain who accessed what, under which policy, with which
  retention and export obligations.
- Sensitive operations are auditable and policy-gated.

## P1: Files, Artifacts, Recordings, Credentials, And Extensions

Issue focus: #21, #43, #82.

Remaining work:

- Add first-class artifact APIs for downloads, recordings, screenshots, PDFs,
  workflow outputs, diagnostic bundles, and support bundles.
- Add recording storage backends beyond local filesystem and a supported
  single-file export path.
- Add file workspace list/create/detail/upload/download/delete flows.
- Add session file binding create/detail/content/delete flows with clear
  distinction between workspace files, runtime uploads/downloads, and workflow
  produced files.
- Add credential binding list/create/detail/health flows without exposing raw
  secret values.
- Add extension definition/version/enable/disable flows, provenance, allowed
  workflow usage, and unsupported-backend messaging.

Acceptance:

- Operators can manage reusable files, credentials, extensions, and recording
  artifacts without curl, database access, or Docker inspection.
- Artifact provenance and retention are visible from the related session or run.

## P1: Session Resource Model

Issue focus: #15, #19, #22, #24, #25, #26, #27, #31, #33.

Remaining work:

- Add first-class session template resources and a session creation
  configurator for owner mode, idle timeout, labels, display size, recording
  mode, file bindings, extension set, runtime policy, network identity, and
  profile/context binding.
- Add metadata-aware session querying and filtering.
- Add explicit keep-alive and release semantics for live runtimes.
- Add projects or namespaces with quota and policy boundaries.
- Add browser context save/restore plus export/import for reusable contexts.
- Add network identity and operator-defined egress profiles with upstream
  proxy, bypass rules, and custom CA trust where supported.
- Add first-class mobile and device-mode sessions if they become a supported
  product target.

Acceptance:

- Session creation is reproducible and inspectable.
- Operators can separate ownership, policy, quota, profile, and runtime
  lifecycle concerns.

## P1: Handoff And Challenge Handling

Issue focus: #67, #71.

Remaining work:

- Add signed live-view links with scope, expiry, one-use options, and revocation.
- Add human handoff resources tied to sessions and workflow runs.
- Add policy-gated browser challenge detection and human fallback.
- Make view-only, control, automation, recorder, and owner capabilities visually
  and contractually distinct.

Acceptance:

- Session owners can safely share or transfer a browser session with explicit
  scope, expiry, and auditability.
- Browser challenge handling does not require ad hoc operator coordination.

## P2: Developer Experience And Experiments

Issue focus: #16, #35, #36, #37, #38, #39, #40, #41.

Remaining work:

- Add a BrowserPane CLI for session lifecycle, artifacts, workflow runs,
  diagnostics, and admin operations.
- Keep experimental features separate from the production contract until their
  safety and support model is clear:
  - speculative session checkpoints
  - page affordance graph resources
  - private input enclaves
  - workflow compilation from successful sessions
  - site twin resources
  - transactional browser actions with approval gates
  - independent verifier runs

Acceptance:

- Developer workflows have supported tools, but experiments do not silently
  expand the production support surface.

## Gateway Engineering Debt To Verify

The previous local gateway audit identified risks that should be verified
against the current code before opening implementation issues:

- Git-backed workflow source fetching needs strict URL/ref validation,
  sandboxing, shallow clone behavior, and disk/memory limits.
- Token validation should use constant-time comparisons and unambiguous subject
  namespaces.
- JWKS refresh and auth validation should avoid thundering-herd behavior.
- Runtime allocation, recording launch, workflow launch, hub subscription, and
  MCP-owner transitions need race-condition review under concurrent load.
- Upload, download, workflow output, event delivery, and archive generation
  paths need request/body/payload size limits and streaming behavior where
  appropriate.
- Gateway shutdown should drain transport, API, background task, recorder, and
  worker lifecycle managers predictably.
- `SessionStore`, API resource types, and cross-subsystem helper modules should
  continue moving toward smaller ownership boundaries before any crate
  extraction.

Acceptance:

- Each verified risk becomes either a closed non-issue with evidence or a
  focused GitHub issue with reproduction, severity, and validation criteria.
