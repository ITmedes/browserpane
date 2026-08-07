# Unified Admin App Requirements

This is the standalone requirements document for the `/admin-new/` redesign.
It consolidates the previous BPANE-00142 planning files and current app state.

## Product Position

The unified admin app is a route-backed operational control plane. It must
eventually replace the old `/admin/` app, but only after parity and security
gates pass.

Current rule:

- `/admin/` remains the stable/default admin console.
- `/admin-new/` remains available for incremental testing and implementation.
- Both apps must coexist until promotion is explicitly accepted.

## Core Design Direction

The admin app must feel like an operational tool, not a marketing page.

Required interaction qualities:

- dense but readable layouts,
- persistent navigation and stable route state,
- clear selected-resource metadata,
- route-backed tabs instead of hidden overlay-only state,
- explicit loading, empty, error, disabled, and success states,
- visible action feedback close to the action that caused it,
- stable browser viewport sizing from container dimensions,
- no nested cards,
- no fake browser chrome or prototype-only UI elements,
- no mock data as production defaults,
- no provider references copied from external competitor/prototype material.

Prototype-only affordances must be translated into BrowserPane product
behavior before implementation. In particular, the legacy prototype
`ShareTokenForm` concept stays deferred until a backend share/handoff contract
exists.

## Information Architecture

The navigation groups are:

- Dashboard
- Operate
  - Sessions
  - Recordings
  - Workflows
  - Workflow runs
- Resources
  - Projects
  - Browser contexts
  - Egress profiles
  - File workspaces
- Govern
  - Identity and access
  - API reference
- Docs
  - Design memo
  - API coverage

Route naming must use BrowserPane resource language:

- `/admin-new/workflow-runs` is preferred over `/admin-new/runs`; the current
  `/runs` route should get an alias or redirect before promotion.
- `/admin-new/browser-contexts`, not `/contexts`.
- `/admin-new/files/workspaces`.
- Session subroutes should use:
  - `/admin-new/sessions/[session_id]/live`
  - `/admin-new/sessions/[session_id]/files`
  - `/admin-new/sessions/[session_id]/recordings`
  - `/admin-new/sessions/[session_id]/network`
  - `/admin-new/sessions/[session_id]/automation`
  - `/admin-new/sessions/[session_id]/policy`
  - `/admin-new/sessions/[session_id]/observability`

## Current Old-Admin Parity Families

These behavior families must remain available through either `/admin/` or
implemented `/admin-new/` routes until promotion:

- Auth bootstrap, OIDC login completion, token refresh, logout, and global auth
  failure handling.
- Global messages, accessibility roles, and route-level feedback.
- Session selection, join/reconnect/disconnect, lifecycle actions, connection
  disconnect, queue cancel, stop, release, kill, and egress probe.
- Live browser attach state, browser SDK loading, local certificate hash and
  fingerprint handling, upload, microphone, camera, display controls, and
  viewport sizing.
- Session files, session file bindings, workspace files, downloads, uploads,
  mount-path validation, and provenance.
- Recording status, start/stop controls where supported, retained segments,
  playback/export downloads, and recording snapshot updates.
- MCP delegation, workflow run creation, run cancel/resume/reject/input submit,
  workflow visibility rules, produced files, run logs, and run events.
- Browser contexts: create, clone, import, export, delete, storage/retention
  visibility, and active-writer constraints.
- Egress profiles: local presets, create/edit/clone/disable/probe, TLS
  interception fields, proxy auth fields, and diagnostics evidence.
- Identity: current principal, access review, service principals, identity
  mappings, delegated principals, unmapped signals, create/edit/disable.
- Metrics/logs: admin event stream status, session snapshots, workflow
  snapshots, recording snapshots, gateway logs, copy/clear behavior.
- Browser policy: local-file and File System Access guardrails, probe command,
  and CDP endpoint evidence.

## Route Requirements

### Dashboard

Purpose: give operators a first-screen summary of active resources and recent
operational state.

Required first-pass content:

- sessions summary,
- project quota/alert hints,
- browser context state,
- egress profile state,
- file workspace state,
- workflow definition and run state,
- recording state,
- direct links to active work.

Current status: first-pass implemented.

### Projects

