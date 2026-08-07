# Admin-New API And Compatibility Coverage

This document preserves the API coverage requirements that must guide the
`/admin-new` migration. It is standalone and should be used when adding route
clients, API examples, mapper tests, or the future API companion route.

## Coverage Baseline

The frozen owner-scoped control contract is `openapi/bpane-control-v1.yaml`.
The admin app must also account for gateway and MCP compatibility endpoints
that sit outside that frozen contract.

Issue #179 owns API lint, implementation conformance, executable examples,
breaking-change detection, and the compatibility/deprecation policy. Issue
#158 owns the admin-new API companion/coverage UI. The UI must consume the
canonical contract and conformance evidence; it must not become a separate
handwritten API truth.

Current generated and contract-tested baseline:

- OpenAPI operations: 131
- OpenAPI operations recognized by the Axum router contract: 131
- OpenAPI component schemas: 202
- OpenAPI ref names: 231
- reusable component parameters: 23
- reusable component responses: 6
- security schemes: 2
- schema properties audited: 1017
- required schema entries audited: 853
- content types: `application/json`, `application/octet-stream`,
  `application/zip`, and `video/webm`

Operation metadata is generated into
`openapi/bpane-control-v1.operations.json`. Classification is enforced against
`openapi/bpane-control-v1.classifications.json` whenever the contract changes.

The generated counts are inventory evidence. Redocly lint, OpenAPI Enforcer
examples, semantic compatibility diffing, and the Rust Axum route contract add
the enforceable #179 ratchet under #151 CI. Domain and Compose tests remain
required for behavior that schemas cannot prove.

## Contract Governance Requirements

- parse and lint the complete OpenAPI document in CI,
- compare public route/method coverage with the implemented router,
- execute representative success and error examples,
- validate response schemas and content types,
- detect breaking path, parameter, security, request, response, enum, and
  required-field changes against the supported baseline,
- publish additive-change, deprecation, compatibility-window, and versioning
  rules,
- generate compatibility exports for #172 connectors from this contract rather
  than maintaining parallel schemas,
- keep internal/compatibility endpoints explicitly outside the frozen public
  surface until intentionally promoted.

## Classification Rules

Every OpenAPI operation must have exactly one admin-new classification:

- `ui-primary`: normal operator workflow in the new app.
- `ui-evidence`: read-only evidence in an existing resource/detail view.
- `api-companion`: documented and copyable, but not a first-class UI control.
- `internal-worker`: worker, executor, bridge, recorder, or observer-owned
  route that must not become a normal operator button.

Current classification counts:

- `ui-primary`: 108
- `ui-evidence`: 6
- `api-companion`: 5
- `internal-worker`: 12

## API Families

The API companion and coverage view must include these families:

- Admin Events
- Automation Tasks
- Browser Contexts
- Credential Bindings
- Egress Profiles
- Extensions
- File Workspaces
- Identity
- Projects
- Session Automation
- Session Files
- Session Recordings
- Session Runtime
- Session Templates
- Sessions
- Workflows

## Operation Classification Matrix

### `ui-primary`

- Admin events: `issueAdminEventAccessToken`, `openAdminEvents`
- Browser contexts: `listBrowserContexts`, `createBrowserContext`,
  `importBrowserContext`, `getBrowserContext`, `cloneBrowserContext`,
  `exportBrowserContext`, `deleteBrowserContext`
- Credential bindings: `listCredentialBindings`, `createCredentialBinding`,
  `getCredentialBinding`
- Egress profiles: `listEgressProfiles`, `createEgressProfile`,
  `getEgressProfile`, `updateEgressProfile`,
  `getEgressProfileDiagnostics`, `runEgressProfileReachabilityProbe`
- Extensions: `listExtensions`, `createExtension`, `getExtension`,
  `createExtensionVersion`, `enableExtension`, `disableExtension`
- File workspaces: `listFileWorkspaces`, `createFileWorkspace`,
  `getFileWorkspace`, `listFileWorkspaceFiles`,
  `uploadFileWorkspaceFile`, `getFileWorkspaceFile`,
  `getFileWorkspaceFileContent`, `deleteFileWorkspaceFile`
