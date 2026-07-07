# Consolidated Validation Matrix

This matrix keeps validation tied to the current `/admin-new` development state.

## Baseline Checks For Any Unified Admin Slice

Run in `code/web/bpane-admin-unified`:

```bash
npm run check
npm test
npm run build
```

Run in `code/web/bpane-admin` when behavior overlaps with the old app:

```bash
npm run check
npm test
npm run build
```

Run in `code/web/bpane-client` for browser/client/smoke-facing changes:

```bash
npx tsc --noEmit
npm test
npm run build
```

Use `npm run test:coverage` when the slice affects browser client behavior,
CLI behavior, or test coverage needs to be demonstrated.

## Current Admin-New Smoke Coverage

Run from `code/web/bpane-client` against local compose:

```bash
npm run smoke:admin-unified-dashboard -- --headless
npm run smoke:admin-unified-projects -- --headless
npm run smoke:admin-unified-browser-contexts -- --headless
npm run smoke:admin-unified-egress-profiles -- --headless
npm run smoke:admin-unified-file-workspaces -- --headless
npm run smoke:admin-unified-sessions -- --headless --connect-timeout-ms 60000
npm run smoke:admin-unified-workflows -- --headless
npm run smoke:admin-unified-workflow-runs -- --headless
```

## Broader Existing Client Smoke Matrix

The old and new admin apps coexist, so broader browser/client smokes must remain
runnable while promotion work continues. Run the relevant subset when a slice
touches the associated behavior, and run the full set before a promotion
decision:

```bash
npm run smoke:admin-session -- --headless
npm run smoke:admin-session-detail -- --headless
npm run smoke:admin-session-files -- --headless
npm run smoke:admin-recording -- --headless
npm run smoke:admin-workflow -- --headless
npm run smoke:admin-workflow-catalog -- --headless
npm run smoke:admin-workflow-run-detail -- --headless
npm run smoke:admin-browserpane-tour -- --headless
npm run smoke:admin-egress-profiles -- --headless
npm run smoke:admin-browser-contexts -- --headless
npm run smoke:admin-file-workspaces -- --headless
npm run smoke:admin-mcp -- --headless --connect-timeout-ms 60000
npm run smoke:admin-metrics -- --headless
npm run smoke:admin-realtime -- --headless
npm run smoke:admin-event-reconnect -- --headless
npm run smoke:automation-tasks -- --headless
npm run smoke:bpane-cli -- --headless
npm run smoke:browser-policy -- --headless
npm run smoke:file-workspaces -- --headless
npm run smoke:mcp-session-endpoints -- --headless --connect-timeout-ms 60000
npm run smoke:multisession -- --headless
npm run smoke:recording -- --headless
npm run smoke:session-files -- --headless
npm run smoke:test-embed-lifecycle -- --headless
npm run smoke:test-embed-overlay -- --headless
npm run smoke:workflow-admission -- --headless
npm run smoke:workflow-cancel -- --headless
npm run smoke:workflow-cli -- --headless
npm run smoke:workflow-credential-injection -- --headless
npm run smoke:workflow-credentials -- --headless
npm run smoke:workflow-embed -- --headless
npm run smoke:workflow-embed-operations -- --headless
npm run smoke:workflow-events -- --headless
npm run smoke:workflow-extension -- --headless
npm run smoke:workflow-failure -- --headless
npm run smoke:workflow-intervention -- --headless
npm run smoke:workflow-queued-cancel -- --headless
npm run smoke:workflow-reconnect -- --headless
npm run smoke:workflow-restart-safety -- --headless
npm run smoke:workflow-runtime-hold -- --headless
npm run smoke:workflow-workspace -- --headless
npm run smoke:workflows -- --headless
npm run workflow:cli -- --help
npm run test:coverage
```

Each migrated route should cover:

- unauthenticated or expired-auth redirect/logout behavior,
- validation errors,
- missing resources,
- conflict responses,
- backend unavailable responses,
- empty lists,
- loading states,
- disabled actions,
- destructive action feedback,
- upload/download failure feedback,
- no horizontal overflow on desktop and narrow mobile viewports.

