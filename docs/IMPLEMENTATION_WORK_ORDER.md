# Consolidated Implementation Work Order

Created: 2026-07-07
Revalidated: 2026-08-20

This file preserves the detailed rationale and topic inventory across the
active `docs/` workspace. It is broader than the review cleanup plan: it maps
security, runtime, admin-new, domain-resource, identity, operator,
documentation, and refactoring topics into candidate shippable slices.

`DELIVERY_ROADMAP.md` is the canonical source for current execution order,
states, dependencies, and release gates. The numbered items below are retained
for traceability and must not override the roadmap when evidence changes the
next Ready slice.

For live GitHub issue context, use `OPEN_ISSUES_CONTEXT.md`. It maps every
currently open issue to the docs and work-order items below. Use
`CAPABILITY_MATURITY_MATRIX.md`, `PRODUCT_PHASES_AND_RELEASE_GATES.md`, and
`RISK_REGISTER.md` before making readiness or external claims.

## Source Scope

This order incorporates the active requirements from:

- `NEXT_WORKING_ROADMAP.md`
- `SECURITY_RUNTIME_ROADMAP.md`
- `ADMIN_NEW_STATUS.md`
- `ADMIN_NEW_REQUIREMENTS.md`
- `ADMIN_NEW_API_COVERAGE.md`
- `ADMIN_NEW_IMPLEMENTATION_GUARDRAILS.md`
- `ADMIN_INTERACTION_REQUIREMENTS.md`
- `DOMAIN_REQUIREMENTS.md`
- `RESOURCE_LIFECYCLE_REQUIREMENTS.md`
- `PROJECT_GOVERNANCE_REQUIREMENTS.md`
- `IDENTITY_ACCESS_REQUIREMENTS.md`
- `RUNTIME_OPERATOR_REQUIREMENTS.md`
- `VALIDATION_MATRIX.md`
- `REVIEW_FINDINGS_RECONCILIATION.md`
- `REVIEW_FINDINGS_COVERAGE_AUDIT.md`
- `OPEN_ISSUES_CONTEXT.md`

## Selection Criteria

Priority is based on:

1. whether the repository can validate the change reliably,
2. production and multi-tenant risk reduction,
3. dependency order,
4. bounded Pilot value or admin-new promotion impact,
5. ability to ship in one coherent PR,
6. testability and smoke coverage,
7. how much future work the slice unlocks.

The 2026-07-31 audit moved #151 ahead of #145 because the repository has no
required CI status checks and has open critical/high dependency findings. The
current Foundation sequence is #151, #145, #146, #147, #150, #152, with #148
and #149 selected when the target Pilot uses imported contexts or recording
evidence.

The Admin-New promotion gate completed through PRs #210 and #211.
`/admin-new/` is the default console; `/admin/` remains a compatibility fallback
until a separate removal decision and regression gate are accepted.

## Current Baseline

These areas are treated as implemented or baseline-protected. Keep regression
coverage when touching adjacent code:

- `/admin-new/` shell, dashboard, projects, browser-context catalog, egress
  profile catalog, file-workspace catalog, sessions catalog, session creation,
  session preview popup, recordings overview, workflows catalog/detail, and
  workflow-runs overview,
- workflow git-source hardening,
- workflow source-preview symlink/path containment,
- bridge-local `/control-session` authorization,
- planning docs are tracked under `docs/`.

## Priority Tiers

| Tier | Meaning | Typical source docs |
| --- | --- | --- |
| P0 | Security/admin trust blockers before production-shaped exposure or admin-new promotion. | `SECURITY_RUNTIME_ROADMAP.md`, review reconciliation, admin auth requirements |
| P1 | Promotion foundation: runtime lifecycle, validation guardrails, backend contracts, and admin architecture prerequisites. | `NEXT_WORKING_ROADMAP.md`, `ADMIN_NEW_*`, `VALIDATION_MATRIX.md` |
| P2 | Admin-new parity and resource workflow completion. | `DOMAIN_REQUIREMENTS.md`, `RESOURCE_LIFECYCLE_REQUIREMENTS.md`, `IDENTITY_ACCESS_REQUIREMENTS.md` |
| P3 | Scalability, operator completeness, docs accuracy, and runtime hardening not required for the next admin slice. | runtime/operator docs, project governance docs, review performance docs |
| P4 | Larger refactors and enterprise/product backlog. | review maintainability docs, missing-feature docs, deferred enterprise sections |