- Identity: `getCurrentIdentity`, `getIdentityAccessReview`,
  `listServicePrincipals`, `createServicePrincipal`,
  `getServicePrincipal`, `updateServicePrincipal`,
  `listIdentityMappings`, `createIdentityMapping`, `getIdentityMapping`,
  `updateIdentityMapping`
- Projects: `listProjects`, `createProject`, `getProject`,
  `updateProject`, `getProjectUsage`
- Session automation: `setAutomationOwner`, `clearAutomationOwner`
- Session files: `listSessionFiles`, `getSessionFile`,
  `getSessionFileContent`, `listSessionFileBindings`,
  `createSessionFileBinding`, `getSessionFileBinding`,
  `getSessionFileBindingContent`, `removeSessionFileBinding`
- Session recordings: `listSessionRecordings`, `getSessionRecording`,
  `getSessionRecordingContent`, `getSessionRecordingPlayback`,
  `getSessionRecordingPlaybackManifest`,
  `getSessionRecordingPlaybackExport`, `updateSessionRecordingPolicy`
- Session runtime: `issueSessionAccessToken`, `getSessionStatus`,
  `getSessionEgressDiagnostics`, `runSessionEgressDiagnosticsProbe`
- Session templates: `listSessionTemplates`, `createSessionTemplate`,
  `getSessionTemplate`, `updateSessionTemplate`
- Sessions: `listSessions`, `createSession`, `getSession`,
  `cancelQueuedSession`, `disconnectSessionConnection`,
  `disconnectAllSessionConnections`, `stopSession`,
  `releaseSessionRuntime`, `killSession`
- Workflows: `listWorkflowDefinitions`, `createWorkflowDefinition`,
  `getWorkflowDefinition`, `validateWorkflowDefinitionSource`,
  `getWorkflowDefinitionSourcePreview`, `listWorkflowDefinitionSourceFiles`,
  `listWorkflowDefinitionVersions`,
  `createWorkflowDefinitionVersion`, `getWorkflowDefinitionVersion`,
  `listWorkflowRuns`, `createWorkflowRun`, `getWorkflowRun`,
  `cancelWorkflowRun`, `rejectWorkflowRun`, `resumeWorkflowRun`,
  `submitWorkflowRunInput`, `getWorkflowRunEvents`,
  `getWorkflowRunLogs`, `listWorkflowRunProducedFiles`,
  `getWorkflowRunProducedFileContent`,
  `getWorkflowRunSourceSnapshotContent`,
  `getWorkflowRunWorkspaceInputContent`,
  `listWorkflowEventSubscriptions`, `createWorkflowEventSubscription`,
  `getWorkflowEventSubscription`, `deleteWorkflowEventSubscription`,
  `listWorkflowEventDeliveries`

### `ui-evidence`

- Operation counters: `getWorkflowOperations`, `getRecordingOperations`
- Automation task evidence: `listAutomationTasks`, `getAutomationTask`,
  `getAutomationTaskEvents`, `getAutomationTaskLogs`

### `api-companion`

- Safe-stop compatibility: `deleteSession`
- Direct automation task management: `createAutomationTask`,
  `cancelAutomationTask`
- API-backed recording controls that are not current operator controls:
  `createSessionRecording`, `stopSessionRecording`

### `internal-worker`

- Automation task worker routes: `appendAutomationTaskLog`,
  `transitionAutomationTaskState`
- Workflow worker routes: `appendWorkflowRunLog`,
  `transitionWorkflowRunState`, `uploadWorkflowRunProducedFile`,
  `getWorkflowRunCredentialBindingResolved`
- Recording worker routes: `completeSessionRecording`,
  `failSessionRecording`
- Egress observer route: `reportSessionEgressUsage`
- Bridge/automation-owned session routes: `issueSessionAutomationAccess`,
  `setSessionMcpOwner`, `clearSessionMcpOwner`

## Current Wrapper And Alias Requirements

Preserve or deliberately replace current frontend aliases during API extraction:

- control client aliases: `setAutomationDelegate`,
  `clearAutomationDelegate`, `downloadSessionFileContent`,
  `downloadFileWorkspaceFileContent`,
  `downloadSessionFileBindingContent`,
  `downloadSessionRecordingContent`,
  `downloadSessionRecordingPlaybackExport`
- workflow client aliases: `listDefinitions`, `createDefinition`,
  `getDefinition`, `listDefinitionVersions`, `createDefinitionVersion`,
  `getDefinitionVersion`, `createRun`, `listRuns`, `getRun`,
  `cancelRun`, `resumeRun`, `submitRunInput`, `rejectRun`,
  `listRunEvents`, `listRunLogs`, `listProducedFiles`,
  `downloadProducedFileContent`
- MCP bridge client aliases: `getHealth`, `setControlSession`,
  `clearControlSession`
- admin event lifecycle: preserve `subscribe`, reconnect status transitions,
  and the current event-stream auth behavior until the token cleanup slice
  replaces raw owner-token query auth

Known OpenAPI operations without direct current frontend wrappers or with
intentional non-operator classification:

- `appendAutomationTaskLog`
- `appendWorkflowRunLog`
- `cancelAutomationTask`
- `clearSessionMcpOwner`
- `completeSessionRecording`
- `createAutomationTask`
- `createCredentialBinding`
- `createExtension`
- `createExtensionVersion`
- `createSessionRecording`
- `createSessionTemplate`
- `createWorkflowEventSubscription`
- `deleteSession`
- `deleteWorkflowEventSubscription`
- `disableExtension`
- `enableExtension`
- `failSessionRecording`
- `getAutomationTask`
- `getAutomationTaskEvents`
- `getAutomationTaskLogs`
- `getCredentialBinding`
- `getExtension`
- `getRecordingOperations`
- `getSessionRecording`
- `getSessionRecordingPlaybackManifest`
- `getSessionTemplate`
- `getWorkflowEventSubscription`
- `getWorkflowOperations`
- `getWorkflowRunCredentialBindingResolved`
- `getWorkflowRunSourceSnapshotContent`
- `getWorkflowRunWorkspaceInputContent`
- `issueSessionAutomationAccess`
- `listAutomationTasks`
- `listCredentialBindings`
- `listExtensions`
- `listWorkflowEventDeliveries`
- `listWorkflowEventSubscriptions`
- `reportSessionEgressUsage`
- `setSessionMcpOwner`
- `stopSessionRecording`
- `transitionAutomationTaskState`
- `transitionWorkflowRunState`
- `updateSessionTemplate`
- `uploadWorkflowRunProducedFile`

## Security Model

- Owner/operator calls use `bearerAuth`.
- Worker or automation routes declaring `sessionAutomationAccessToken` must
  stay separated from generic operator services.
- API examples must not imply session automation access is a broad admin token.
- `issueSessionAccessToken` is part of the browser connect flow and must not be
  presented as a generic operator token button.
- `issueSessionAutomationAccess` is internal to workflow, MCP, or session
  automation setup and must not become a generic UI action.
- Worker-owned mutations such as recording completion, workflow state
  transitions, produced-file upload, credential resolution, automation log
  append, and egress usage reporting should appear only as sanitized evidence
  or API companion documentation.

## Schema Property Parity

Generated coverage must track:

- schema name,
- required properties,
- optional properties,
- nullable fields,
- enum values,
- content/media fields,
- link/path fields,
- timestamp fields,
- nested references.

High-risk property families:

- lifecycle timestamps and state transitions such as `created_at`,
  `updated_at`, `started_at`, `completed_at`, `deleted_at`, `last_seen_at`,
  `last_used_at`, `last_attempt_at`, and `next_attempt_at`
- content, export, runtime, log, event, ticket, transport, manifest, status,
  and MCP-owner paths
- egress diagnostics proof fields, runtime assignment/binding evidence,
  observed public IP/TLS issuer fields, profile reachability, warnings,
  sensitive-log sink ids/refs, TLS interception fields, and CA refs
- workflow, recording, event-delivery, playback-export, produced-file, and
  retention counters/totals
