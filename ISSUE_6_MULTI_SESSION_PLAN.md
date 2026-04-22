# Issue #6 Plan: Multi-Session BrowserPane Control Plane

## Goal

Turn BrowserPane sessions into first-class resources with:

- multiple independent host-side sessions on one host
- a public, versioned session control plane
- session-scoped browser connect semantics
- a clean split between control-plane auth and browser data-plane access
- compatibility with the current single-session dev flow during migration

## Phase -1: Auth Baseline Before Multi-Session

Before the actual multi-session control plane lands, the local stack now needs a real identity boundary instead of the old shared dev token.

Completed baseline for that pre-phase:

- `bpane-gateway` supports OIDC/JWT auth
- browser WebTransport connects with `access_token`
- gateway HTTP API uses bearer auth
- `test-embed.html` uses Authorization Code + PKCE
- local compose runs Keycloak for dev/testing
- `mcp-bridge` uses client-credentials for gateway API access

Completed baseline for the first Phase 0 control-plane slice:

- `bpane-gateway` exposes owner-scoped `POST/GET/DELETE /api/v1/sessions`
- those session resources are persisted in Postgres
- the current resource contract returns session-scoped connect metadata
- browser transport routing is now keyed by public `session_id` through short-lived connect tickets
- the actual runtime remains in `legacy_single_runtime` compatibility mode, so only one active runtime-backed session is allowed for now

Implication for issue #6:

- the control plane should assume an external IDP-compatible auth model from the start
- session resources and session-scoped access material should build on bearer-token semantics, not on the old global shared-token file flow

## Current Code Reality

The current repo already has one strong building block: `SessionHub` is the right abstraction for one logical session with multiple browser clients.

The main blockers are outside that abstraction:

- `code/apps/bpane-gateway/src/api.rs`
  - public API now has both legacy global compatibility routes and owner-scoped `/api/v1/sessions` plus session-scoped `status` / `mcp-owner`
  - formal delegation and access-ticket flows across mixed principals now exist
  - the remaining gap is real runtime selection and worker lifecycle behind those session resources
- `code/apps/bpane-gateway/src/transport.rs`
  - WebTransport now resolves a session-scoped connect ticket and routes browser connections through the public `session_id`
  - the remaining compatibility limit is that session routing still resolves to one active host runtime candidate
- `code/apps/bpane-gateway/src/runtime_manager.rs`
  - this seam now exists and currently supports:
    - `static_single`: one shared host socket
    - `docker_single`: one start-on-demand runtime container with idle shutdown
  - both current backends still enforce one active runtime at a time
  - the remaining work is replacing those single-runtime backends with real per-session runtime assignment, configurable capacity limits, and runtime metadata persistence
- `code/apps/bpane-gateway/src/main.rs` and `config.rs`
  - one `--agent-socket`, one host endpoint
- `code/integrations/mcp-bridge/src/index.ts`
  - can now resolve a control session and use session-scoped ownership/status APIs
  - still assumes one BrowserPane runtime endpoint and one CDP target underneath
  - still lacks a first-class cross-principal delegation flow
- `deploy/compose.yml`
  - local stack is still hard-wired to one host worker and one socket volume by default
  - the opt-in Docker backend is not wired into the default compose path yet

`SessionRegistry` is now keyed by public logical session ID inside the gateway. The remaining multi-session gap is no longer gateway identity/routing; it is host/runtime lifecycle, runtime capacity policy, and mapping multiple runtime workers behind those session IDs through `runtime_manager.rs`.

## Industry Patterns Worth Copying

### 1. Session as a first-class resource

Common pattern across Browserbase, Browserless, and Steel:

- create session via REST
- get/list session via REST
- receive a session ID plus session-scoped connect material
- manage lifecycle explicitly instead of treating a session as an accidental side effect of one global browser

BrowserPane should adopt this directly.

### 2. Separate control plane from connect plane

Common pattern:

- control plane creates and manages sessions
- browser/automation clients connect using session-scoped connect URLs or tickets
- the connect material is short-lived and scoped to one session

BrowserPane should not expose one global `gatewayUrl + token` as the main documented contract anymore.

### 3. Explicit lifecycle states

Common pattern:

- sessions have visible lifecycle states
- sessions time out, fail, reconnect, or are explicitly released
- status and observability are queryable per session

