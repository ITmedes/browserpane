export const UNIFIED_ADMIN_PROMOTION_SMOKES = Object.freeze([
  smoke('compose-admin-new-promotion', 'Smoke admin-new promotion and fallback routing',
    'smoke:admin-unified-promotion', ['--connect-timeout-ms', '60000']),
  smoke('compose-admin-new-dashboard', 'Smoke admin-new dashboard',
    'smoke:admin-unified-dashboard'),
  smoke('compose-admin-new-projects', 'Smoke admin-new projects',
    'smoke:admin-unified-projects'),
  smoke('compose-admin-new-browser-contexts', 'Smoke admin-new browser contexts',
    'smoke:admin-unified-browser-contexts'),
  smoke('compose-admin-new-egress-profiles', 'Smoke admin-new egress profiles',
    'smoke:admin-unified-egress-profiles'),
  smoke('compose-admin-new-file-workspaces', 'Smoke admin-new file workspaces',
    'smoke:admin-unified-file-workspaces'),
  smoke('compose-admin-new-recordings', 'Smoke admin-new recordings',
    'smoke:admin-unified-recordings'),
  smoke('compose-admin-new-workflows', 'Smoke admin-new workflows',
    'smoke:admin-unified-workflows'),
  smoke('compose-admin-new-workflow-runs', 'Smoke admin-new workflow runs',
    'smoke:admin-unified-workflow-runs'),
  smoke('compose-admin-new-identity', 'Smoke admin-new identity and access',
    'smoke:admin-unified-identity'),
  smoke('compose-admin-new-resource-catalogs',
    'Smoke admin-new extension, credential, and event catalogs',
    'smoke:admin-unified-resource-catalogs', ['--connect-timeout-ms', '60000']),
  smoke('compose-admin-new-sessions', 'Smoke admin-new sessions',
    'smoke:admin-unified-sessions', ['--connect-timeout-ms', '60000']),
  smoke('compose-admin-new-api-companion', 'Smoke admin-new API companion',
    'smoke:admin-unified-api-companion', ['--connect-timeout-ms', '60000']),
]);

export const COMPATIBILITY_ADMIN_PROMOTION_SMOKES = Object.freeze([
  smoke('compose-admin-auth-security', 'Smoke shared admin authentication',
    'smoke:admin-auth-security'),
  smoke('compose-admin-compat', 'Smoke compatibility admin session',
    'smoke:admin-session'),
  smoke('compose-admin-compat-realtime', 'Smoke compatibility admin realtime updates',
    'smoke:admin-realtime'),
  smoke('compose-admin-compat-event-reconnect',
    'Smoke compatibility admin event reconnect', 'smoke:admin-event-reconnect'),
  smoke('compose-admin-compat-browser-contexts',
    'Smoke compatibility admin browser contexts', 'smoke:admin-browser-contexts'),
  smoke('compose-admin-compat-egress-profiles',
    'Smoke compatibility admin egress profiles', 'smoke:admin-egress-profiles'),
  smoke('compose-admin-compat-session-files',
    'Smoke compatibility admin session files', 'smoke:admin-session-files'),
  smoke('compose-admin-compat-mcp', 'Smoke compatibility admin MCP delegation',
    'smoke:admin-mcp'),
  smoke('compose-admin-compat-recording', 'Smoke compatibility admin recording',
    'smoke:admin-recording'),
  smoke('compose-admin-compat-workflow', 'Smoke compatibility admin workflow execution',
    'smoke:admin-workflow'),
  smoke('compose-admin-compat-workflow-run-detail',
    'Smoke compatibility admin workflow run detail', 'smoke:admin-workflow-run-detail'),
]);

export const ADMIN_PROMOTION_SMOKES = Object.freeze([
  ...UNIFIED_ADMIN_PROMOTION_SMOKES,
  ...COMPATIBILITY_ADMIN_PROMOTION_SMOKES,
]);

function smoke(id, description, script, extraArgs = []) {
  return Object.freeze({ id, description, script, extraArgs: Object.freeze(extraArgs) });
}
