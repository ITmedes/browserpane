import fs from 'node:fs/promises';
import { execFile } from 'node:child_process';
import process from 'node:process';
import { promisify } from 'node:util';
import { chromium } from 'playwright-core';
import {
  cleanupAdminBeforeRun,
  closeAdminOverlay,
  disconnectEmbeddedBrowser,
  ensureAdminLoggedIn,
  getAdminAccessToken,
  openAdminTab,
  waitForBrowserConnected,
} from './admin-smoke-lib.mjs';
import { McpStreamableClient } from './support/mcp-streamable-client.mjs';
import { DEFAULTS, createLogger, fetchAuthConfig, fetchJson, launchChrome, parseSmokeArgs, poll } from './workflow-smoke-lib.mjs';

const execFileAsync = promisify(execFile);

async function run() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-mcp-session-endpoints-smoke.mjs');
  if (options.pageUrl === DEFAULTS.pageUrl) options.pageUrl = `${DEFAULTS.pageUrl}/admin/`;
  const log = createLogger('mcp-session-endpoints-smoke');
  const browser = await launchChrome(chromium, options);
  const context = await browser.newContext({ viewport: { width: 1440, height: 980 } });
  const pageA = await context.newPage();
  const pageB = await context.newPage();
  const clients = [];
  const sessions = [];
  let accessToken = '';

  try {
    log(`Opening ${options.pageUrl}`);
    await ensureAdminLoggedIn(pageA, options);
    await ensureAdminLoggedIn(pageB, options);
    await cleanupAdminBeforeRun(pageA, options, log);
    accessToken = await getAdminAccessToken(pageA);
    const bridge = await resolveMcpBridge(options);
    await clearBridgeControl(bridge);

    const sessionA = await createConnectedSession(pageA, options);
    sessions.push(sessionA);
    const sessionB = await createConnectedSession(pageB, options);
    sessions.push(sessionB);
    await delegateSession(accessToken, options, bridge, sessionA);
    await delegateSession(accessToken, options, bridge, sessionB);

    const containerA = await lookupRuntimeContainerId(sessionA);
    const containerB = await lookupRuntimeContainerId(sessionB);
    if (!containerA || !containerB) throw new Error('Expected both runtime containers to be active.');

    const clientA = await openMcpClient(bridge, sessionA, options);
    clients.push(clientA);
    const clientB = await openMcpClient(bridge, sessionB, options);
    clients.push(clientB);
    await waitForBridgeClients(bridge, options, [[sessionA, 1], [sessionB, 1]]);
    await waitForMcpOwner(accessToken, options, sessionA, true);
    await waitForMcpOwner(accessToken, options, sessionB, true);

    const markerA = await verifyNavigation(clientA, containerA, containerB, sessionA, options);
    const markerB = await verifyNavigation(clientB, containerB, containerA, sessionB, options);
    await clientA.close();
    clients.splice(clients.indexOf(clientA), 1);
    await waitForMcpOwner(accessToken, options, sessionA, false);
    await waitForMcpOwner(accessToken, options, sessionB, true);
    await waitForBridgeClients(bridge, options, [[sessionA, 0], [sessionB, 1]]);

    await emitSummary(options, { sessionA, sessionB, markerA, markerB, sessionAClosedWithoutDroppingB: true }, log);
  } finally {
    for (const client of clients.splice(0)) await client.close().catch(() => {});
    await cleanupSessions(accessToken, options, sessions);
    await disconnectEmbeddedBrowser(pageB, options).catch(() => {});
    await disconnectEmbeddedBrowser(pageA, options).catch(() => {});
    await context.close();
    await browser.close();
  }
}

async function resolveMcpBridge(options) {
  const bridge = (await fetchAuthConfig(options))?.mcpBridge;
  if (!bridge?.controlUrl || !bridge.clientId) throw new Error('Smoke requires auth-config mcpBridge metadata.');
  return bridge;
}

async function createConnectedSession(page, options) {
  await openAdminTab(page, 'sessions');
  const previous = await readSelectedSessionId(page);
  await page.getByTestId('session-new').click();
  const sessionId = await poll('new selected session', () => readSelectedSessionId(page), (id) => id && id !== previous, options.connectTimeoutMs);
  await closeAdminOverlay(page);
  await page.getByTestId('browser-connect').click();
  await waitForBrowserConnected(page, options);
  await page.locator('[data-testid="browser-viewport"] canvas').first().waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  return sessionId;
}

async function readSelectedSessionId(page) {
  const row = page.locator('[data-testid="session-row"][aria-pressed="true"]').first();
  return await row.getAttribute('data-session-id').catch(() => '') ?? '';
}