BrowserPane should formalize session states early so runtime refactors do not redefine API semantics later.

### 4. Metadata, labels, and integration context

Common pattern:

- sessions carry user-defined metadata for filtering, ownership, and workflow tracking
- public API supports querying or at least returning those labels later

BrowserPane should support lightweight labels and integration context in the session resource from the start.

### 5. Embedded live view / human-in-the-loop stays session-scoped

Common pattern:

- live view/debugging is attached to a specific running session
- multitab and viewer/embed behavior are session-local

BrowserPane already has the shared-session side of this. The missing part is making session selection explicit at the control-plane boundary.

### 6. Delegation across principals is explicit

Common pattern:

- one actor creates or owns a session
- another actor is explicitly allowed to attach, observe, or automate that session
- the attach path is deliberate, scoped, and revocable

BrowserPane needs this because its browser user and `mcp-bridge` service principal are currently different identities.

### 7. Reconnect / resume is expected

Common pattern:

- session connections can drop and reconnect
- the session resource remains the stable handle
- reconnect and explicit release are normal parts of the lifecycle

BrowserPane should plan for reconnect-friendly attach semantics early, even before persistent cross-worker resurrection exists.

### 8. Recordings / artifacts / replay are normal companion features

Common pattern:

- recordings, live views, and artifacts are attached to a session resource
- these are not necessarily phase-1 requirements, but the session model should leave space for them

BrowserPane should keep recordings out of the immediate critical path, but the session resource should not paint us into a corner.

## Industry Patterns We Should Not Copy Directly

### 1. Account/project/billing model as part of the first rollout

Browserbase and similar platforms expose project/account-level concepts because they are commercial hosted platforms.

BrowserPane should not block issue #6 on:

- project/account hierarchy
- billing constructs
- organization-level quota management

Those can stay out of scope for the first multi-session control plane.

### 2. Persistent profile reuse across disconnected sessions as a first requirement

Browserless leans heavily on persistent session state and reconnect workflows. BrowserPane should support reconnect-friendly connect tickets, but the first milestone should focus on isolated parallel sessions, not long-lived session resurrection across arbitrary worker restarts.

### 3. Large hosted-platform feature surface

Do not make issue #6 depend on:

- recording as a prerequisite
- global dashboards
- broad analytics UI
- arbitrary multi-region scheduling

The first win is a stable session resource model and session-scoped routing.

## Recommended BrowserPane Target Model

### Public session resource

Expose a session resource with at least:

- `id`
- `state`
- `template_id`
- `owner_mode`
- `viewport`
- `capabilities`
- `created_at`
- `expires_at`
- `idle_timeout_sec`
- `labels`
- `integration_context`
- `telemetry`

Recommended initial states:

- `pending`
- `starting`
- `ready`
- `active`
- `idle`
- `stopping`
- `stopped`
- `failed`
- `expired`

### Public connect contract

Make the public browser client contract:

- `connectUrl`
- `accessToken` or short-lived connect ticket

Keep the current `gatewayUrl + token` only as a temporary compatibility path.

### Public API shape

Start with `/api/v1` and a small stable core:

- `POST /api/v1/sessions`
- `GET /api/v1/sessions/{id}`
- `DELETE /api/v1/sessions/{id}`
- `GET /api/v1/sessions/{id}/status`
- `POST /api/v1/sessions/{id}/access-tokens`
- `POST /api/v1/sessions/{id}/automation-owner`
- `DELETE /api/v1/sessions/{id}/automation-owner`

For BrowserPane specifically, the next practical addition after the current Phase 0 slice should be explicit delegation or attach semantics for mixed principals, not more global compatibility endpoints.

`GET /api/v1/sessions` should be included early even if filtering is basic at first, because session listing is one of the most standard and integration-friendly control-plane expectations.

### Internal boundary

Keep the public boundary in the gateway.

Recommended split:

- public API: gateway
- internal session lifecycle API: gateway <-> host session manager
- per-session runtime worker: `bpane-host`

Do not expose host-manager internals as the product API.

## Phased Implementation

### Phase 0: Freeze the public contract

Deliverables:

- OpenAPI draft for `/api/v1`
- Rust and TypeScript types for session resources and access-token responses
- session state machine definition
- auth model definition
- compatibility story for current single-session mode