## Work Order

### 1. Token Domain Separation And URL Credential Cleanup

Tier: P0.

Why first:

- It is confirmed open in the current tree.
- It crosses API, transport, logs, and both admin apps.
- It is a trust-boundary cleanup with limited product-design ambiguity.

Scope:

- separate connect-ticket and automation-token signing domains or bind purpose
  into the MAC,
- use constant-time signature verification,
- redact query material from WebTransport and admin-event logs,
- replace raw owner bearer query auth for admin events with a scoped
  event-stream credential,
- update the current old-admin event consumer and establish one reusable secure
  event credential/client contract for admin-new observability integration.

Validation:

- wrong-purpose token rejection tests,
- malformed and expired-token tests,
- transport log redaction tests,
- old-admin event-stream/reconnect smokes,
- admin-new auth/session regression coverage when shared credential issuance
  changes; admin-new does not yet have an event-stream consumer.

### 2. Shared Admin Browser Auth And Web-Security Hardening

Tier: P0.

Status: implemented and merged by #146 through PR #190.

Why next:

- It removes duplicated security-sensitive auth behavior across the two admin
  apps.
- It is a prerequisite for promoting admin-new.
- It directly addresses browser-facing auth findings.

Scope:

- extract or align shared admin auth logic,
- add OIDC nonce validation,
- verify ID-token issuer, audience, and signature before using identity claims,
- reduce JavaScript-readable refresh-token exposure where practical,
- add CSP and standard security headers to the static serving path,
- keep demo credentials documented as local-only.

Validation:

- auth unit tests for nonce, state, logout, and refresh,
- negative ID-token validation tests,
- security-header checks,
- old admin and admin-new login/logout/expired-auth smokes.

### 3. Webhook SSRF Controls

Tier: P0/P1.

Status: implemented and validated by #147; merged through PR #191.

Why here:

- It is a high-risk network boundary.
- It is independent enough to ship without touching admin-new layout work.
- It protects workflow event subscriptions before broader automation work grows.

Scope:

- parse and canonicalize `target_url` with the `url` crate,
- reject every non-public answer by default, including loopback, link-local,
  private, multicast, unspecified, shared, documentation, benchmarking, and
  reserved addresses,
- resolve immediately before dispatch and pin approved answers into `reqwest`,
- disable redirects and implicit system proxy discovery,
- permit controlled HTTP/non-public receivers only through exact-origin
  deployment configuration.

Validation:

- 21 focused URL, address, DNS, redirect, API, dispatch, signing, retry, and
  ordering tests,
- 382 passing gateway tests,
- dependency safety across Cargo and seven npm lockfiles,
- 16 passing authenticated compose API surfaces, including successful delivery
  to the explicitly allowed fixed receiver.

### 4. Browser Context Import Safety

Tier: P1.

Why here:

- It is a confirmed DoS and archive-safety gap.
- It has clear acceptance criteria.
- It protects a resource type already exposed in admin-new.

Scope:

- restore or replace request body limits,
- cap declared and actual uncompressed profile size,
- cap ZIP entry count and manifest size,
- reject symlink and hardlink tar members,
- move CPU-heavy archive parsing off async runtime threads where needed.

Validation:

- oversized ZIP/profile tests,
- multiple-profile and missing-manifest tests,
- symlink/hardlink rejection tests,
- existing browser-context clone/export/import tests.

### 5. Recording Artifact Finalization Boundary

Tier: P1.

Why here:

- Recording download/export is already visible to operators.
- The current worker completion path is a file-boundary cleanup, not a UX
  feature.
