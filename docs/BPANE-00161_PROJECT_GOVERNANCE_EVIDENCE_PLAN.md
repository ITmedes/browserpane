# BPANE-00161 Project Governance Evidence Plan

## Metadata

- Issue: `#161`
- State: Ready for implementation
- Lane: Operator Product
- Target gate: admin-new project governance parity
- Depends on: admin-new shared patterns and session subareas merged through
  `#156`; browser-context lifecycle parity merged through PR `#203`
- Branch: `feature/BPANE-00161`
- Last verified: `main` at `9da017d6db75da8905e1b88c196206b7f7c1808b`,
  2026-08-10

## Business Outcome

An operator can understand how a project governs new and existing work without
reconstructing policy from several screens or waiting for a rejected API call.
Project detail shows current quota pressure, usage alerts, sanitized egress and
retained-storage counters, configured allowlists, and related session/workflow
admission state. Existing session and workflow actions show the same project
decision close to the affected control while the gateway remains authoritative
for final admission.

This turns project configuration from a passive form into operational evidence.
It does not create a browser-side policy engine: the UI derives previews from
the project resource, submits only valid known choices, and still renders the
gateway's stable reason code and message if state changes between render and
submission.

## Example Use Case

A project allows one active session, one active workflow run, one reusable
browser context, and one egress profile. Its retained-storage and runtime
budgets are approaching their limits. An operator opens project detail and sees
the active and queued work, current usage against each configured limit, the
allowed resource names, and direct links to related sessions and workflow runs.
When creating another session, the selected project explains that it is at
capacity and that the new session may queue; disallowed contexts and egress
profiles stay visible but cannot be selected and name the project policy reason.
If a workflow run queues, its detail route shows the authoritative project
admission reason, count/limit snapshot, queue timestamp, and project link.

## Current Implementation Evidence

- The gateway and frozen OpenAPI already expose project quotas, policy,
  sanitized usage, generated alerts, project admission decisions, session queue
  evidence, and workflow-run project admission.
- Admin-new project detail supports metadata, quota and policy editing, usage
  refresh, all five allowlist catalogs, and basic usage/alert rows.
- `/admin-new/sessions/{id}/policy` already separates configured project policy,
  effective session capabilities, selected resources, admission, and managed
  browser evidence. This slice must reuse and refine that model rather than
  build a replacement route.
- Session creation loads projects, templates, browser contexts, and egress
  profiles, but currently validates only resource existence/state. It does not
  explain selected-project allowlists, resource scope conflicts, quota pressure,
  or budget alerts before submission.
- Workflow-run detail renders a compact admission label, but not the admission
  message, count/limit snapshot, checked time, generic queue reason, queued time,
  or a direct project link.
- Workflow launch accepts a free-text project id and does not load project
  state, quotas, or alerts.
- The project detail route does not load related session/workflow-run resources,
  so operators cannot reconcile project counters with visible work from that
  screen.

## Scope

- Add a pure project-governance model for usage pressure, allowlist effects,
  operation policies, related work, and safe reason copy.
- Preserve a machine-readable project id on selector options so owner/project
  scope can be evaluated without parsing display strings.
- Extend project detail with a read-only governance evidence section:
  - active/queued sessions and active/queued workflow runs,
  - current values against configured active, creation, runtime, egress, and
    retained-storage limits,
  - generated alerts and budget enforcement mode,
  - resolved allowed-resource names and stale/missing references,
  - operation policy status for upload, download, session files, and manual
    recording,
  - sanitized egress RX/TX/total only,
  - direct links to related resource routes.
- Load related session and workflow-run catalogs independently so a partial
  failure does not hide project configuration or usage evidence.
- Make session creation project-aware before submission:
  - show selected project state, active/queued capacity, budget mode, and alerts,
  - reject archived/missing projects locally,
  - disable and explain template, reusable-context, and egress choices blocked
    by the selected project's allowlist or project scope,
  - keep the gateway response authoritative for races and unsupported cases.
- Replace workflow launch's free-text project id with a project-backed selector
  and compact governance summary where the project catalog is available.
- Expand workflow-run detail with authoritative project admission and queue
  evidence, including a project link and count/limit facts.
- Refine existing session policy evidence only where needed for consistent
  reason wording and project navigation.
- Extend unit, component/integration, Compose smoke, documentation, and manual
  evidence.

## Non-Goals

- No new gateway route, database field, queue algorithm, quota, policy
  dimension, OpenAPI operation, or protocol message.
- No independent client-side authorization or admission decision. Local
  guidance never overrides a gateway result.
- No central policy engine, organization RBAC, service-principal grant
  enforcement, API key lifecycle, or immutable audit log; those remain owned by
  `#79`, `#176`, `#70`, and related enterprise issues.
