# BrowserPane Domain Requirements Relevant To Admin-New

This document preserves the still-valid product and control-plane requirements
from older plans. It is organized by domain rather than by historical issue.

## Sessions

Sessions are owner-scoped browser resources backed by Postgres and runtime
assignments. They can be active, queued, released, stopped, killed, or failed
depending on lifecycle and admission state.

Admin requirements:

- create sessions with project, template, context, network identity, egress,
  extensions, labels, owner mode, idle timeout, capabilities, and recording
  policy,
- list/filter sessions by state, template, labels, project, and runtime state,
- show effective template/project/context/network/egress/capability facts,
- connect/reconnect using short-lived connect tickets,
- release runtime without deleting the session resource,
- stop only when eligibility allows it,
- cancel queued sessions,
- kill sessions when cleanup is needed,
- disconnect all clients or targeted connections,
- keep stopped/released reconnect semantics clear to users.

Important lifecycle behavior:

- if a worker is still alive, reconnect should return to the exact live
  runtime,
- after idle stop or release, reconnect restarts from persisted Chromium
  profile where available,
- exact process memory state is not preserved after worker shutdown,
- queued sessions must not mint usable browser connect tickets until admitted,
- switching selected sessions in the UI must not leave stale browser state.

## Session Templates

Session templates are owner-scoped defaults for session creation.

Requirements:

- create/list/get/update template resources,
- version template updates,
- define defaults for fields already supported by session creation,
- merge template defaults with caller overrides on session creation,
- expose template id and metadata in session resources and catalogs,
- support session catalog filtering by template id and metadata.

Current admin-new state:

- templates are selectable during session creation,
- project policy allowlists can reference templates,
- dedicated admin template catalog management is still missing.

Remaining work:

- add template catalog management if non-CLI template operations become a demo
  or operator priority,
- include duplicate/archive/version visibility where supported by API.

## Browser Contexts

Browser contexts are owner-scoped reusable Chromium profile resources.

Requirements:

- represent reusable browser state as explicit resources,
- mount context-scoped Chromium profile volumes for reusable contexts,
- keep upload/download/session-file data session-scoped,
- allow only one active writer per reusable context,
- expose active writer/reference and storage-limit evidence,
- support clone/export/import where runtime-backed profile data exists,
- prevent destructive operations while active writers exist,
- handle retention cleanup for expired ready contexts.

Current admin-new state:

- catalog/create/detail/edit exist,
- clone/import/export still need UI parity where considered promotion scope.

## Projects, Quotas, Admission, And Usage

Projects are governance boundaries for sessions, workflows, resource usage, and
policy allowlists.

Requirements:

- project resource metadata: name, description, labels, state, scope,
  timestamps,
- admission quotas for active sessions and active workflow runs,
- queueing and queue visibility where quota is exhausted,
- warning-only and blocking budget modes,
- session creation rate limits over a configured window,
- runtime usage budgets,
- retained storage accounting,
- egress byte usage ingestion and rollup,
- project-scoped egress profiles and credential bindings,
- project policy allowlists for session templates, browser contexts, egress
  profiles, extensions, and file workspaces,
- project policy controls for live upload/download, session-file bindings, and
  manual recording starts,
- clear admission errors and stable API shapes.

Current admin-new state:

- project catalog/create/detail/edit exists,
- quotas/policy settings are editable,
- project selectors are used by session creation,
- richer workflow quota/queue evidence and cross-resource project governance
  can still be improved.

Remaining work:

- surface project workflow-run quota/queue state clearly,
- connect retained storage and artifact quotas once artifact models mature,
- feed audit/event surfaces once generalized events exist.

## Network Identity And Egress

Network identity covers locale, timezone, geolocation, user agent/browser
identity, egress profile, custom CA trust, observer metadata, and proxy auth.

Requirements:

- session and template network identity fields,
- owner/project-scoped egress profile resources,
- explicit proxy and TLS-intercept modes,
- no inline proxy credentials in normal resources,
- credential binding references resolved only at runtime launch,
- sanitized diagnostics and observer correlation,
- safe docker runtime labels for session/profile/container correlation,
- local presets for no egress, proxy, and TLS interceptor,
- active probes only against already-ready session runtimes,
- diagnostics must not implicitly start stopped sessions.

Privacy boundary:

- BrowserPane stores sanitized session/profile/container correlation metadata
  and sanitized egress byte deltas,
- the configured proxy or secure web gateway owns outbound URL/status/timing
  and full traffic logs,
- BrowserPane must not ingest requested URLs, headers, proxy credentials,
  payloads, decrypted traffic, raw CA material, or sensitive-log sink secrets.

Current admin-new state:

- egress profile catalog/create/edit exists,
- session creation can reference egress profiles,
- session network subroute is missing.

## File Workspaces And Session Files

File workspaces provide owner/project-scoped file resources used by sessions
and workflows. Session files bind workspace files to relative mount paths.