Current status:

- the gateway now has the first versioned resource model and owner-scoped storage
- the implemented Phase 0 API is:
  - `POST /api/v1/sessions`
  - `GET /api/v1/sessions`
  - `GET /api/v1/sessions/{id}`
  - `DELETE /api/v1/sessions/{id}`
- Phase 0 also now includes session-scoped compatibility routes:
  - `POST /api/v1/sessions/{id}/access-tokens`
  - `POST /api/v1/sessions/{id}/automation-owner`
  - `DELETE /api/v1/sessions/{id}/automation-owner`
  - `GET /api/v1/sessions/{id}/status`
  - `POST /api/v1/sessions/{id}/mcp-owner`
  - `DELETE /api/v1/sessions/{id}/mcp-owner`
- persistence is Postgres-backed in the normal compose/runtime path
- `test-embed.html` already consumes that API and resolves/creates a session resource before browser connect
- browser transport now uses a short-lived session-scoped connect ticket minted from `/api/v1/sessions/{id}/access-tokens`
- `test-embed.html` can now explicitly delegate the current session to the local `bpane-mcp-bridge` principal and assign that same session through `mcp-bridge`'s local `/control-session` API
- `mcp-bridge` now has the first session-control client hooks and can use session-scoped ownership APIs for an explicit managed session without relying on implicit bootstrap
- the remaining Phase 0 gap is tightening the formal contract surface and expanding downstream integration to consume these resources instead of the older implicit single-session assumptions

Exit criteria:

- no new single-session-only public API lands after this point
- client and integration work can target stable shapes instead of inferred behavior

### Phase 1: Build an internal host session manager

Deliverables:

- new internal host-side manager process or crate
- per-session runtime allocation:
  - display
  - socket path
  - Chromium profile
  - upload/download dirs
  - temp/log dirs
  - CDP port allocation
- startup, readiness, stop, timeout, and cleanup flow
- keep `bpane-host` as the per-session worker

Exit criteria:

- two independent session workers can run in parallel on one host
- stopping one session does not disturb the other

### Phase 2: Make gateway routing session-scoped

Deliverables:

- session-scoped WebTransport request parsing
- registry keyed by logical session ID
- session-scoped status and ownership APIs
- gateway lookup from session ID to host manager runtime endpoint

Exit criteria:

- completed in the current branch for the single-runtime compatibility model
- browser clients for session A cannot attach to session B accidentally
- remaining follow-up: replace the legacy single-runtime lookup with true per-session host runtime resolution

### Phase 3: Ship the public control plane

Deliverables:

- `/api/v1/sessions` implementation
- session-scoped access-ticket minting
- control-plane auth middleware
- normalized authorization claims
- audit logging for create/connect/ownership/destroy

Exit criteria:

- an external backend can create, inspect, connect to, and destroy sessions using documented APIs only

### Phase 4: Update browser client and MCP bridge

Deliverables:

- `bpane-client` supports `connectUrl + accessToken`
- compatibility path for current callers remains temporarily
- `mcp-bridge` becomes session-aware:
  - session ID required
  - session-scoped ownership calls
  - per-session CDP endpoint resolution
  - explicit delegated attach model for mixed browser-user and service-account principals

Exit criteria:

- one automation-owned session and one human-owned session can run in parallel
- `mcp-bridge` no longer assumes a single global BrowserPane session

### Phase 5: Validation and documentation

Deliverables:

- automated multi-session validation
- API auth validation
- owner/viewer behavior validation across multiple sessions
- operational docs
- integration examples for ticket exchange and auth modes

Exit criteria:

- API docs match runtime behavior
- multi-session behavior is covered by automation, not only manual testing

## Test Integration Plan

### Current test reality

What exists now:

- gateway Rust tests under `code/apps/bpane-gateway/src/**/tests.rs`
- host Rust tests under `code/apps/bpane-host/src/**/tests.rs`
- protocol tests under `code/shared/bpane-protocol/tests`
- browser client unit/integration tests under `code/web/bpane-client/js/__tests__`

What does not really exist yet:

- a dedicated repo-level multi-session end-to-end harness
- `mcp-bridge` automated tests

That means issue #6 should extend the existing subsystem test seams first, then add a focused multi-session integration layer.