- No generalized artifact model, DLP/content inspection, malware scanning,
  storage-provider expansion, or hard runtime-minute termination.
- No requested URL, header, payload, credential, proxy log, CA material, or
  decrypted traffic display. Egress evidence remains byte counters and safe
  correlation metadata only.
- No new extension, file-workspace, or recording mutation flow solely for this
  project screen. Existing affected routes/actions receive policy evidence.
- No catalog pagination or server-side project filtering; scalability remains
  owned by `#164`.

## UX And Product Decisions

- **Evidence precedes configuration.** Project detail shows current governance
  status before the editable policy/quota form so operators can distinguish
  observed state from proposed changes.
- **One source model, several projections.** Pure project-governance helpers
  produce consistent limit, allowlist, and reason semantics for project detail,
  session create, workflow launch/detail, and session policy views.
- **Blocked resources remain discoverable.** Selectors retain disallowed
  resources as disabled options with a concise reason instead of silently
  removing them and making existing references impossible to understand.
- **Empty allowlists mean unrestricted.** The UI must never present an empty
  list as deny-all.
- **Project scope and allowlist are separate reasons.** A project-bound resource
  from another project is a scope conflict even if an id appears in stale local
  policy. Owner-scoped resources remain eligible when allowed by policy.
- **Quota pressure is not pre-admission.** Reaching active capacity may mean
  queueing rather than rejection. The UI describes likely behavior but only the
  returned `ProjectAdmissionDecision` states allowed, queued, or rejected.
- **Alerts are not denials.** Warning-only alerts remain visually distinct from
  blocking budget mode and retained-storage enforcement.
- **Partial evidence stays useful.** A failed related-session or workflow-run
  request renders an inline warning for that subsection while project metadata,
  policy, usage, and other successful evidence remain available.
- **No policy JSON dump.** Operators see named resources, status, reason, and
  links. Raw ids remain secondary evidence where names cannot be resolved.

## Contract And Data Impact

- Gateway/OpenAPI/database/protocol: N/A.
- Admin-new project selector option:
  - add `projectId: string | null` alongside the human-readable scope label.
- Admin-new project governance state:
  - project resource and independently loaded related sessions/workflow runs,
  - partial-error state per related collection,
  - pure derived policy/usage/admission rows.
- Session create request: unchanged.
- Workflow run create request: unchanged; project ids come from a validated
  catalog selection instead of unrestricted free text when options are loaded.
- Egress display: existing sanitized project usage counters only.

## Implementation Slices

1. **Governance model and selector identity**: preserve selector `projectId`,
   add pure limit/policy/scope/relation models, and cover unrestricted,
   restricted, stale-reference, archived, cross-project, warning, blocking, and
   queue-pressure cases. Commit boundary: shared governance semantics.
2. **Project operational evidence**: independently load related sessions and
   workflow runs; add responsive governance summary, policy impact, related
   work links, partial errors, usage refresh, and sanitized egress evidence.
   Commit boundary: project detail operations view.
3. **Policy-aware session creation**: add selected-project summary, resource
   option availability/reasons, pre-submit allowlist/scope validation, and
   authoritative API-race error handling. Commit boundary: session action UX.
4. **Workflow governance evidence**: replace free-text project entry with a
   catalog selector/summary and expand run detail admission/queue/project
   evidence. Commit boundary: workflow action and inspection UX.
5. **Cross-route consistency**: align project links/reason wording in the
   existing session policy route and affected file/recording surfaces without
   adding new mutation flows. Commit boundary: consistent policy projections.
6. **Battle test and handoff**: focused suites, admin-new coverage, extended
   project and workflow admission smokes, compatibility/CLI regressions,
   responsive checks, README/docs, issue, and PR evidence. Commit boundary:
   promotion evidence.

## Test Strategy

### Unit

- Usage pressure for bounded/unbounded, below-limit, at-limit, over-limit, and
  invalid/stale evidence.
- Empty versus restricted allowlists, resolved and missing ids, disabled
  resources, owner scope, matching project scope, and cross-project scope.
- Session create policy validation for templates, reusable contexts, egress
  profiles, archived projects, and state changes.
- Workflow project selection and admission/queue formatting for allowed,
  queued, rejected, partial count snapshots, and absent project scope.
- Egress projection proves only sanitized byte counters enter the view model.

### Component And Integration

- Project detail renders usage/limit evidence, policy impact, related sessions
  and runs, direct links, no-alert state, warning/blocking states, and partial
  collection failures without losing the edit form.
- Session create renders selected-project pressure and alerts, disables blocked
  options with reasons, permits unrestricted/allowed options, and does not POST
  an invalid policy selection.
- A server-side conflict after locally valid selection remains attached to the
  create action and does not clear the draft.
