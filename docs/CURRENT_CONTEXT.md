# BrowserPane Current Context

Last synchronized: 2026-08-21 for the #172 review candidate

This is the first local document to read when starting a clean BrowserPane
session. It records current product decisions, delivery order, and repository
state. Detailed requirements remain in the linked domain and plan documents.

## Current Baseline

- Canonical base: `main` at `ab9dc564`; #172 is implemented on its review
  branch and is not merged evidence yet.
- `/admin-new/` is the default operator console. `/admin/` is a compatibility
  fallback pending a separate removal decision.
- The gateway uses the typed runtime launch broker for the production-like
  Docker-host boundary. Direct Docker proxy access is local compatibility only.
- A bounded single-node Compose deployment profile, threat-model baseline,
  OpenTelemetry runtime trace checkpoint, workflow/recording metrics,
  Prometheus starter alerts/runbook, and Grafana operations dashboard are
  merged through PRs #224, #226, #228, #230, #232, and #234.
- Compose CI reliability was restored through PR #236. The post-merge main run
  passed gateway default, gateway docker-pool, browser/integration, unified
  admin, and compatibility admin jobs.
- The supported immutable Playwright workflow package contract is frozen
  through #47/PR #242, including publication compatibility, Admin-New/CLI
  visibility, worker evidence redaction, and live package regression evidence.
- The #172 review candidate implements the bounded project Workflow Endpoint
  across the gateway, Postgres, Admin-New, CLI, and OpenAPI. Its real local
  Keycloak/Postgres/workflow-worker/browser fake-BPM smoke passes, including
  idempotency, validation, cancellation, timeout, intervention, typed failure,
  uncertain-side-effect, and authorization-denial paths.
- The public owner-scoped v1 API remains frozen in
  `openapi/bpane-control-v1.yaml`. New BPM endpoint work must extend it through
  the compatibility policy rather than bypass it.

## Product Boundary

BrowserPane is a governed browser execution and remote-session platform. It is
not a BPMN engine, system of record, business scheduler, compensation engine,
or autonomous business decision system.

Use this integration order:

1. native target-system API,
2. established connector or integration,
3. BrowserPane for the remaining browser-only activity.

The external BPM owns process state, surrounding subprocesses, broad retry,
compensation, and human tasks. BrowserPane owns the approved browser workflow,
its run, normally one browser session, and agreed result/evidence.

## Frozen Phase 0 Direction

Phase 0 proves one reusable browser activity in one bounded environment:

- one externally invoked asynchronous BPM activity,
- one approved immutable Git-backed Playwright TypeScript workflow version,
- one workflow run and normally one isolated browser session per invocation,
- OIDC client-credentials authentication through an enforced service-principal
  project/endpoint grant,
- JSON Schema Draft 2020-12 input and output validation,
- endpoint/caller-scoped idempotency with payload conflict detection,
- polling-based invoke, status, and cancel operations,
- typed terminal outcomes and explicit browser side-effect certainty,
- bounded JSON results plus authorized artifact references,
- Admin-New, CLI, OpenAPI, runbook, and deterministic conformance evidence.

Explicit Phase 0 non-goals:

- BrowserPane-managed subprocesses,
- BrowserPane-managed Human Handoff,
- Teach Mode, semantic demonstrations, workflow generation, or model training,
- automatic workflow repair or publication,
- callbacks, connector-specific adapters, HA, broad multi-tenancy, or generic
  production claims.

A portal challenge, MFA, CAPTCHA, consent prompt, or required judgment ends the
BrowserPane run with `external_intervention_required`. The external BPM decides
whether and how to create a human task. An ambiguous failure after a mutating
browser action must expose `side_effect_state=uncertain`; the BPM must not
blindly retry the complete activity.

## Immediate Issue Sequence And Parallel Gate

1. `#47` - complete through PR #242: supported immutable Playwright workflow
   package and publishing contract.
2. `#172` - review and merge the Phase 0 project-scoped polling Workflow
   Endpoint candidate.
3. `#174` - after #172 merges and its remaining gates pass: select, deliver, and operate one
   real bounded BPM browser activity.

In parallel, `#180` resolves AGPL/Cargo/Node metadata and
contribution-governance inconsistency before an external Pilot relies on the
open-source posture.

`#240` owns later endpoint revisions, promotion/rollback, callbacks, replay,
trace expansion, throttling, and connector compatibility. `#71` Human Handoff
and `#171` Teach Mode remain valid later capabilities but are not Phase 0
dependencies.

Conditional Phase 0 owners are `#21` for artifact gaps, `#66` for deployment
beyond bounded single-node Compose, and `#20`, `#72`, `#73`, or `#178` only when
the selected process requires more inspection, security, recovery, or telemetry
than the current baseline provides.

## Beyond Phase 0

The later phases remain valid, but they are evidence gates rather than a
committed calendar or one continuous implementation branch:

- **Phase 1 Operate:** make the proven activity repeatable for an agreed scope.
  This includes the required Admin-New operator paths, incident/change
  procedures, upgrades, rollback, backup, observed telemetry/SLO proposals,
  support ownership, and explicit disposition of Phase 0 gaps.