- It reduces risk before recording routes become deeper in admin-new.

Scope:

- treat recording completion as trusted-worker-only behavior,
- enforce a configured staging root,
- canonicalize and reject relative paths, path escapes, symlinks, directories,
  and wrong-session paths,
- prefer opaque staging artifact IDs if the API can evolve cleanly.

Validation:

- staging-path unit tests,
- API negative tests for each rejected path shape,
- recording-worker build/test,
- compose recording smoke with downloadable artifact/export.

### 6. Gateway Lifecycle, Health, And Readiness

Tier: P1.

Status: implemented and fully validated on `feature/BPANE-00150`; under review.

Why here:

- It is the highest effort-adjusted operational gap from the review.
- It makes local and future production restarts less destructive.
- It gives later smoke/e2e suites stable readiness probes.

Scope:

- add SIGINT/SIGTERM handling,
- use graceful shutdown for the API server,
- define bounded WebTransport drain behavior,
- add `/healthz`,
- add dependency-aware `/readyz` for Postgres, runtime manager, Vault when
  configured, and artifact stores.

Validation:

- focused lifecycle/readiness and timeout tests,
- compose dependency-loss/recovery smoke for `/healthz` and `/readyz`,
- real SIGTERM smoke proving readiness withdrawal before bounded listener
  shutdown,
- affected session, MCP, workflow, and recording regression smokes before
  review.

### 7. Minimal CI, Dependency Safety, And Validation Ratchet

Tier: P0 Foundation enabler; current first product slice under #151.

Why it now executes first:

- It is the main guardrail for every following refactor.
- It should exist before auth, store, admin, or smoke-helper changes are relied
  on as gate evidence.
- It turns existing manual quality expectations into enforced checks.
- The live dependency baseline contains open critical/high findings.

Scope:

- Rust fmt, clippy, and tests,
- Node TypeScript checks, tests, and builds for relevant packages,
- admin-new build and focused smokes where feasible,
- `node --check` or TS migration coverage for operational scripts,
- dependency-vulnerability checks for Rust and every committed Node lockfile,
- remediation of patched critical/high advisories, with any temporary exception
  documenting scope, reachability, owner, and expiry,
- coverage baselines and an explicit no-unexplained-regression rule,
- GitHub Actions checks required by branch protection rather than an optional
  local-only validation path,
- lightweight path/doc checks for AGENTS, README, and ARCH drift.

Validation:

- CI passes on the branch,
- local script or documented command sequence mirrors CI,
- the current Dependabot baseline is reconciled with the local dependency scan,
  and no unreviewed critical/high finding remains.

### 8. Postgres Session-Control Store Contract Tests

Tier: P1 enabler.

Why here:

- Backend divergence is the largest silent maintainability risk.
- Store refactors should not start before backend behavior is pinned.
- It supports later catalog scalability work.

Scope:

- shared behavioral suite for in-memory and Postgres stores,
- seed coverage for sessions, projects, workflow runs, recordings, templates,
  browser contexts, egress profiles, service principals, and identity mappings,
- keep ignored compose e2e but add faster in-tree contract coverage where
  practical.

Validation:

- contract suite runs against both backends,
- negative cases cover visibility, ownership, project scoping, and lifecycle
  transitions.

### 9. Admin-New Pattern, API Client, And Feedback Consolidation

Tier: P1/P2 enabler.

Why before many more routes:

- It prevents repeated scaffolding from multiplying.
- It should follow the auth decision so helpers do not encode a stale token
  model.
- It makes remaining admin-new route work cheaper and more consistent.

Scope:

- consolidate repeated catalog/detail/form patterns,
- consolidate admin API clients, mappers, and selector manifests,
- align global and panel-local feedback components,
- extract shared smoke helpers where duplication is already clear,
- keep route view models DOM-free and individually tested.

Validation:

- route-specific component and view-model tests remain meaningful,
- admin-new smokes still cover dashboard, projects, contexts, egress, files,
  workflows, sessions, recordings, and workflow runs.

### 10. Route-Backed Workflow Run Detail

