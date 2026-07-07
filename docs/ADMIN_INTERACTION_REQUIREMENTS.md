# Admin Interaction Requirements

This document consolidates user-facing admin interaction requirements that are
not tied to one backend resource: global feedback, panel-local messages,
session creation ergonomics, and payload preview behavior.

## Feedback Model

Operators must see state changes where they happen. Logs are diagnostic output,
not the only source of user-facing feedback.

Required global notifications:

- browser connect started,
- browser connection succeeded,
- browser connection failed,
- embedded browser disconnected,
- selected session lifecycle changed externally,
- selected session disappeared from snapshots,
- selected session live client presence materially changed,
- event stream reconnecting/error,
- event stream recovered,
- workflow run followed/auto-connected a session,
- workflow run active/awaiting-input/terminal transitions,
- selected-session runtime file count increased,
- selected-session recording started/stopped/ready/failed,
- selected-session MCP delegation or MCP ownership changed externally.

Required panel-local feedback:

- lifecycle actions,
- display preference changes,
- browser policy refresh/copy,
- metrics start/stop/copy/reset,
- file downloads/uploads,
- recording actions,
- MCP delegation actions,
- live session operations,
- workflow invoke/cancel/input submit/hold release/run refresh/produced-file
  download,
- route-level refreshes for session list/detail, file workspace list/detail,
  workflow catalog/detail, and workflow run list/detail.

Accessibility requirements:

- errors and warnings announce assertively,
- normal feedback announces politely,
- static notes and empty states should not announce as live regions,
- global messages are dismissible or replaceable,
- stale success feedback clears before showing a validation error,
- messages clear when selected session/browser connection changes so artifacts
  or recordings do not appear to belong to the wrong session.

Auth and event-stream requirements:

- admin event-stream startup failure should probe with authenticated HTTP so
  auth failures reuse the global authentication recovery path,
- event-driven automatic refreshes stay quiet unless they produce meaningful
  selected-resource diffs,
- manual refreshes report visible success or failure.

## Session Creation Configurator

Session creation must be explicit and reproducible. `New session` should not
silently create a default ready session.

Base requirements:

- use a dedicated form/route/drawer before the session exists,
- preserve one-click backend-default creation inside the configurator,
- validate before submit,
- show exact API payload preview,
- after creation, route to session detail, live/preview, or selected-session
  state depending on the operator's chosen next action,
- expose next-step links to file bindings where relevant.

Initial field requirements:

- owner mode: `collaborative` or `exclusive_browser_owner`,
- idle timeout: empty means backend default, otherwise positive integer,
- labels: parse rows or comma-separated key/value entries, reject empty keys,
  empty values, duplicates, and malformed rows,
- project,
- session template,
- browser context,
- network identity and egress profile,
- viewport/display size where explicitly set,
- extensions,
- capability checkboxes,
- recording policy,
- integration context.

Payload requirements:

- optional fields are absent when unset rather than sent as `undefined`,
- API preview stays open after expansion,
- project/template/context/egress/extension policy rejects are explained
  clearly,
- session creation should not auto-start before operator submit.

Manual creation smoke:

1. Open session creation.
2. Select `collaborative`.
3. Set idle timeout to `1800`.
4. Add labels `case=1234` and `purpose=import-repro`.
5. Verify the API preview matches the intended payload.
6. Create the session.
7. Verify the session appears in list/detail.
8. Verify owner mode, idle timeout, labels, project, template, context, egress,
   extensions, capabilities, and recording facts where exposed.
9. Repeat with invalid labels, invalid idle timeout, invalid viewport, and
   disallowed project-policy choices and verify submission is blocked before
   the API call.

Open interaction decisions:

- whether every creation entry point should navigate to a dedicated route or
  share a compact drawer,
- which post-create action should be default when a session is created from the
  live workspace versus catalog,
- whether both owner modes are covered by smoke or only by unit tests when the
  local runtime is constrained.

## Metrics And Logs

Metrics requirements:

- start/stop sample,
- copy metrics,
- reset samples,
- show throughput, tile, video, and transport summaries,
- preserve preview metrics drawer behavior until route-backed observability is
  complete.

Logs requirements:

- local admin event timeline covers auth, transport, selection, session,
  recording, workflow, MCP, and file events,
- copy and clear actions report visible feedback,
- logs remain clearly separated from persisted gateway logs unless a backend
  log API is added.