Purpose: define governance boundaries used by sessions, workflows, resource
allowlists, quota admission, and usage reporting.

Required catalog behavior:

- concept-style table overview,
- search and lens filtering,
- project scope/state,
- quotas,
- policy gates,
- usage counters,
- alerts,
- activity/runtime/egress/storage columns,
- details action to route-backed detail.

Required detail/create/edit behavior:

- edit name, description, labels, and state,
- enable/disable policy gates,
- configure allowlists for session templates, browser contexts, egress
  profiles, extensions, and file workspaces from backend-backed catalogs,
- configure active sessions, workflow runs, storage/usage, runtime, and
  creation-rate quotas where the API supports them,
- keep usage and generated alerts read-only evidence,
- validate invalid quotas before save,
- redirect to auth on expired tokens.

Current status: implemented first pass.

### Browser Contexts

Purpose: reusable persisted browser profiles for sessions and workflows.

Required behavior:

- catalog with context state, scope, project binding, reusable flag, active
  writer/reference evidence, storage and retention status,
- create route,
- detail/edit route,
- prevent unsafe delete when active sessions or writers exist,
- expose clone/import/export as follow-up operations where API/runtime support
  is present.

Current status: catalog/create/detail/edit implemented. Import/export/clone UX
is not the main promotion blocker but must remain in old admin/API until
implemented.

### Egress Profiles

Purpose: configure network identity and outbound proxy/observer behavior for
sessions.

Required behavior:

- catalog with direct/proxy/TLS-intercept mode,
- project binding where applicable,
- profile enabled/disabled state,
- create/edit,
- clone/disable where API supports it,
- reachability/probe diagnostics,
- sanitized diagnostics evidence,
- proxy auth credential binding references without inline secrets,
- TLS interception only when mode is explicit and proxy, CA, and sensitive log
  sink references are configured.

Current status: catalog/create/edit implemented in unified admin; production
proxy-auth validation and local presets exist at the platform level.

### File Workspaces

Purpose: owner/project-scoped storage for files that can be used by sessions
and workflows.

Required behavior:

- catalog,
- create/edit/detail,
- project binding,
- upload/download/delete files,
- provenance metadata,
- clear validation messages,
- session-file binding integration from session subroutes.

Current status: workspace catalog/create/detail/edit implemented. Session-file
subroute remains missing.

### Sessions

Purpose: create, inspect, connect, control, and delegate live browser sessions.

Required catalog behavior:

- focused list,
- selected-session metadata area,
- state filters/lenses for all, active, queued, stopped, and needs-attention,
- explicit selected session state,
- session switch should not leave stale browser connection state.

Required creation behavior:

- project selection,
- session template selection,
- browser context mode/reference,
- network identity and egress profile,
- labels,
- idle timeout,
- owner/collaboration mode,
- capability checkboxes,
- recording policy,
- API payload preview,
- create should not auto-start a session before the operator submits the form.

Required detail behavior:

- lifecycle actions,
- stop/release/kill/cancel eligibility and reasons,
- queue state and queue cancel,
- connections and disconnect-all/per-connection disconnect,
- current template/project/context/network identity/capability facts,
- recording policy and current recording state,
- MCP delegation card,
- route-backed subareas for live, files, recordings, network, automation,
  policy, and observability.

Current status: catalog/create/detail/preview/MCP are implemented. Route-backed
live, files, recordings, and network subareas are implemented; automation,
policy, and observability remain issue #156.

### Session Preview And Live Browser

Purpose: view and control the selected browser session in a dedicated surface.

Required behavior:

- preview popup or route-backed live tab,
- browser SDK loading from `/dist/`,
- WebTransport connect-ticket flow,
- container-based viewport sizing,
- independent width/height resize unless a fixed resolution/aspect policy is
  selected,
- attach/detach and reconnect,
- upload, microphone, camera, and display controls where session policy allows,
- clear connection state and error feedback,
- metrics drawer for local browser transition diagnostics.

Current status: preview popup implemented with metrics drawer. The dedicated
`/live` route provides refresh-safe connection evidence and launches that
standalone stream surface explicitly.

### Recordings

Purpose: configure and inspect retained recording artifacts.

Required behavior:

- recording disabled by default,
- session creation can opt into recording,
- existing sessions can change recording policy without starting stopped
  sessions implicitly,
- active connections should finalize recordings when disconnect/stop/kill
  occurs,
- top-level recordings catalog,
- session-specific recordings subroute,
- segment download,
- playback manifest/export download,
- clear failed/unavailable artifact state.

Current status: top-level recordings catalog and session policy controls exist.
Session subroute and deeper playback management remain incomplete. Backend
artifact-boundary hardening remains a priority.

### Workflows

Purpose: define and execute browser workflows against BrowserPane sessions.

Required behavior:

- workflow catalog,
- hidden/smoke workflow filtering,
- definition detail,
- version metadata,
- source reference summaries,
- source tree and file navigation,
- TypeScript syntax-highlighted code preview using a library,
- launch controls with schema-backed input parameters,
- baseline session/project/context selectors,
- clear external integration guidance.

Current status: catalog/detail/source browser/code preview/launch controls exist
in first pass. Publishing/catalog management remains incomplete.

### Workflow Integration Endpoints

Purpose: expose approved BrowserPane workflows as stable, project-scoped
actions for external BPM and workflow systems.

Recommended routes:

- `/admin-new/workflow-endpoints`
- `/admin-new/workflow-endpoints/new`
- `/admin-new/workflow-endpoints/[endpoint_id]`
- `/admin-new/workflow-endpoints/[endpoint_id]/runs`
- `/admin-new/workflow-endpoints/[endpoint_id]/deliveries`

Required behavior:

- catalog stable endpoint keys, state, project, active workflow/version, and
  contract version,
- create/edit draft endpoints and activate/disable/deprecate them,
- promote an approved immutable workflow version without changing the external
  endpoint key,
- show immutable endpoint revisions, environment/stage, compatibility status,
  and audited rollback,
- preview input/output schemas, deadlines, inline-result limits, supported
  controls, and callback contract,
- manage registered service-principal grants and operation scopes,
- configure endpoint/caller concurrency and request-rate limits,
- explain polling, webhook, and callback-token completion profiles,
- select and explain external-managed versus BrowserPane-managed Human Handoff,
- show endpoint data-classification, redaction, retention, environment, and
  private-connectivity policy references,
- show copyable invocation and authentication examples without secret
  material,
- inspect correlated recent runs, typed outcomes, progress, intervention state,
  artifacts, attempt/checkpoint evidence, side-effect uncertainty, and callback
  delivery health,
- show readiness, degraded/maintenance, admission, throttling, and overload
  diagnostics without implying that an HTTP listener is execution-ready,
- show event sequence/reconciliation health without exposing connector or
  target-system credentials,
- rotate signing configuration and trigger bounded redelivery where
  authorized,
- expose audit evidence for activation, grant, promotion, invocation, and
  intervention decisions.

Endpoint management is route-backed, full-width resource administration. It
must not be placed in the compact Operations Overlay, and it must not create a
second UI-only endpoint or permission model.

Current status: missing. This is Phase N issue `#172`, not an Admin-New
promotion blocker.

### Workflow Studio And Teach Mode

Purpose: turn reviewed process demonstrations and prose intent into validated
workflow candidates without introducing autonomous production mutation.

Recommended routes:

- `/admin-new/workflows/teach`
- `/admin-new/workflow-training/[draft_id]`

Required behavior:

- create and resume a training draft,
- edit prose intent, allowed domains/actions, inputs/outputs, Human Gates,
  failure policy, and data-sensitivity constraints,
- start/stop a governed demonstration session,
- review a normalized semantic timeline and bounded evidence,
- annotate variables, Credential Bindings, outputs, assertions, branches, and
  escalation points,
- review generated plan, schemas, source tree/diff, selector evidence,
  requirements, warnings, and provenance,
- start fresh-context validation and inspect scenario results,
- approve/reject and publish only through the immutable workflow version
  contract,
- inspect controlled-repair candidates without changing the published version.

The Studio should be route-backed and full-width. It does not belong inside the
compact Operations Overlay. Secret values and unrestricted browser evidence
must not be shown or sent to a compiler provider without explicit policy.

Current status: missing. This is Phase N issue `#171`, not an admin-new
promotion blocker.