- quota and admission counters such as active limits, queued counts, byte
  limits, usage counts, threshold/current/limit values, and state/reason codes
- identity/access-review fields such as delegated principals, resource counts,
  unmapped principal signals, and registered service-principal ids
- automation and worker-owned artifact refs such as `artifact_refs`,
  `automation_task_id`, `events_path`, `logs_path`, and `source_path`

Representative schema families to keep in mapper/view-model tests:

- sessions/runtime: `SessionResource`, `SessionStatus`,
  `SessionRuntimeInfo`, `SessionViewport`, `SessionGeolocation`,
  `SessionNetworkIdentity`, `SessionEffectiveEgress`,
  `SessionConnectInfo`, `SessionIdleStatus`, `SessionConnectionInfo`,
  `SessionAutomationDelegate`, `SessionCapabilities`,
  `SessionStopEligibility`, `SessionAccessTokenResponse`,
  `SessionAutomationAccessResponse`, and `SessionTelemetry`
- session files: `SessionFileResource`, `SessionFileListResponse`,
  `SessionFileBindingResource`, `SessionFileBindingListResponse`,
  `SessionFileBindingMode`, and `SessionFileBindingState`
- recordings: `SessionRecordingResource`, `SessionRecordingListResponse`,
  `SessionRecordingPlaybackResource`, `SessionRecordingStatus`,
  `WorkflowRunRecordingResource`, and `RecordingObservabilitySnapshot`
- browser contexts: `BrowserContextResource`, `BrowserContextListResponse`,
  `BrowserContextUsageResource`, `BrowserContextState`,
  `BrowserContextPersistenceMode`
- egress: `EgressProfileResource`, `EgressProfileListResponse`,
  `EgressProxyConfig`, `EgressCustomCaConfig`,
  `EgressTrafficObservationMode`, `EgressDiagnosticsResource`,
  `EgressDiagnosticsHealth`, `EgressDiagnosticsProofLevel`, and
  `SessionEgressUsageResource`
- projects: `ProjectResource`, `ProjectListResponse`,
  `SessionProjectResource`, `ProjectAdmissionState`, `ProjectQuotas`,
  `ProjectPolicy`, `ProjectUsageResource`, `ProjectUsageAlertResource`,
  and `ProjectUsageBudgetEnforcement`
- identity: `ServicePrincipalResource`, `IdentityMappingResource`,
  `IdentityPrincipalResource`, `IdentityResourceCounts`,
  `IdentityDelegatedPrincipalResource`, and `IdentityAccessReviewResponse`
- workflows: `WorkflowDefinitionResource`, `WorkflowRunResource`,
  `WorkflowDefinitionVersionResource`, `WorkflowGitSource`,
  `WorkflowSource`, `WorkflowRunAdmissionResource`,
  `WorkflowRunInterventionResource`, `WorkflowRunRuntimeResource`,
  `WorkflowRunRetentionResource`, `WorkflowRunProducedFileResource`,
  `WorkflowRunSourceSnapshotResource`, `WorkflowRunWorkspaceInputResource`,
  `WorkflowRunEventResource`, `WorkflowRunLogResource`,
  `WorkflowEventSubscriptionResource`, `WorkflowEventDeliveryResource`,
  and `WorkflowObservabilitySnapshot`
- generic/worker evidence: `AutomationTaskResource`,
  `AutomationTaskEventResource`, `AutomationTaskLogResource`, and
  `OkResponse`

## Request Field Parity

CRUD forms and API companion payload previews must preserve accepted request
fields:

- common metadata: `name`, `description`, `labels`, and `state`
- sessions: `viewport`, `owner_mode`, `idle_timeout_sec`, `labels`,
  `integration_context`, `project_id`, `template_id`, `network_identity`,
  `browser_context`, `extension_ids`, and `recording`
- session templates: `defaults`
- browser contexts: `persistence_mode`, `retention_sec`,
  `max_profile_storage_bytes`, and import metadata headers such as
  `x-bpane-browser-context-name`,
  `x-bpane-browser-context-description`,
  `x-bpane-browser-context-project-id`,
  `x-bpane-browser-context-labels`,
  `x-bpane-browser-context-retention-sec`, and
  `x-bpane-browser-context-max-profile-storage-bytes`
