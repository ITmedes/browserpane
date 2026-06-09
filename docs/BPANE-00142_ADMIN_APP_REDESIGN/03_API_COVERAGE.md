# API Coverage

The frozen owner-scoped API contract is
`openapi/bpane-control-v1.yaml`. The new admin app must be planned against the
OpenAPI contract and the actual gateway route implementation.

## Current Audit Baseline

- OpenAPI operations: 126
- Axum `/api/v1` route-method pairs: 126
- OpenAPI component schemas: 202
- OpenAPI ref names: 231
- Reusable component parameters: 23
- Reusable component responses: 6
- Security schemes: 2
- Content types: `application/json`, `application/octet-stream`,
  `application/zip`, `video/webm`
- Schema properties audited: 1017
- Required schema entries audited: 853

## Operation Classifications

Every OpenAPI operation must be classified exactly once:

- `ui-primary`: normal operator workflow in the new app
- `ui-evidence`: read-only detail evidence in an existing resource view
- `api-companion`: documented/copyable but not first-class UI
- `internal-worker`: worker/executor/observer-owned route, not an operator
  button

Current classification counts:

- `ui-primary`: 103
- `ui-evidence`: 6
- `api-companion`: 5
- `internal-worker`: 12

## API Families To Cover

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

## Security Model

Preserve the security split:

- Owner/operator calls use `bearerAuth`.
- Worker or automation-accessible calls using `sessionAutomationAccessToken`
  must stay separated from generic operator services.
- API examples must not imply that session automation access is a broad admin
  token.

## Schema Property Parity

Generated coverage must include:

- schema name
- required properties
- optional properties
- nullable fields
- enum values
- content/media fields
- link/path fields
- timestamp fields
- nested refs

High-risk property families:

- lifecycle timestamps and state transitions
- content, export, transport, ticket, log, event, and manifest paths
- egress diagnostics proof and sensitive-log sink fields
- workflow and recording observability counters
- quotas, admission counters, and byte limits
- identity/access-review delegated-principal fields
- automation and worker artifact refs

## Non-OpenAPI Compatibility Surfaces

Keep these separate from the frozen OpenAPI contract:

- `/auth-config.json`
- configured OIDC issuer, authorization, token, and logout endpoints
- `/cert-fingerprint`
- `/cert-hash`
- `/api/session/status`
- `/api/session/mcp-owner`
- `/api/v1/sessions/{session_id}/mcp-owner`
- `/api/v1/admin/events`
- MCP bridge `/health`
- MCP bridge `/control-session`
- MCP bridge `/mcp`
- MCP bridge `/sse`
- MCP bridge `/sessions/{session_id}/mcp`
- MCP bridge `/sessions/{session_id}/sse`
- MCP bridge `/messages`
- MCP bridge `/register`
- MCP selectors `Bpane-Session-Id`, `bpaneSessionId`, and
  `/messages?sessionId=...`

## Gateway Route Audit

The new app must keep API planning aligned with:

- `code/apps/bpane-gateway/src/api/router.rs`
- `code/apps/bpane-gateway/src/api/http_helpers.rs`
- route modules under `code/apps/bpane-gateway/src/api`
- MCP bridge compatibility under `code/integrations/mcp-bridge/src`