### Test layer 1: contract and type tests

Add early:

- protocol/resource serialization tests for session resources and token responses
- gateway request/response schema tests
- browser client tests for `connectUrl + accessToken` parsing and fallback compatibility

### Test layer 2: host-manager tests

Add in the new session-manager subsystem:

- resource allocation tests
- collision avoidance tests for display/ports/socket paths
- cleanup tests
- timeout and abandoned-session GC tests
- concurrent start/stop tests

### Test layer 3: gateway session-routing integration tests

Add gateway-focused integration coverage for:

- session ID -> runtime resolution
- session-scoped access control
- session-scoped status APIs
- ticket minting and expiry behavior
- preventing cross-session attach

These can start with a fake or test session-manager backend before requiring real Chromium workers.

### Test layer 4: browser client integration tests

Extend `code/web/bpane-client/js/__tests__` for:

- new connect contract
- capability updates on session-scoped connect
- compatibility mode with current `gatewayUrl + token`

Do not start with browser E2E for these changes; keep the first client validation at the API/transport seam.

### Test layer 5: MCP bridge tests

Issue #6 should add an actual test story for `mcp-bridge`.

Recommended first step:

- add a Node test runner there
- extract gateway API interactions behind a thin client module
- test:
  - session-scoped owner registration
  - session-scoped owner release
  - per-session status polling
  - missing/invalid session handling

### Test layer 6: compose-driven multi-session smoke test

After Phases 2-4 are stable, add one compose-driven smoke path that proves:

- create session A
- create session B
- connect browser client to A
- connect browser client to B
- verify isolation
- claim automation owner on one session without affecting the other

This does not need to start as a full browser-heavy E2E suite. A narrow orchestrated smoke harness is enough initially.

## Recommended First Implementation Slice

Start with Phase 0 only.

Concrete first PR target:

1. Add `/api/v1` resource shapes and OpenAPI draft.
2. Define session IDs and access-ticket response types in Rust and TypeScript.
3. Introduce session-aware gateway route parsing without switching runtime routing yet.
4. Add tests for the new public contract and compatibility behavior.

This is the lowest-risk way to start because it freezes the product boundary before runtime refactors begin.

## MCP / Automation Use Cases To Design For

These are the MCP-adjacent use cases that match the way modern browser-session platforms are typically used:

1. Human-supervised automation on one live session
   - human watches or intervenes while automation drives
2. Attach automation to an existing user-owned session by explicit delegation
   - this is the most important missing capability for BrowserPane right now
3. Create a session for automation first, then let humans join later
4. Temporary ownership handoff between human and automation
5. Reconnect or resume the same session after disconnect
6. Embed a live session view inside another product or dashboard
7. Query or resolve sessions by metadata, labels, or integration context
8. Record or replay a session later for debugging, support, or audit
9. Reuse auth or context where the product needs session continuity
10. Run multiple isolated automation sessions in parallel

What this implies for BrowserPane:

- the next important control-plane feature is explicit delegation, not more implicit bootstrap
- reconnect-friendly semantics matter early
- metadata and integration context should remain first-class
- recordings should stay out of the critical path for now, but the session model should leave room for them

## Decision Summary

What BrowserPane should copy now:

- first-class session resource
- REST create/get/list/delete lifecycle
- session-scoped connect material
- explicit lifecycle states
- metadata/labels
- separate control-plane auth from browser connect auth

What BrowserPane should postpone:

- project/account hierarchy
- hosted-platform billing concepts
- long-lived persistent session resurrection as a first requirement
- broad dashboard/UI work
- recording as a dependency for the control plane

## Suggested Success Criteria For Issue #6

Issue #6 is done when:

- sessions are first-class public resources
- gateway routing is session-scoped
- the host can run at least two isolated sessions in parallel
- `bpane-client` and `mcp-bridge` can target one explicit session
- auth is split cleanly between control-plane and data-plane access
- the new flow is covered by automated tests, not just manual verification

## External Reference Points

The design choices above were cross-checked against current public session-control patterns in:

- Browserbase: create/get/list sessions, connect URLs, keep-alive, metadata, observability, and live view
- Browserless: session lifecycle URLs, reconnect/persistent state, and session listing
- Steel: sessions as the atomic browser unit, explicit lifecycle states, and embeddable live sessions