- egress profiles: `proxy`, `bypass_rules`, `custom_ca`,
  `traffic_observation`, and proxy credential references
- workflow definitions and versions: `version`, `executor`, `entrypoint`,
  `source`, `input_schema`, `output_schema`, `default_session`,
  `allowed_credential_binding_ids`, `allowed_extension_ids`,
  `allowed_file_workspace_ids`
- workflow runs: `workflow_id`, `version`, `project_id`, `session`, `input`,
  `source_system`, `source_reference`, `client_request_id`,
  `credential_binding_ids`, and `workspace_inputs`
- session file bindings: `workspace_id`, `file_id`, `mount_path`, and `mode`
- credential bindings: `provider`, `external_ref`, `namespace`,
  `allowed_origins`, `injection_mode`, `totp`, and write-only
  `secret_payload`
- projects: `quotas` and `policy`
- service principals: `client_id`, `issuer`, `scopes`,
  `allowed_project_ids`
- identity mappings: `kind`, `issuer`, `external_id`, `claim_name`,
  `service_principal_id`, `project_id`, and `scopes`
- extension versions: `install_path`
- workspace upload helper metadata such as `x-bpane-file-provenance`

Future Phase N Workflow Endpoint fields from issue `#172` must be added to the
frozen API and this coverage matrix before UI implementation. They include:

- endpoint key, project, lifecycle state, immutable version binding, contract
  version, immutable revision, environment, compatibility status, schema
  metadata, timeout/result limits, and supported controls,
- service-principal endpoint grants and operation scopes,
- endpoint invocation idempotency, source/process/activity correlation,
  deadline, completion profile, caller limits, and bounded input,
- typed outcome, retryability, progress/heartbeat, cancellation
  acknowledgement, attempt/checkpoint and side-effect uncertainty,
  trace/request correlation, and artifact result references,
- callback contract version, cursor/replay, secret rotation, and redelivery
  controls,
- per-run event sequence, delivery replay/reconciliation evidence, Human
  Handoff ownership profile, and data-classification/retention references,
- canonical OpenAPI plus generated, tested compatibility exports; compatibility
  documents must never become independent handwritten contracts.

The endpoint control-plane contract belongs in OpenAPI. Its lifecycle callback
contract belongs in a companion AsyncAPI document rather than being inferred
from examples or UI state.

Reusable upload/import helper schemas and headers must remain covered even
when the operator UI only shows a friendly file picker. This includes file
name/provenance metadata for workspace uploads and workflow workspace input
ids used by workflow run creation.

## Parameters, Paths, Content, And Errors

Path/link-builder tests must cover reusable parameters and path templates:

- `SessionId`, `ConnectionId`, `BrowserContextId`, `SessionTemplateId`,
  `ProjectId`, `EgressProfileId`, `TaskId`, `WorkflowId`,
  `WorkflowVersion`, `RunId`, `FileId`, `RunWorkspaceInputId`,
  `RecordingId`, `WorkspaceId`, `SessionFileBindingId`, `SessionFileId`,
  `CredentialBindingId`, `ServicePrincipalId`, `IdentityMappingId`,
  `ExtensionId`, and `SubscriptionId`
- path names including `session_id`, `connection_id`, `context_id`,
  `template_id`, `project_id`, `profile_id`, `task_id`, `workflow_id`,
  `version`, `run_id`, `file_id`, `input_id`, `recording_id`,
  `workspace_id`, `binding_id`, `service_principal_id`,
  `identity_mapping_id`, `extension_id`, and `subscription_id`

Upload/download helpers must handle non-JSON routes:

- JSON APIs: `application/json`
- browser-context import/export: `application/zip`
- workspace files, session files, produced files, workflow source snapshots,
  workflow workspace inputs, and generic recording content:
  `application/octet-stream`
- recording media downloads where available: `video/webm`