Tier: P2 admin-new parity.

Status: implemented on `feature/BPANE-00154`; pending merge review.

Why here:

- The previous `/admin-new/runs` surface had only an overview.
- Operators need logs, events, produced files, run controls, errors, and
  session links before replacing old admin workflows.
- This is a contained route slice after helper consolidation.

Scope:

- add canonical `/admin-new/workflow-runs/[run_id]` and retain
  `/admin-new/runs/[run_id]` as a compatibility alias,
- show run metadata, state, timestamps, project/session/workflow references,
  logs, events, outputs, produced files, and errors,
- add safe controls for cancel, resume, reject, and input submit where the API
  supports them,
- link to related session detail/preview.

Validation:

- workflow-run client/view-model/component tests,
- admin-new workflow-run smoke opens detail and verifies key sections,
- old admin workflow-run behavior remains smoke-covered where available.

### 11. Route-Backed Session Subareas, Phase 1

Tier: P2 admin-new parity.

Status: implemented and validated on `feature/BPANE-00155`; pending merge of
PR #197.

Why here:

- Session detail is currently too broad.
- Operators need the browser runtime and files/recordings/network facts
  separated before the new app can be ergonomic.

Scope:

- add or formalize session live/preview routing,
- add session files and file-binding route,
- add session recordings route with segment/playback/export visibility,
- add session network route with effective network identity, egress profile,
  diagnostics, and active probe evidence.

Validation:

- session subroute component tests,
- admin-new sessions smoke covers subroute navigation,
- old admin session/files/recording/network smokes stay green where available.

### 12. Route-Backed Session Subareas, Phase 2

Tier: P2 admin-new parity.

Status: implemented and validated on `feature/BPANE-00156`, pending review.

Why here:

- Automation, policy, and observability are distinct operator tasks.
- They should not remain hidden in one mixed session detail view.

Scope:

- add automation route for MCP delegation, automation owner/delegate state, and
  workflow associations,
- add policy route for capabilities, project restrictions, browser policy
  evidence, and effective session-template facts,
- add observability route for metrics, local event timeline, logs, snapshots,
  recording/workflow indicators, and admin event-stream state.

Validation:

- route-level tests for automation/policy/observability,
- MCP delegation smoke,
- metrics/logs interaction smoke,
- stale selected-session state does not leak across routes.

### 13. Identity And Access Review Route

Tier: P2 admin-new enterprise parity.

Status: implemented for #157 scope on 2026-08-07. The route includes the
sanitized review plus contained service-principal and identity-mapping
lifecycle management; enforced RBAC remains #176.

Why here:

- Backend identity/access-review and service-principal APIs already exist.
- Navigation advertises identity.
- It is the first visible enterprise control-plane route for the unified app.

Scope:

- add `/admin-new/identity`,
- show current principal and project access review,
- show service-principal catalog and detail actions where supported,
- show identity mappings and unmapped safe principal signals,
- keep raw token claims and secrets out of the UI.

Validation:

- identity client/view-model/component tests,
- access-review smoke,
- service-principal and identity-mapping API tests remain green.

### 14. API Companion, Coverage, And Docs Routes

Tier: P2 completion.

Status: implemented for #158 scope on 2026-08-07. The three route-backed
surfaces consume checked contract artifacts, provide placeholder-only task
commands, expose the full generated inventory, and separate non-v1
compatibility surfaces. Promotion of admin-new remains #163.

Why here:

- Navigation advertises API, coverage, and docs.
- OpenAPI operation classification exists in docs but not in the app.
- These routes help operators and integrators understand owner, worker,
  evidence, and compatibility surfaces.

Scope:

- add `/admin-new/api`,
- add `/admin-new/coverage`,
- add `/admin-new/docs`,
- expose copyable examples for high-value owner API flows,
- keep compatibility endpoints separated from frozen v1 API operations.

Validation:

- route/component tests,
- smoke that routes load and expose expected operation families,
- docs link checks.

### 15. Missing Resource Catalogs

