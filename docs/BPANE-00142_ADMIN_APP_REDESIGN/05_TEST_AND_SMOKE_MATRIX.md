# Test And Smoke Matrix

## Current Admin Baseline

Run before implementation starts:

```sh
cd code/web/bpane-admin && npm test
cd code/web/bpane-admin && npm run check
cd code/web/bpane-client && npm run smoke:admin-session -- --headless
```

## New App Package Checks

Run for `code/web/bpane-admin-unified` after scaffolding:

```sh
npm test
npm run check
npm run build
```

## Current Client Smoke Coverage

The redesign must keep these runnable while `/admin/` and `/admin-new/`
coexist:

- `smoke:admin-session`
- `smoke:admin-session-detail`
- `smoke:admin-session-files`
- `smoke:admin-recording`
- `smoke:admin-workflow`
- `smoke:admin-workflow-catalog`
- `smoke:admin-workflow-run-detail`
- `smoke:admin-browserpane-tour`
- `smoke:admin-egress-profiles`
- `smoke:admin-unified-browser-contexts`
- `smoke:admin-unified-egress-profiles`
- `smoke:admin-unified-file-workspaces`
- `smoke:admin-browser-contexts`
- `smoke:admin-file-workspaces`
- `smoke:admin-mcp`
- `smoke:admin-metrics`
- `smoke:admin-realtime`
- `smoke:admin-event-reconnect`
- `smoke:automation-tasks`
- `smoke:bpane-cli`
- `smoke:browser-policy`
- `smoke:file-workspaces`
- `smoke:mcp-session-endpoints`
- `smoke:multisession`
- `smoke:recording`
- `smoke:session-files`
- `smoke:test-embed-lifecycle`
- `smoke:test-embed-overlay`
- `smoke:workflow-admission`
- `smoke:workflow-cancel`
- `smoke:workflow-cli`
- `smoke:workflow-credential-injection`
- `smoke:workflow-credentials`
- `smoke:workflow-embed`
- `smoke:workflow-embed-operations`
- `smoke:workflow-events`
- `smoke:workflow-extension`
- `smoke:workflow-failure`
- `smoke:workflow-intervention`
- `smoke:workflow-queued-cancel`
- `smoke:workflow-reconnect`
- `smoke:workflow-restart-safety`
- `smoke:workflow-runtime-hold`
- `smoke:workflow-workspace`
- `smoke:workflows`
- `test:coverage`
- `workflow:cli -- --help`
- `build`

## New App Smoke Direction

Add `/admin-new/` smokes progressively:

1. shell/deep-link smoke
2. dashboard smoke
3. sessions catalog smoke
4. session detail smoke
5. live attach smoke
6. session files smoke
7. recordings smoke
8. network/egress diagnostics smoke
   - `smoke:admin-unified-egress-profiles` covers the unified egress catalog,
     create route, validation errors, detail/update flow, disabled state, and
     edit-form overflow checks.
9. automation/MCP smoke
10. browser policy smoke
11. observability/logs smoke
12. create session configurator smoke
13. resource catalog smokes
    - `smoke:admin-unified-browser-contexts` covers the unified browser-context
      catalog, create route, validation errors, detail refresh/delete flow,
      project binding, and mobile overflow checks.
    - `smoke:admin-unified-file-workspaces` covers the unified file-workspace
      catalog, create route, validation errors, project binding, upload,
      download, delete, catalog search, and mobile overflow checks.
14. API companion smoke

## Error Cases

Every migrated route should cover:

- unauthenticated/expired auth redirect or logout handling
- validation errors
- missing resource
- conflict responses
- backend unavailable
- empty lists
- loading state
- disabled actions
- destructive action feedback
- download/upload failure feedback