### Workflow Runs

Purpose: inspect and operate workflow executions.

Required overview behavior:

- run list,
- state/search filters,
- workflow/session/project links,
- external correlation where available.

Required detail behavior:

- route-backed `/admin-new/workflow-runs/[run_id]` or `/runs/[run_id]`,
- run metadata, state, timestamps, project/session/workflow references,
- logs,
- events,
- outputs and structured result,
- produced files with downloads,
- errors,
- cancel/resume/reject/input submit controls where supported,
- related session connect/inspect links.

Current status: implemented at canonical `/admin-new/workflow-runs` overview
and detail routes, with `/admin-new/runs` compatibility aliases. The inspector
includes independently loaded evidence, safe state-gated controls, produced
file downloads, and related workflow/session links.

### Identity And Access

Purpose: make enterprise identity/access-review facts visible and manageable.

Required behavior:

- current principal,
- visible projects/resources,
- delegated service principals,
- service principal registry with create/edit/disable,
- identity mappings for users, groups, claims, and registered service
  principals,
- unmapped signals,
- access-review evidence for delegated sessions and project usage,
- safe token claim rendering without raw token payload exposure.

Current status: backend and prior admin functionality exist; unified admin route
is missing despite navigation.

### API Companion And Coverage

Purpose: support operators/integrators with copyable API examples and operation
coverage.

Required behavior:

- route-backed `/admin-new/api`,
- route-backed `/admin-new/coverage` or hidden navigation until implemented,
- OpenAPI operation family list,
- classification for `ui-primary`, `ui-evidence`, `api-companion`, and
  `internal-worker`,
- copyable examples,
- clear separation between owner bearer APIs, worker/session automation APIs,
  and compatibility endpoints.

Current status: missing.

## Pattern Requirements

Use local Svelte/TypeScript patterns where they reduce duplication. Do not add
a third-party component framework unless a concrete gap justifies it.

Initial reusable patterns:

- shell/nav/header/auth/global feedback,
- page header,
- searchable resource list/table,
- selected-resource summary,
- route-backed detail tabs,
- compact action bar with disabled reasons for real resource commands,
- status badges,
- feedback message,
- empty/loading/error states,
- form section and field row,
- JSON payload preview that does not auto-collapse,
- danger zone,
- upload/download actions,
- live viewport frame,
- copy button,
- command palette.

The `ActionBar` pattern should group actual resource actions near the selected
resource metadata. It must not carry mock concept actions into production, and
unsupported actions should be disabled with a reason or omitted.

Acceptance for a pattern:

- at least one real route consumer,
- component/view-model tests where logic exists,
- documented loading/empty/disabled/error states where relevant,
- stable selectors if smokes depend on it.

## Selector Requirements

Selectors must be semantic and tied to behavior. Do not make smokes depend on
cosmetic wrappers.

High-risk selectors to preserve or update in the same slice as the smoke:

- `browser-viewport`
- `browser-viewport-mount`
- `session-row`
- `session-inspector-row`
- `session-join`
- `session-disconnect`
- `session-detail-link`
- `admin-log-entry`
- `admin-global-message-region`
- `recording-library-row`
- `recording-segment-download`
- `download-recording`
- `file-workspace-file-row`
- `session-file-binding-row`
- `session-file-binding-download`
- `egress-profile-row`
- `egress-profile-edit`
- `egress-profile-clone`
- `egress-profile-reachability-probe`
- `browser-context-row`
- `browser-context-clone`
- `browser-context-import`
- `browser-context-export`
- `workflow-run-inspector-row`
- `workflow-catalog-row`
- `identity-service-principal-row`
- `identity-mapping-row`
- `download-workflow-file`

## Promotion Gate

Do not promote `/admin-new` until:

1. all visible navigation routes exist or are intentionally hidden,
2. old-admin parity families are implemented or explicitly deferred,
3. route-backed session and workflow-run detail gaps are closed,
4. identity/access review exists,
5. API companion/coverage route state is resolved,
6. security cleanup items that affect admin trust are addressed,
7. old and new admin smoke suites pass,
8. manual regression checkpoints pass,
9. `/admin/` remains available as a fallback until a dated removal gate is
   accepted.