- Workflow launcher lists active projects, blocks archived projects, renders
  quota/alert context, and preserves owner-scoped launch.
- Workflow-run detail renders project link, project admission message,
  count/limit snapshot, queue reason/time, and partial/null fields safely.
- Session policy keeps existing capability and operation evidence stable while
  adding consistent project navigation/reasons.
- Desktop and 390 px layouts do not overflow or hide action feedback.

### Gateway And Contract Regression

- Focused project, admission, policy, retained-storage, egress usage, session
  queue, and workflow-run queue tests continue to pass.
- Frozen OpenAPI operation inventory and examples remain unchanged.
- No new sensitive egress fields enter frontend types, fixtures, or snapshots.

## Manual Test Sequence

1. Start Compose and sign into `/admin-new/projects` as `demo`.
2. Create an active project with one active session, one active workflow run,
   bounded runtime/retained-storage/egress usage, blocking session budgets, and
   restricted template/context/egress/extension/workspace allowlists.
3. Open project detail and confirm every configured limit, current counter,
   enforcement mode, alert, operation policy, and resolved allowlist is visible
   without raw proxy/storage material.
4. Create one active and one queued session in the project. Confirm project
   detail shows both with state/admission/queue evidence and working links.
5. Open New Session, select the project, and confirm its pressure/alerts appear.
   Verify disallowed template/context/egress options remain visible but disabled
   with the project reason, while allowed options can be submitted.
6. Change policy after the form loads and submit the previously allowed choice;
   confirm the authoritative gateway conflict appears without clearing the
   draft.
7. Start two project workflow runs with a one-run limit. Confirm the second is
   queued and both appear on project detail.
8. Open the queued run and confirm project link, project admission reason and
   message, active/max count, generic queue reason, and queued time.
9. Launch a workflow from its definition route using the project selector;
   confirm archived projects are blocked and active-project quota/alert context
   is visible.
10. Open a project session's Policy route and confirm effective capabilities,
    operation restrictions, selected-resource policy, and project navigation
    agree with project detail.
11. Disable uploads/downloads/session-file bindings/manual recordings and
    confirm the corresponding existing routes/actions explain the block.
12. Force related-session and workflow-run request failures independently and
    confirm project configuration plus successful evidence remain usable.
13. Repeat core project detail, session create, and workflow-run checks at a
    390 px viewport and verify no horizontal overflow or overlapping controls.

## Post-Implementation Smoke Sequence

1. Run project-governance, project client/view-model/form/detail, session create,
   session policy, workflow launcher, and workflow-run detail unit/component
   tests.
2. Run admin-new check, full unit suite, coverage ratchet, and production build.
3. Run focused gateway project/admission/policy/retained-storage/egress/session
   queue/workflow-run queue tests.
4. Run admin-new project smoke through constrained project -> allowed session ->
   queued session -> blocked resource -> policy evidence -> cleanup.
5. Run workflow admission and queued-cancel smokes and verify project/run detail
   evidence through admin-new.
6. Run compatibility-admin project, session, workflow, and operator CLI
   regressions.
7. Run the impacted Compose gateway matrices and browser/integration stages.
8. Verify desktop/mobile screenshots, browser console/network cleanliness, and
   absence of sensitive egress values.
9. Run repository document, OpenAPI, and hosted PR validation.

## Documentation And Claim Impact

- Update `README.md` because project and action-level governance visibility is
  user-facing.
- Update `PROJECT_GOVERNANCE_REQUIREMENTS.md`, `ADMIN_NEW_STATUS.md`,
  `ADMIN_NEW_MANUAL_CHECKPOINTS.md`, `DELIVERY_ROADMAP.md`,
  `IMPLEMENTATION_WORK_ORDER.md`, and `OPEN_ISSUES_CONTEXT.md` after validation.
- `ARCH.md` should not change unless implementation introduces a boundary not
  described here; consuming existing resources in additional views is not an
  architecture change.
- OpenAPI should not change because this slice consumes frozen project,
  session, and workflow-run evidence.

## Definition Of Done

- Project detail answers what is limited, what is consumed, what is queued or
  blocked, which resources are allowed, and where related work can be inspected.
- Session and workflow creation show selected-project governance before submit
  without pretending to replace server admission.
- Workflow-run and session policy views render consistent authoritative project
  reason evidence and navigation.
- Egress evidence remains sanitized and retained-storage evidence exposes only
  counts/limits and normal resource links.
- Partial failures preserve usable project configuration and successful
  evidence.
- Focused unit/integration, full frontend, gateway regression, CLI,
  compatibility, Compose smoke, responsive, and hosted checks pass.
- README/docs, issue `#161`, and PR evidence agree.