- **Production Baseline:** qualify named deployment profiles and limits with a
  reviewed threat model, release signing/provenance, compatibility policy,
  tested upgrades/rollback/backup/DR, SLOs, capacity, audit/retention, and
  consistent open-source governance. The current single-node Compose profile is
  Prototype evidence, not a generic Production claim.
- **Phase N Scale:** generalize proven capabilities across organizations,
  projects, callers, and deployment environments. Candidate owners include
  `#176` authorization, `#177` identity lifecycle, `#76` residency/BYOK, `#79`
  policy, `#80` DLP, `#74` HA, and `#66` Kubernetes/Fargate/cloud adapters.
- **Workflow Endpoint productization:** `#240` may add immutable endpoint
  revisions, promotion/rollback, callbacks, replay, tracing, throttling, and
  connector compatibility after the bounded `#172` contract succeeds.
- **Optional innovation:** `#171` Teach Mode/controlled repair and `#71` Human
  Handoff remain deferred capabilities. They require separate evidence and
  never imply autonomous publication or high-impact decision making.

BrowserPane subprocess orchestration is not currently planned for a later
phase. Scheduling, surrounding subprocesses, broad retries, compensation, and
human-task orchestration remain responsibilities of the external BPM unless a
future explicit product decision changes the boundary.

Use `PRODUCT_PHASES_AND_RELEASE_GATES.md` for exact gate evidence and
`DELIVERY_ROADMAP.md` for current issue order. Investor phase descriptions in
`../pane-invest` must remain consistent with these engineering gates.

## Current Material Gaps

The current code has immutable workflow versions, runs, session bindings,
owner-scoped idempotency, logs/events/files, OIDC validation, service-principal
registry metadata, and Admin-New run detail. It does not yet have:

- a stable project-scoped Workflow Endpoint resource/key,
- enforced endpoint invocation/read/cancel grants for service principals,
- runtime enforcement of workflow input and output schemas,
- endpoint/caller-scoped idempotency with request fingerprinting,
- a stable machine-readable terminal outcome and side-effect certainty model,
- the dedicated external polling API and fake-BPM conformance fixture.

## Working Tree Guardrail

At the time of this synchronization, local `main` matches `origin/main`. The
generated files below are locally modified and must not be staged or reverted
unless the user explicitly requests it:

- `dev/certs/cert-fingerprint.txt`
- `dev/certs/cert-hash.txt`

Do not assume a local Compose stack is still running in a future session.
Inspect it before testing.

Investor and management presentation artifacts are maintained in the sibling
`../pane-invest` repository. Product claims there must follow this repository's
capability and gate evidence; do not treat presentation targets as implemented
code.

## Durable Working Agreements

- Commit every coherent, validated change instead of accumulating an unrelated
  working-tree batch.
- Start implementation work from one canonical issue and a matching
  `docs/*_PLAN.md`; keep business case, acceptance criteria, use case, and smoke
  sequence synchronized.
- Prefer established libraries and current repository boundaries over custom
  infrastructure without a measured need.
- Validate affected behavior at unit, integration, smoke/E2E, and negative-path
  levels appropriate to its risk. Keep API, Admin-New, CLI, OpenAPI, README, and
  ARCH representations aligned.
- When a merge is requested, wait for required GitHub checks and preserve a
  clean, reviewable commit history.
- The optional `dev_loop/` tooling uses separate Codex qualification,
  requirements-specification, proposal, and repair sessions. It consumes Ready
  work first; with an empty Ready queue it may promote exactly one
  roadmap-prioritized Qualified issue after a bounded audit. When the direction
  is decided but its issue/plan contract has evidence-backed omissions, one
  documentation-only specification PR may be created instead. The issue stays
  Qualified and a later post-merge iteration must reassess it before product
  implementation. The loop never creates product issues or reprioritizes the
  roadmap. Start it only from clean, synchronized `main` with an approved
  project GitHub identity; automatic merging remains opt-in. Local work stops
  below `MIN_FREE_DISK_GB` (50 GiB by default) without deleting operator data.

## Fresh Session Checklist

1. Read `AGENTS.md`, this file, and the focused issue body.
2. Run `git status --short --branch` and preserve unrelated local changes.
3. Check the live GitHub issue state; GitHub owns labels and execution status.
4. Read `DELIVERY_ROADMAP.md` and the focused `docs/*_PLAN.md`.
5. Verify code and runtime manifests before trusting stale prose.
6. Before implementation, create or update a bounded plan using
   `PLAN_TEMPLATE.md` and ensure the issue, use case, acceptance criteria, and
   smoke sequence agree.
7. Keep README, ARCH, OpenAPI, Admin-New, CLI, tests, and issue state aligned
   with user-visible changes.

## Context Hierarchy

1. Code, runtime manifests, and executable contracts.
2. Live GitHub issue state.
3. This current-context handoff.
4. `DELIVERY_ROADMAP.md` and `PRODUCT_PHASES_AND_RELEASE_GATES.md`.
5. Focused plan and domain requirement documents.
6. Historical audit, legacy, and superseded specification documents.