async function delegateSession(accessToken, options, bridge, sessionId) {
  await fetchJson(`${apiOrigin(options)}/api/v1/sessions/${sessionId}/automation-owner`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${accessToken}`, 'Content-Type': 'application/json' },
    body: JSON.stringify({ client_id: bridge.clientId, issuer: bridge.issuer, display_name: bridge.displayName }),
  });
}

async function openMcpClient(bridge, sessionId, options) {
  const client = new McpStreamableClient({ endpointUrl: sessionMcpUrl(bridge, sessionId), requestTimeoutMs: options.connectTimeoutMs });
  await client.initialize();
  const tools = await client.listTools();
  if (!tools.some((tool) => tool?.name === 'browser_navigate')) throw new Error('MCP bridge did not expose browser_navigate.');
  return client;
}

async function verifyNavigation(client, targetContainer, otherContainer, sessionId, options) {
  const marker = `bpane-session-endpoint-${sessionId}-${Date.now()}`;
  const url = `data:text/html,${encodeURIComponent(`<title>${marker}</title><body>${marker}</body>`)}`;
  const result = await client.callTool('browser_navigate', { url });
  if (result?.isError === true) throw new Error(`MCP browser_navigate failed: ${JSON.stringify(result)}`);
  await poll(`navigation in ${sessionId}`, () => fetchRuntimeUrls(targetContainer), (urls) => urls.some((entry) => entry.includes(marker)), options.connectTimeoutMs, 1000);
  const otherUrls = await fetchRuntimeUrls(otherContainer);
  if (otherUrls.some((entry) => entry.includes(marker))) throw new Error(`MCP navigation for ${sessionId} reached the wrong runtime.`);
  return marker;
}

async function waitForBridgeClients(bridge, options, expected) {
  await poll('MCP bridge selected clients', () => fetchJson(healthUrl(bridge)), (health) => {
    const entries = Array.isArray(health?.selected_session_clients) ? health.selected_session_clients : [];
    return expected.every(([sessionId, count]) => clientCount(entries, sessionId) === count);
  }, options.connectTimeoutMs);
}

async function waitForMcpOwner(accessToken, options, sessionId, expected) {
  await poll(`MCP owner ${sessionId}`, async () => {
    const status = await fetchJson(`${apiOrigin(options)}/api/v1/sessions/${sessionId}/status`, {
      headers: { Authorization: `Bearer ${accessToken}` },
    });
    return status.mcp_owner;
  }, (owner) => owner === expected, options.connectTimeoutMs);
}

async function lookupRuntimeContainerId(sessionId) {
  const name = `bpane-runtime-${sessionId.replaceAll('-', '')}`;
  const { stdout } = await execFileAsync('docker', ['ps', '-q', '--filter', `name=^/${name}$`]);
  return stdout.trim();
}

async function fetchRuntimeUrls(containerId) {
  const script = 'import json,sys,urllib.request\nsys.stdout.write(urllib.request.urlopen("http://127.0.0.1:9222/json/list",timeout=5).read().decode())';
  const { stdout } = await execFileAsync('docker', ['exec', containerId, 'python3', '-c', script]);
  return JSON.parse(stdout).map((target) => typeof target?.url === 'string' ? target.url : '').filter(Boolean);
}

async function cleanupSessions(accessToken, options, sessions) {
  if (!accessToken) return;
  for (const sessionId of sessions) {
    await fetch(`${apiOrigin(options)}/api/v1/sessions/${sessionId}/automation-owner`, { method: 'DELETE', headers: { Authorization: `Bearer ${accessToken}` } }).catch(() => {});
    await fetch(`${apiOrigin(options)}/api/v1/sessions/${sessionId}/kill`, { method: 'POST', headers: { Authorization: `Bearer ${accessToken}` } }).catch(() => {});
  }
}

async function clearBridgeControl(bridge) {
  const response = await fetch(bridge.controlUrl, { method: 'DELETE' });
  if (!response.ok && response.status !== 404) throw new Error(`Could not clear MCP bridge control session: HTTP ${response.status}`);
}

function clientCount(entries, sessionId) {
  return entries.find((entry) => entry?.session_id === sessionId)?.clients ?? 0;
}

function sessionMcpUrl(bridge, sessionId) {
  return `${new URL(bridge.controlUrl).origin}/sessions/${encodeURIComponent(sessionId)}/mcp`;
}

function healthUrl(bridge) {
  return `${new URL(bridge.controlUrl).origin}/health`;
}

function apiOrigin(options) {
  return new URL('/', options.pageUrl).origin;
}

async function emitSummary(options, summary, log) {
  console.log(JSON.stringify(summary, null, 2));
  if (options.outputPath) {
    await fs.writeFile(options.outputPath, `${JSON.stringify(summary, null, 2)}\n`, 'utf8');
    log(`Wrote summary to ${options.outputPath}`);
  }
}

run().catch((error) => {
  console.error(`[mcp-session-endpoints-smoke] ${error.stack || error.message}`);
  process.exitCode = 1;
});
