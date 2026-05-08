import { fetchJson, poll } from '../workflow-smoke-lib.mjs';

export async function waitForManagedSessionClients(bridge, options, expected) {
  return await poll('MCP bridge managed sessions', () => fetchJson(healthUrl(bridge)), (health) => {
    const entries = Array.isArray(health?.managed_sessions) ? health.managed_sessions : [];
    return expected.every(([sessionId, count]) => clientCount(entries, sessionId) === count)
      && entries.every((entry) => validManagedSession(entry));
  }, options.connectTimeoutMs);
}

function validManagedSession(entry) {
  return entry?.session_id
    && Number.isInteger(entry.clients)
    && entry.backend_delegated === true
    && entry.mcp_owner === (entry.clients > 0)
    && entry.alignment === 'aligned'
    && typeof entry.cdp_endpoint === 'string'
    && typeof entry.playwright_cdp_endpoint === 'string';
}

function clientCount(entries, sessionId) {
  return entries.find((entry) => entry?.session_id === sessionId)?.clients ?? 0;
}

function healthUrl(bridge) {
  return `${new URL(bridge.controlUrl).origin}/health`;
}