Requirements:

- workspace catalog/create/detail/edit,
- workspace file upload/download/delete,
- project binding,
- opaque artifact refs rather than raw local filesystem paths,
- provenance metadata,
- session-file binding resource with safe relative mount path validation,
- automation access can read/list session file bindings before runtime
  materialization,
- project policy can restrict file workspace use and session-file binding.

Current admin-new state:

- workspace UI exists,
- session-specific file binding route is missing.

## Recordings

Recordings are session-scoped segments produced by a recording worker and
retained by the gateway artifact store.

Requirements:

- recording disabled by default,
- sessions can be configured for recording at creation time,
- existing sessions can update recording policy without implicitly starting a
  stopped runtime,
- `always` mode should start recording when the session starts,
- disconnect/stop/release/kill should finalize or fail recordings with clear
  reasons,
- recorder-only clients must not count as user activity that blocks idle/stop,
- stale in-flight segments after restart should fail and link to fresh
  segments rather than pretending continuity,
- retained ready segments should support WebM download,
- playback/export should package a manifest, player, and included media files,
- completed artifacts can expire while segment metadata remains.

Security requirement:

- completion must not let a recording worker or automation token move arbitrary
  gateway-local files; it must use a controlled staging boundary or opaque
  artifact handoff.

Current admin-new state:

- top-level recordings catalog exists,
- session recording policy is visible/editable in session detail,
- session-specific recordings route and artifact-boundary hardening remain.

## Workflows

Workflows are owner-scoped definitions with immutable versions and source
metadata. Runs execute with a workflow worker against BrowserPane sessions.

Requirements:

- workflow catalog,
- hidden/smoke workflow filtering,
- definition detail and version metadata,
- git-backed source snapshots pinned to immutable commit at publish time,
- source tree/file browser,
- code preview with TypeScript syntax highlighting via a library,
- schema-backed launch input parameters,
- workflow run creation against selected/baseline session or new session,
- runtime hold/release semantics for paused/intervention states,
- run logs, events, outputs, produced files, and errors,
- cancel queued/active runs where safe,
- resume/reject/input submit for awaiting-input interventions,
- workspace inputs and credential bindings scoped by project.

Source security requirements:

- reject dangerous git URL forms and unsupported protocols,
- preserve local trusted `/workspace` development roots explicitly,
- set git protocol restrictions,
- reject symlink source previews and path escapes,
- enforce source listing/preview limits.

Current admin-new state:

- workflow catalog/detail/source browser/code preview/launch controls exist,
- workflow runs overview exists,
- workflow-run detail route is missing.

## MCP Delegation And Operator CLI

MCP delegation allows automation clients to drive governed BrowserPane
sessions.

Requirements:

- session-scoped MCP endpoint: `/sessions/{session_id}/mcp`,
- session-scoped SSE endpoint where legacy SSE is supported,
- compatibility `/mcp` default-session behavior for older clients,
- explicit session automation delegate through the gateway API,
- direct bridge control mutation must require internal bearer auth,
- browser/admin callers mutate default bridge control through authenticated
  gateway proxy,
- CLI commands for session list/get/status/create/stop/kill/access-token,
  automation-access, disconnect-all, cleanup, MCP health/authorize/revoke,
  set-default, clear-default, doctor, preflight, and repair,
- CLI profile config with predictable precedence and safe file permissions,
- strict non-zero preflight behavior for automation.

Current state:

- MCP control auth is implemented in the current baseline,
- session-scoped MCP endpoint smoke verifies real `tools/list` and
  `browser_navigate` against isolated sessions,
- unified admin MCP card is implemented in session detail.

Remaining MCP/security work:

- decide whether exposed `/mcp`/`/sse` transports need inbound auth or
  internal-network-only binding for production,
- keep health detail split by trust level where needed.

## Identity And Access

Identity resources normalize external users/groups/claims and delegated service
principals into BrowserPane access-review surfaces.

Requirements:

- current principal endpoint,
- access review summarizing visible projects/resources and delegated principals,
- service-principal registry with lifecycle metadata,
- disabled service principals block new automation delegation,
- identity mappings for users, groups, claims, and registered service
  principals,
- safe rendering of token claims without raw token payloads,
- unmapped signal evidence,
- project mapping facts consumed by governance/admission.

Current admin-new state:

- backend and old-admin/CLI foundations exist,
- `/admin-new/identity` route is missing.

## Admin Feedback And Observability

Operators must get visible messages for state changes and external updates.

Requirements:

- global notifications,
- browser connection success/failure/disconnect messages,
- lifecycle success/error messages,
- admin event stream health,
- selected-session diffs from external state changes,
- workflow run state messages,
- artifact and delegation snapshot messages,
- gateway logs and copy/clear behavior where exposed.

Current admin-new state:

- component-level feedback patterns exist,
- preview metrics drawer exists,
- route-backed observability/logs/event-stream surface is missing.