Tier: P2/P3 admin-new resource parity.

Why here:

- The main resource foundation exists, but several backend-backed resources
  still have no dedicated admin-new management surface.
- These should follow the shared resource catalog pattern.

Scope:

- session-template catalog/create/edit/version visibility,
- approved extensions catalog,
- credential bindings catalog,
- workflow event subscriptions catalog,
- operation counters or resource evidence where available.

Validation:

- one resource family per focused PR when possible,
- catalog/detail/create/edit component tests,
- API payload examples and field validation,
- admin-new smoke for each route added.

### 16. Browser Context Clone, Import, Export UI Parity

Tier: P2/P3 resource lifecycle.

Status: Implemented and hosted-validated on `feature/BPANE-00160` under PR
`#203`; merge remains.

Why here:

- Context catalog/create/detail/edit existed, but clone/import/export were
  lifecycle gaps.
- Import safety landed through `#148` before archive upload was exposed in
  admin-new.

Scope:

- clone inactive reusable contexts,
- export inactive reusable contexts,
- import safe BrowserPane context archives,
- show active-writer and storage-limit blockers clearly.

Validation:

- UI tests for clone/export/import forms and blockers,
- browser-context API tests,
- admin-new context smoke extended to cover lifecycle actions.

### 17. Project Governance Evidence And Cross-Resource Policy UX

Tier: P2/P3 governance.

Why here:

- Projects are implemented, but richer evidence makes quotas and policy
  decisions understandable.
- This work depends on stable resource catalogs and session subareas.

Scope:

- surface workflow-run quota/queue state,
- show retained storage and artifact quota evidence as models mature,
- show policy allowlist effects for templates, contexts, egress profiles,
  extensions, and file workspaces,
- show upload/download/session-file/recording policy effects close to affected
  controls,
- preserve the egress usage privacy boundary.

Validation:

- project governance component tests,
- API tests for quota/policy validation errors,
- manual smoke that blocked choices explain the project policy reason.

### 18. Operator CLI Resource Parity And Local Setup Docs

Tier: P3 operator completeness.

Why here:

- CLI is the automation-safe operator surface.
- Some resource families already have APIs but incomplete CLI/admin coverage.
- Local setup diagnostics must stay aligned with runtime behavior.

Scope:

- add or polish CLI commands for resource families where API support exists,
- keep profile precedence and token file permissions strict,
- preserve MCP doctor/preflight/repair semantics,
- document local workflow source, MCP, certificate, Docker socket, and camera
  diagnostics.

Validation:

- CLI unit tests,
- `smoke:bpane-cli`,
- MCP preflight/doctor smokes,
- local setup documentation link/path checks.

### 19. Admin-New Promotion Gate (Completed)

Tier: completed release decision retained for traceability.

Why here:

- The executable promotion contract merged through PR #210.
- PR #211 made `/admin-new/` the default while preserving `/admin/` as a
  compatibility fallback.

Scope:

- preserve the route and validation contract,
- keep route-by-route regression checkpoints current,
- make compatibility-admin removal a separate explicit decision.

Validation:

- mandatory Admin-New smokes,
- selected compatibility-admin regression smokes,
- root redirect and direct `/admin/` fallback checks.

### 20. Admin And Session Catalog Scalability

Tier: P3 scalability.

Why after store tests:

- It changes query behavior and aggregate semantics.
- Contract tests reduce the risk of breaking control-plane shape.
- It is needed before large demos or production-scale admin usage.

Scope:

- push session pagination/filtering into SQL,
- add targeted counts and `ANY($ids)` batching,
- add workflow-run `session_id` and state-aware indexes,
- replace queued-position scans with targeted queries,
- batch identity/access-review and admin-event snapshots,
- document Postgres pool sizing.

Validation:

- seeded 1k-session/10k-run API test or perf smoke,
- payload-shape regression tests,
- admin-new dashboard/session/identity smoke.

### 21. Worker And Archive Runtime Hygiene

Tier: P3 reliability.

Why here:

- It is reliability cleanup with clear boundaries.
- It complements workflow and recording hardening without broad redesign.
- It reduces async-runtime and worker failure modes before larger workflow UI
  expansion.

Scope:

- cap workflow/recording worker stdout and stderr accumulation,
- add request timeouts to worker polling/fetches,
- prevent overlapping supervisor polls,
- move CPU-heavy ZIP/export work to blocking tasks,
- remove avoidable `Body::from(bytes.clone())` clones where safe.

Validation:

- worker unit/integration tests for timeout and bounded logs,
- archive/export tests for large artifacts,
- workflow and recording smokes.

### 22. Documentation Accuracy And Production References

Tier: P3.

Why here:

- ARCH.md currently misstates important hot-path behavior.
- Config, threat-model, deployment, backup, and API guide material are needed
  before a production-readiness claim.
- This work should track code changes rather than lag behind them.

Scope:

- fix capture/classify/cache claims in ARCH.md,
- add both admin apps to durable architecture maps where still missing,
- remove stale hardcoded LOC/file-size claims,
- add gateway config reference for important flags,
- add security/threat-model and local-dev caveat docs,
- split or point README sections where duplication is highest,
- enrich OpenAPI examples for high-use operations.

Validation:

- link/path checks,
- compare key runtime claims against manifests/code,
- no stale `doc_updated` or deleted-plan references.

### 23. Docker Runtime Launch Boundary

Tier: P1 production hardening, implemented through #167 and #214.

Why here:

- A direct gateway socket mount is not a safe production boundary.
- The generic proxy selected by #167 reduces API exposure but cannot validate
  allowed container/volume request bodies or enforce resource ownership.

Implemented #167 direct compatibility scope:

- remove the direct gateway socket mount,
- add a digest-pinned internal Docker API proxy with explicit API allowlists,
- keep local session, workflow, recording, context, and file-helper behavior,
- add structured manifest and live API denial checks,
- update production topology docs without claiming full authorization.

Implemented #214 production-like Docker-host scope:

- add an audience-authenticated, policy-validating launch broker and shared
  typed adapter contract,
- route browser, workflow, recording, browser-context, and session-data
  operations through broker-owned adapters,
- validate or derive owned images, names, networks, mounts, privileges,
  resource limits, and lifecycle operations,
- remove the gateway's Docker endpoint, proxy dependency, socket, and
  Docker-control network membership in the production-like topology,
- retain base `docker_pool` as an explicit local direct compatibility path.

Validation:

- local compose runtime-launch smoke,
- denied Docker API operations through the proxy,
- static and live broker-isolation checks,
- browser, worker, storage, restart, MCP, workflow, recording, and compose API
  parity on the isolated topology,
- production docs no longer present raw socket or the direct proxy as a
  production authorization boundary.

### 24. Host And Client Render Hot-Path Work

Tier: P3/P4, profile-guided.

Why not earlier:

- It may produce large diffs in sensitive media paths.
- The review is confident about waste, but magnitude should be measured before
  broad changes beyond mechanical fixes.
- It is not the top blocker for admin-new promotion.

Scope:

- measure host capture/classify under idle, cursor-only, and video scenarios,
- reuse frame buffers,
- hash once per tile per frame,
- scope classify work to damage where feasible,
- cache-check before tile decode,
- evaluate worker decode and `ImageBitmap` cache path,
- fix ARCH.md alongside behavior changes.

Validation:

- perf trace before and after,
- host tests for tile/classification behavior,
- browser render smoke with video/tile updates,
- regression check for reconnect/full-refresh behavior.

### 25. Gateway Fan-Out And Transport Optimization

Tier: P4.

Why here:

- It is important for multi-viewer scalability but should be profiled under a
  realistic viewer count first.
- It touches streaming behavior and should not be mixed with token/auth
  cleanup.

Scope:

- profile per-viewer frame/keyframe encode and large memcpy,
- share pre-encoded bytes where safe,
- examine send-stream lock/backpressure behavior,
- evaluate late-join full-refresh broadcast behavior.

