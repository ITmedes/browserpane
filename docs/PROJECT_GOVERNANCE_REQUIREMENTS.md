# Project Governance Requirements

This document preserves the detailed project-governance requirements that
matter to `/admin-new`, API clients, CLI behavior, and future enterprise
control-plane work.

## Business Purpose

Projects are the governance boundary for BrowserPane sessions, workflow runs,
durable files, recordings, egress usage, policy allowlists, and delegated
automation. Operators must be able to answer:

- which project owns this session, workflow run, file, recording, context, or
  credential reference,
- whether admission was allowed, queued, rejected, or warning-only,
- which quota, policy binding, or usage budget caused the decision,
- how much runtime, retained storage, session creation volume, and sanitized
  egress byte usage the project has consumed.

## Current Implemented Shape

The current control plane supports:

- project metadata and state,
- active session quotas,
- active workflow-run quotas,
- retained-storage quotas,
- project-scoped session resources,
- project-scoped workflow runs,
- project summaries on session and workflow resources,
- project admission decisions and stable reason codes,
- queued project-scoped sessions,
- queued project-scoped workflow runs,
- warning-first usage budget alerts,
- opt-in blocking of new project sessions when configured budgets are
  exhausted,
- rolling session-creation rate limits,
- runtime-usage budget blocking for new sessions,
- sanitized egress byte usage ingestion and project rollup,
- project-scoped egress profiles,
- project-scoped credential bindings,
- project-scoped browser contexts and file workspaces,
- allowlists for templates, egress profiles, extensions, browser contexts, and
  file workspaces,
- project policy controls for live uploads, live downloads, session-file
  bindings, and manual recording starts.

## Admission And Queueing

### Workflow Runs

Workflow-run create can carry `project_id` directly or inherit the project from
an explicitly bound or newly created session. Project-scoped runs must:

- persist and expose project id and compact project summary,
- enforce `max_active_workflow_runs`,
- reuse queued workflow-run semantics where delayed dispatch is valid,
- reject missing or archived projects before duplicate work is created,
- keep idempotent retries stable by returning the original run and admission
  state for the same request id,
- expose stable project-admission reason codes.

### Browser Sessions

Project-scoped browser sessions can persist as visible `queued` resources when
`max_active_sessions` is exhausted. Queued sessions must:

- carry `active_session_quota_exceeded` style admission metadata,
- show queue position, queue age, active/queued count snapshot, dispatch
  blocker, and cancellable state,
- contribute to project `queued_sessions` usage,
- reject browser connect-ticket minting until admitted,
- support explicit queued-session cancel,
- promote the oldest eligible queued session to `ready` when capacity opens
  through stop, release, idle stop, or reconnect preparation.

## Usage Budgets

Projects can expose warning-first alerts and opt-in blocking through policy.

Required usage metrics:

- session creation count,
- live plus finalized browser runtime milliseconds,
- sanitized egress receive/transmit byte counters,
- retained storage bytes.

Required budget behavior:

- existing projects default to `warning_only`,
- alerts emit at meaningful thresholds such as 80% and 100% where configured,
- `usage_budget_enforcement=block_session_creation` can reject new project
  sessions for exhausted session creation budgets,
- rolling limits use `max_session_creations_per_window` together with
  `session_creation_window_sec`,
- rolling limit configuration is invalid unless both values are positive and
  configured together,
- exhausted rolling budgets reject new sessions with
  `session_creation_rate_exceeded`,
- exhausted runtime budgets reject new sessions with
  `runtime_usage_budget_exceeded`,
- egress-byte budgets remain advisory unless proxy-side ingestion is
  authoritative enough for blocking,
- budget blocking must not stop or kill already-running sessions.

## Retained Storage

Retained-storage usage counts project-owned or project-linked retained bytes
that can be attributed without ambiguous ownership:

- workflow produced files from project-scoped workflow runs,
- completed recording artifacts from sessions that belong to the project,
- uploaded/downloaded session files from sessions that belong to the project,
- files retained in project-owned file workspaces.

Workflow produced files stored in a file workspace owned by the same project
are counted once through the workspace. Reusable browser-context profile
storage remains governed by per-context limits rather than project retained
storage.

Quota enforcement must:

- reject retained artifacts that would exceed the project quota,
- clean up rejected stored content and metadata,
- preserve stable quota error codes,
- keep successful retained artifacts downloadable and listed normally.

## Policy Bindings

Empty allowlists mean unrestricted for that policy dimension. Non-empty
allowlists are enforced after template defaults and workflow/session effective
configuration are resolved.

Required project policy dimensions:

- `allowed_session_template_ids`,
- `allowed_egress_profile_ids`,
- `allowed_extension_ids`,
- `allowed_browser_context_ids`,
- `allowed_file_workspace_ids`,
- live browser upload enable/disable,
- live browser download enable/disable,
- session-file binding enable/disable,
- manual recording start enable/disable.

Policy rejects should be visible through API, CLI, and admin surfaces with
stable, actionable reason codes such as:

- template not allowed,
- egress profile not allowed,
- extension not allowed,
- browser context not allowed,
- file workspace not allowed,
- session file binding not allowed,
- manual recording not allowed,
- project scope conflict for egress profile or credential binding.

Fresh or ephemeral browser contexts remain allowed when the reusable context
allowlist is configured, unless a later policy explicitly forbids them.

## Credential, Egress, And File Boundaries

Credential bindings can be owner-scoped or project-scoped. Project-scoped
workflow runs and project-scoped sessions must not consume credential bindings
from a different project.

Egress profiles can be owner-scoped or project-scoped. A project-scoped session
must not consume another project's egress profile. If an egress profile uses a
project-scoped proxy credential binding, that binding must belong to the same
project as the profile.

File workspaces can be owner-scoped or project-scoped. Project policy can
allowlist workspace ids for workflow inputs and session-file binding sources.
The same project policy reason should apply to workflow workspace inputs and
session file bindings.

## Egress Usage Privacy Boundary

The egress usage API is an ingestion point for sanitized counters, not a proxy
log store.

BrowserPane may store:

- session/profile/container correlation metadata,
- observer id,
- source kind,
- timestamps,
- receive/transmit byte deltas.

BrowserPane must not ingest:

- requested URLs,
- response status,
- timing traces,
- headers,
- payloads,
- credentials,
- raw CA material,
- decrypted traffic.

The proxy or secure web gateway owns detailed outbound access logs.

## Admin-New Requirements

The unified admin app must expose project governance through:

- project catalog and project detail/edit,
- quota and policy configuration,
- generated usage alerts,
- active and queued session evidence,
- workflow-run project/admission evidence,
- create-session project selector summaries,
- resource selectors for templates, contexts, egress profiles, extensions, and
  file workspaces,
- retained-storage usage and quota evidence,
- egress usage evidence without sensitive traffic details,
- clear validation for disallowed project policy combinations.

Follow-up admin-new gaps:

- richer project workflow-run quota and queue evidence,
- route-backed session policy tab showing effective project policy,
- identity route showing project access-review and delegated principals,
- retained artifact quota UX once the artifact model is hardened,
- audit/event surfaces once generalized events exist.

## Validation And Smoke Requirements

Project-governance changes should include the relevant subset of:

- `cargo test -p bpane-gateway project`
- `cargo test -p bpane-gateway workflow_run`
- `cargo test -p bpane-gateway project_policy`
- `cargo test -p bpane-gateway session_project_policy`
- `cargo test -p bpane-gateway retained_storage`
- `cargo test -p bpane-gateway project_usage`
- `cargo test -p bpane-gateway project_egress`
- `cargo test -p bpane-gateway credential_binding_project_scope`
- `cargo test -p bpane-gateway file_workspace_project_policy`
- `cargo test -p bpane-gateway session_files`
- `cargo test -p bpane-gateway recordings`
- compose API suites for projects, workflow-run controls, and affected
  sessions when API behavior changes
- `cd code/web/bpane-client && npm run smoke:workflow-admission -- --headless`
- `cd code/web/bpane-client && npm run smoke:workflow-queued-cancel -- --headless`
- `cd code/web/bpane-client && npm run smoke:workflow-workspace -- --headless`
- `cd code/web/bpane-client && npm run smoke:workflow-credentials -- --headless`
- `cd code/web/bpane-client && npm run smoke:bpane-cli -- --headless`

## Manual Project Governance Smoke

1. Create a project with `max_active_sessions=1`.
2. Create the first project session and confirm it is `ready`.
3. Create a second project session and confirm it is `queued` with queue
   position, blocker, age, active/queued count, and cancellable state.
4. Attempt to mint a connect ticket for the queued session and confirm the API
   returns a conflict.
5. Cancel one queued session and confirm it becomes stopped.
6. Stop or release the active session and confirm the next eligible queued
   session promotes to `ready`.
7. Create a project with `max_active_workflow_runs=1`.
8. Start two workflow runs in that project and confirm the second remains
   visible as queued with a stable project quota reason.
9. Configure a blocking session creation budget and confirm the next project
   session fails with a stable budget reason.
10. Configure a rolling session creation budget and confirm the excess session
    fails with `session_creation_rate_exceeded`.
11. Configure a runtime budget and confirm only new project sessions are blocked
    after exhaustion.
12. Configure allowed template, egress profile, extension, browser context, and
    file workspace policies and verify allowed resources pass and disallowed
    resources fail before runtime launch.
13. Disable file transfer, session-file bindings, and manual recording for a
    project and confirm the affected session capabilities/actions are blocked.
14. Report sanitized egress byte deltas and confirm project usage rolls them up
    without exposing proxy logs.
15. Add retained files, recordings, and workflow produced files and confirm
    retained-storage usage and quota behavior.

## Deferred Enterprise Work

Project governance should not absorb these broader platform projects directly:

- central policy engine,
- immutable audit log and API-key lifecycle,
- generalized resource/security event subscriptions,
- DLP/content inspection and quarantine,
- unified artifact/output model,
- hard runtime-minute stops,
- proxy/egress byte blocking,
- active/active multi-gateway quota reconciliation.