## Old Admin Regression Smokes To Keep Until Promotion

Run from `code/web/bpane-client` when the touched area overlaps old `/admin/`:

```bash
npm run smoke:admin-session -- --headless
npm run smoke:admin-session-detail -- --headless
npm run smoke:admin-session-files -- --headless
npm run smoke:admin-recording -- --headless
npm run smoke:admin-workflow -- --headless
npm run smoke:admin-workflow-catalog -- --headless
npm run smoke:admin-workflow-run-detail -- --headless
npm run smoke:admin-mcp -- --headless --connect-timeout-ms 60000
npm run smoke:admin-realtime -- --headless
npm run smoke:admin-event-reconnect -- --headless
npm run smoke:admin-metrics -- --headless
```

## MCP-Specific Validation

Required for MCP delegation/control changes:

```bash
cd code/integrations/mcp-bridge
npm test
npm run build
```

```bash
cd code/web/bpane-client
npm run smoke:admin-mcp -- --headless --connect-timeout-ms 60000
npm run smoke:mcp-session-endpoints -- --headless --connect-timeout-ms 60000
npm run smoke:bpane-cli -- --headless --connect-timeout-ms 60000
```

Manual MCP check:

1. Open `/admin-new/sessions`.
2. Connect a session.
3. Authorize MCP.
4. Copy `/sessions/{session_id}/mcp`.
5. Connect a Streamable HTTP MCP client.
6. Run `tools/list`.
7. Call `browser_navigate`.
8. Verify the selected BrowserPane preview navigates.

## Recording-Specific Validation

Required for recording lifecycle or artifact-boundary changes:

```bash
cargo test -p bpane-gateway recordings
cargo test -p bpane-gateway --test compose_api_surface compose_recording_artifacts_and_playback_api_surface -- --ignored --test-threads=1
```

```bash
cd code/integrations/recording-worker
npm run build
```

```bash
cd code/web/bpane-client
npm run smoke:recording -- --headless --connect-timeout-ms 90000
npm run smoke:admin-recording -- --headless --connect-timeout-ms 90000
```

If unified admin recordings UI changes:

```bash
cd code/web/bpane-admin-unified
npm test -- Recording
```

Then manually verify `/admin-new/recordings` can list and download the expected
artifact.

## Workflow-Specific Validation

Required for workflow source, launch, runs, events, or produced files:

```bash
cargo test -p bpane-gateway workflow
```

```bash
cd code/integrations/workflow-worker
npm run build
```

```bash
cd code/web/bpane-client
npm run smoke:admin-unified-workflows -- --headless --connect-timeout-ms 90000
npm run smoke:admin-unified-workflow-runs -- --headless --connect-timeout-ms 90000
npm run smoke:workflow-cli -- --headless
npm run smoke:workflow-failure -- --headless
npm run smoke:workflow-runtime-hold -- --headless
```

For source hardening or source browser changes, include:

```bash
npm run smoke:workflow-workspace -- --headless
npm run smoke:admin-browserpane-tour -- --headless
```

## Gateway/API Safety Checks

For backend API/security/runtime changes:

```bash
cargo test --workspace
cargo test -p bpane-gateway
cargo test -p bpane-gateway --test compose_api_surface <target_test_name> -- --ignored --test-threads=1
```

For route/API contract changes, check:

- `openapi/bpane-control-v1.yaml`
- `README.md`
- `ARCH.md`
- `AGENTS.md`

Only update those docs when behavior, topology, setup, API, or validation flow
actually changes.

## Manual Promotion Gate

Before `/admin-new` can become default:

1. Verify every visible navigation route exists or is intentionally hidden.
2. Run all current old-admin smokes for migrated behavior.
3. Run all admin-new smokes.
4. Manually test:
   - create/connect/disconnect/reconnect session,
   - preview popup resize and metrics,
   - MCP delegation and real MCP tool call,
   - recording enablement and download,
   - workflow launch and run inspection,
   - file workspace upload/download,
   - egress profile edit/probe,
   - identity/access review once implemented.
5. Keep `/admin/` as fallback until a dated removal gate is accepted.