Validation:

- 1/5/10-viewer allocation and CPU profile,
- multi-session/multiviewer smoke,
- late-join bootstrap regression tests.

### 26. Structural Refactors

Tier: P4.

Why last:

- These are valuable but have the highest churn.
- They should be done after tests, CI, and contract coverage are stronger.
- They should follow touched-domain boundaries, not happen as one broad PR.

Scope:

- split `SessionStore` into per-domain repository traits,
- introduce domain ID newtypes at high-risk boundaries,
- standardize error strategy and stable error codes by subsystem,
- replace long same-typed positional parameter lists with context structs,
- reduce crate-wide `use super::*` seams when touching modules,
- split top monolith files by stable subdomain.

Validation:

- no behavior changes without tests,
- contract suite stays green,
- targeted smoke for each touched subsystem,
- no broad formatting-only churn.

### 27. Deferred Product And Enterprise Backlog

Tier: P4-plus.

Why deferred:

- These topics are valid but should not block the next implementation slices.
- Several need product and deployment decisions beyond cleanup/refactoring.

Deferred items:

- organization/project RBAC and service-principal grant enforcement under
  focused issue `#176`,
- provisioning/deprovisioning and break-glass lifecycle under `#177`,
- immutable audit/event export,
- BrowserPane-issued API keys and PATs,
- global abuse/rate limiting beyond current project quotas,
- billing and usage metering productization,
- HA/DR, backup/restore drills, BYOK, and data residency,
- stealth/fingerprint/CAPTCHA/mobile automation,
- Python SDK and broader language SDK expansion,
- deleting old `/admin/` without a separate compatibility-removal decision and
  regression gate.

The following cross-cutting gaps have focused owners:

- supported immutable Playwright package: `#47`,
- Phase 0 polling Workflow Endpoint: `#172`,
- Phase 0 BPM browser-activity qualification and delivery: `#174`,
- remote protocol specification and conformance: `#175`,
- platform telemetry, SLOs, and capacity evidence: `#178`,
- OpenAPI conformance and compatibility governance: `#179`,
- open-source license, contribution, and IP governance: `#180`.

#### Current Pilot Value Sequence

1. `#47` freezes Playwright TypeScript, Git commit-pinned source, entrypoint,
   schemas, resource bindings, credential behavior, and regression evidence as
   one supported workflow package.
2. `#172` is complete through PR #250 and exposes that package as one
   project-scoped, service-principal-authorized asynchronous BPM activity using
   polling, schema enforcement, idempotency, typed outcomes, timeout/cancel,
   side-effect certainty, and bounded results/artifacts.
3. `#174` selects and operates one real process under a bounded agreement and
   records Stop, bounded Operate, or Phase 1. It remains externally deferred
   until the real activity, accountable owners, deployment, and data/threat
   profile are selected; automation must not infer them.
4. `#180` is explicitly eligible in parallel and is the next requirements
   candidate while `#174` waits. It must complete before an external Pilot
   relies on BrowserPane's open-source posture; engineering consistency and
   legal approval must remain separately stated.
5. While both `#174` and `#180` wait for external decisions, `#175` is the first
   engineering fallback. ADR 0003 fixes its product direction, so a focused
   protocol specification/conformance plan may be prepared from code and
   repository evidence.
6. If `#175` is blocked, exhausted, or complete, `#124` is the final current
   fallback for the bounded Admin-New session-template catalog. No other issue
   is implicitly eligible.

BrowserPane subprocesses, Human Handoff, Teach Mode, workflow training or
generation, and automatic repair are not Phase 0 requirements. Challenges end
as `external_intervention_required` and the external process owns human tasks.

The executable plans are `BPANE-00047_WORKFLOW_PACKAGE_CONTRACT_PLAN.md`,
`BPANE-00172_PHASE_0_WORKFLOW_ENDPOINT_PLAN.md`, and
`BPANE-00174_PHASE_0_REFERENCE_WORKFLOW_PLAN.md`.

#### Deferred Workflow Endpoint Productization

