# BPANE-00156 Session Operations Subareas Plan

## Metadata

- Issue: [#156](https://github.com/ITmedes/browserpane/issues/156)
- State: Implemented and validated; pending review
- Owner: `thebackplane`
- Lane: Operator Product
- Target gate: Admin-new session operations parity
- Depends on: #155 route-backed session lifecycle, live, files, recordings, and network areas
- Branch: `feature/BPANE-00156`
- Last verified: 2026-08-07 on `feature/BPANE-00156`

## Business Outcome

Complete the route-backed admin-new session workspace with dedicated Automation,
Policy, and Observability areas. Operators should be able to answer who may
automate a session, which effective restrictions govern it, and what changed in
its runtime without searching through a single oversized inspector or relying
on the compatibility admin.

## Example Use Case

An operator investigates why a project-scoped workflow cannot upload a file to
its BrowserPane session. The Automation area confirms the assigned service
principal, MCP bridge state, and workflow runs bound to the session. The Policy
area shows that effective file transfer is disabled and identifies the project
restriction near that capability. The Observability area shows the current
connection and recording state, the latest session-scoped control-plane events,
and whether the authenticated admin event stream is live. Switching to another
session must replace all three views without retaining the first session's
delegate, policy, workflow, or event state.

## Current Evidence

- #155 established stable session subroutes and shared navigation for Overview,
  Live, Files, Recordings, and Network.
- The session resource and status API already expose lifecycle, capabilities,
  project, template, delegate, runtime, connection, recording, and MCP-owner
  facts.
- The MCP bridge client and `SessionMcpDelegationCard` already implement the
  authenticated authorize, revoke, default-session, refresh, and endpoint-copy
  behavior currently embedded in the Overview route.
- Workflow runs expose `session_id` and can be filtered client-side from the
  owner-scoped workflow-run catalog without adding a new API.
- Project resources expose the admission and policy restrictions needed to
  explain effective session behavior. A project lookup is optional for
  owner-scoped sessions and must fail visibly without hiding the session's
  effective capability facts.
- `/api/v1/admin/events/access-tokens` and `/api/v1/admin/events` provide the
  existing short-lived, initial-message-authenticated event stream. Its events
  include sessions, workflow runs, session files, recordings, MCP delegation,
  and stream errors.

## Architecture Decisions

1. Add `/admin-new/sessions/{id}/automation`, `/policy`, and `/observability`
   routes to the existing session subarea contract and navigation.
2. Move session MCP orchestration from the Overview route into one dedicated
   automation route. Reuse the existing bridge client, delegation view model,
   and card instead of maintaining a second control path.
3. Load the owner-scoped workflow-run catalog in Automation and display only
   runs whose `session_id` equals the active route session. The view is an
   association summary; full run control remains under Workflow Runs.
4. Build a pure policy view model from the session, optional live status, and
   optional project policy. Effective session capabilities remain authoritative
   for runtime access; project data explains restrictions and configured
   allowlists.
5. Extract the existing admin-event transport, access-token mapper, event
   mapper, and snapshots into the shared admin package boundary used by both
   admin applications. Do not duplicate the WebSocket authentication protocol
   in admin-new.
6. Keep a bounded, in-memory session timeline in Observability. This is an
   operator view over current snapshots, not an audit-log persistence system.
7. Keep platform-wide telemetry, durable audit export, trace correlation, and
   historical analytics in their owning follow-up work, including #178.

## Implementation Slices

### Slice 1: Automation

- Extend the session subarea route and navigation contract.
- Add a route-owned automation state model and component.
- Reuse all MCP delegation actions and surface the session endpoint consistently.
- List workflow runs associated with the selected session and link to their
  existing detail routes.
- Remove the full MCP control card from Overview after the dedicated route is
  available; retain only overview facts already represented by the session
  detail model.
- Add unit and component tests for configured/unconfigured bridge states,
  authorize/revoke/default actions, workflow filtering, failures, and session
  changes.

### Slice 2: Policy

- Add a route-owned policy loader for the session, live status, and optional
  project.
- Present effective capabilities, owner mode, recording policy, project
  restrictions, allowed resource sets, browser context, template, admission,
  and network-policy references in grouped sections.
- Place denial or unavailable-policy explanations directly beside the affected
  capability or restriction.
- Add pure view-model tests plus component tests for owner-scoped,
  project-scoped, restricted, and failed-project-lookup states.

### Slice 3: Observability

- Share the established admin-event client and wire mappers with admin-new.
- Add a session-scoped event projection and bounded timeline.
- Distinguish live status facts from event history and stream-health state.
- Surface connection counts, stop blockers, resolution, MCP ownership,
  recording indicators, workflow associations, files, and last event times.
- Handle connecting, open, reconnecting, closed, protocol error, authentication
  failure, empty history, and session-switch cleanup states.
- Add transport/mapping tests, projection tests, route component tests, and a
  browser smoke that proves live updates do not require page refresh.

### Slice 4: Integration And Documentation

- Run all focused admin-new tests and coverage.
- Run admin-new session, workflow-run, file-workspace, recording, MCP,
  browser-policy, metrics/logs, realtime-event, and reconnect smokes affected by
  the route extraction.
- Verify responsive navigation and all three routes in desktop and narrow
  browser viewports.
- Update `README.md`, architecture documentation, the roadmap, and this plan
  only where behavior or status changed.

Status: complete on `feature/BPANE-00156`.

## Validation Evidence

- Shared admin auth/event tests: 7 files, 41 tests; coverage baseline passed
  at 91.18% statements, 84.27% branches, and 97.36% functions.
- Compatibility admin: 45 files, 200 tests; type-check and production build
  passed.
- Admin-new: 130 files, 431 tests; type-check, production build, and coverage
  baseline passed at 91.03% statements, 76.50% branches, and 93.22% functions.
- `smoke:admin-unified-sessions` passed all eight route transitions, session
  switching, live event projection, MCP actions, preview, resize, and restart.
- Compatibility regression smokes passed for realtime snapshots, event-stream
  reconnect, MCP delegation, browser policy, metrics, session detail, session
  files, and recording.
- Unified regression smokes passed for recordings, file workspaces, and
  workflow runs.

## Acceptance Criteria

- The three new areas are addressable and refresh-safe through stable URLs.
- MCP controls behave exactly once from Automation and remain consistent with
  the session resource, bridge health, and per-session MCP endpoint.
- Workflow associations never include runs belonging to another session.
- Policy facts distinguish configured project restrictions from effective
  session capabilities and explain unavailable project evidence.
- Observability clearly separates current state, stream health, and bounded
  event history; it does not present snapshots as durable audit records.
- Switching routes or session ids cannot leak prior-session workflow, policy,
  MCP, metric, or event state.
- Authentication failures use the shared global logout/redirect handler.
- Existing Overview, Live, Files, Recordings, Network, and Preview behavior
  remains green.

## Post-Implementation Smoke Sequence

1. Start the local compose stack and sign in to `/admin-new/` as `demo`.
2. Create two sessions under different project policies; ensure one policy
   disables at least one capability or operation.
3. Open the first session's Automation route, authorize the MCP bridge, set it
   as default, copy its endpoint, and confirm the endpoint contains that session
   id.
4. Run or locate a workflow bound to the first session and verify that only its
   runs appear in Automation; open the linked workflow-run detail.
5. Open the second session's Automation route and verify that the first
   session's delegate, default state, and workflow associations are absent.
6. Return to the first session, clear the default, revoke authorization, and
   confirm both the route and MCP bridge health update.
7. Open each session's Policy route and compare effective capabilities, project
   restrictions, template/context, recording, admission, and network facts.
8. Confirm a denied file or recording operation is explained beside the related
   policy fact and remains enforced in the Files or Overview action surface.
9. Open Observability, confirm the stream reaches `open`, then connect a browser
   preview, delegate/revoke MCP, start a workflow, and change recording state.
10. Verify current facts and corresponding session-scoped timeline entries
    update without a full page refresh and identify their snapshot source.
11. Switch between both sessions and refresh each route; verify no stale facts
    or events cross session boundaries.
12. Stop one session, disconnect/reconnect the event stream, and verify the UI
    distinguishes current stopped state from earlier historical entries.
13. Run the automated admin-new session subarea smoke plus the compatibility MCP,
    metrics/logs, realtime-event, browser-policy, and reconnect smokes.

## Non-Goals

- No new session lifecycle, MCP, workflow-run, project-policy, recording, or
  file APIs unless an audited contract defect blocks the existing UI.
- No custom bearer-token persistence or raw OIDC token in WebSocket URLs.
- No durable audit log, full-text log search, metrics warehouse, alert engine,
  or distributed tracing backend.
- No duplication of Workflow Runs, Files, Recordings, or Network detail controls
  inside the new routes.
- No removal of the compatibility admin until its remaining migration gates are
  complete.