Structured error rendering must preserve `ErrorResponse.category`,
`ErrorResponse.code`, and `ErrorResponse.recovery_hint`. The UI should keep
`BadRequest`, `Unauthorized`, `NotFound`, `Conflict`, `Gone`, and
`BackendUnavailable` distinct enough to show the right recovery path.

## Non-OpenAPI Compatibility Surfaces

These endpoints must be documented separately from the frozen owner-scoped API:

- web-tier `/auth-config.json`
- configured OIDC issuer discovery, authorization, token, and logout endpoints
- gateway admin-event token issuance `/api/v1/admin/events/access-tokens` and
  first-message-authenticated stream `/api/v1/admin/events`
- local `mcp-bridge` `/health`
- local `mcp-bridge` `/control-session` GET/PUT/DELETE
- MCP protocol routes `/mcp`, `/sse`, `/sessions/{session_id}/mcp`,
  `/sessions/{session_id}/sse`, `/messages`, and `/register`
- MCP selectors `Bpane-Session-Id`, `bpaneSessionId`, and
  `/messages?sessionId=...`; conflicting selectors are rejected by the bridge
- gateway legacy `/api/session/status`
- gateway legacy `/api/session/mcp-owner`
- versioned gateway `/api/v1/sessions/{session_id}/mcp-owner`
- local certificate helpers `/cert-fingerprint` and `/cert-hash`

The coverage manifest should document why each endpoint exists, whether it is
frozen, and which UI flow owns it.

## Project Route Examples

Project CRUD and usage examples in the API companion should include at least:

- `POST /api/v1/projects`
- `GET /api/v1/projects`
- `GET /api/v1/projects/{project_id}`
- `PUT /api/v1/projects/{project_id}`
- `GET /api/v1/projects/{project_id}/usage`

The corresponding admin forms should keep `ProjectResource`, `ProjectQuotas`,
`ProjectPolicy`, `ProjectUsageResource`, and `ProjectUsageAlertResource`
mapping tests aligned with those routes.

## Gateway Route Audit Anchors

When a route is migrated, compare client behavior with these implementation
boundaries:

- route assembly/helpers:
  `code/apps/bpane-gateway/src/api/router.rs`,
  `code/apps/bpane-gateway/src/api/http_helpers.rs`
- realtime admin snapshots:
  `code/apps/bpane-gateway/src/api/admin_events.rs`
- identity:
  `code/apps/bpane-gateway/src/api/service_principals.rs`,
  `code/apps/bpane-gateway/src/api/identity_mappings.rs`
- runtime/session access:
  `code/apps/bpane-gateway/src/api/runtime_access.rs`,
  `code/apps/bpane-gateway/src/api/session_bindings.rs`,
  `code/apps/bpane-gateway/src/api/sessions/crud.rs`,
  `code/apps/bpane-gateway/src/api/sessions/egress_diagnostics.rs`,
  `code/apps/bpane-gateway/src/api/sessions/egress_usage.rs`
- workflows:
  `code/apps/bpane-gateway/src/api/workflow_definitions.rs`,
  `code/apps/bpane-gateway/src/api/workflow_files/source_snapshot.rs`,
  `code/apps/bpane-gateway/src/api/workflow_run_operations.rs`,
  `code/apps/bpane-gateway/src/api/workflow_run_operations/produced_files.rs`

## API Companion Acceptance

Before API companion or coverage routes are considered complete:

1. Every OpenAPI operation has exactly one classification.
2. Every `ui-primary` and `ui-evidence` operation has a typed client wrapper
   before its route is marked done.
3. Internal-worker operations are visible only as API companion documentation or
   sanitized evidence.
4. Compatibility endpoints are listed separately from the OpenAPI contract.
5. Client wrappers have tests for path encoding, auth failure propagation,
   validation errors, content types, and secret redaction.
6. The coverage manifest is tested against `openapi/bpane-control-v1.yaml`.
7. #179 conformance and compatibility checks are visible from the route rather
   than inferred from operation counts alone.
8. Planned Workflow Endpoint or Teach Mode operations are labeled as Planned
   until their public contract is implemented and passes conformance.