Issue `#240` owns immutable endpoint revisions, promotion/rollback, callbacks,
replay, trace expansion, throttling/readiness, and connector compatibility.
Its specification is `BPANE-00240_WORKFLOW_ENDPOINT_PRODUCTIZATION_PLAN.md`.
The older `BPANE-00172_BPM_WORKFLOW_ENDPOINT_INTEGRATION_PLAN.md` is retained
only as historical combined design context.

#### Focused Phase N Slice: Workflow Teach Mode

Issue `#171` promotes one concrete Phase N capability out of this broad
backlog without changing the immediate work order.

Business outcome:

- combine prose process intent and semantic human demonstrations,
- generate a reviewable workflow candidate,
- validate it in a fresh Browser Context,
- publish only an approved immutable workflow version,
- turn later Vendor Drift corrections into candidate patches rather than
  autonomous production mutation.

Dependencies and boundaries:

- `#20` provides reusable page/tab/session inspection primitives,
- `#21` provides generalized artifact/evidence resources,
- `#47` owns workflow packaging, publishing, and execution,
- `#172` owns stable external Workflow Endpoint deployment and invocation,
- `#71` owns Human Handoff/intervention semantics,
- `#174` provides a bounded real process from which Teach Mode demand and
  evidence may be derived,
- the Foundation and stable workflow publishing/endpoint contracts remain
  ahead of Teach Mode by default.

The step-by-step plan and smoke sequence are in
`BPANE-00171_WORKFLOW_TEACH_MODE_PLAN.md`.

## Topic-To-Order Map

| Topic family | Priority owner in this file |
| --- | --- |
| Security cleanup from review | Items 1-6, 21, 23 |
| Admin-new promotion blockers | Items 2, 9-19 |
| Session domain requirements | Items 10-12, 17, 19 |
| Workflow domain requirements | Items 3, 10, 15, 21, 27 (`#47` package, `#172` Phase 0 endpoint, `#240` productization, then `#171` Teach Mode) |
| Recording requirements | Items 5, 11, 21 |
| Browser context lifecycle | Items 4, 16, 22 |
| Network identity and egress | Items 3, 11, 17, 23 |
| File workspaces and session files | Items 11, 15, 17 |
| Project governance | Items 13, 17, 20 |
| Identity and access | Items 2, 13, 20, 27 |
| Operator CLI and local setup | Items 6, 18, 22 |
| Admin feedback, metrics, and logs | Items 9, 12, 19 |
| Validation matrix | Applies to every item; choose the narrow subset plus impacted smokes. |
| Documentation cleanup | Items 18, 22, 23, plus per-slice docs updates |
| Performance and maintainability refactors | Items 20-26 |

## Suggested Immediate Next Issue

Use focused issue `#151` as the first product implementation slice after the
`#173` governance update:

Title: `Add minimal CI, dependency safety, and validation ratchet`

Business case:

- BrowserPane already has meaningful tests and smokes, but they are not enforced
  on pull requests.
- Main branch protection has no required status checks.
- Open critical/high dependency advisories include runtime transport/TLS and
  MCP HTTP/routing packages as well as development tooling.
- Auth and security changes should be judged on a trusted repeatable baseline.

Acceptance criteria:

- critical/high findings are remediated or have reviewed bounded exceptions,
- Rust, Node, admin-new, worker, API-contract, and docs checks run in CI,
- coverage baselines and regression rules are recorded,
- controlled failures prove each major CI stage fails visibly,
- the required checks are configured in branch protection,
- a local wrapper mirrors the CI commands.

Smoke sequence:

1. follow the Ready plan in `BPANE-00151_MINIMAL_CI_VALIDATION_PLAN.md`,
2. run the local CI wrapper on a clean checkout,
3. verify every required stage executes and reports failures clearly,
4. exercise controlled Rust, Node, contract, docs, and dependency failures,
5. restore the fixtures and confirm the complete required path is green,
6. verify branch protection requires the resulting checks,
7. record coverage and any time-bounded dependency exception evidence.
