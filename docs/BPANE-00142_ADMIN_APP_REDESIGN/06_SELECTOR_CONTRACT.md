# Selector Contract

The current admin smoke tests depend on stable DOM selectors. The new app may
rename selectors only when the related smoke is updated in the same slice.

## Generated Selector Manifest

Before completing a route migration, generate a selector manifest from:

- `code/web/bpane-admin/src`
- impacted `code/web/bpane-client/scripts/*.mjs`

Include:

- `data-testid`
- `data-action`
- `aria-label`
- route paths
- key DOM selectors used by Playwright scripts

Current audit baseline:

- 319 current source `data-testid` values
- 1 current source `data-action` value
- 10 smoke `data-testid` values
- 2 smoke `data-action` values

## High-Risk Selectors

Keep these especially stable or update smokes in the same slice:

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

Broader non-admin fixture smoke selectors must also remain covered where
impacted:

- `download-workflow-file`

## Selector Policy

- Prefer semantic selectors tied to product behavior, not layout structure.
- Do not make smokes depend on cosmetic wrappers.
- Keep route IDs, row IDs, and selected-state attributes explicit.
- If a selector is intentionally removed, update the smoke and the manifest in
  the same slice.
