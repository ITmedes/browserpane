import fs from 'node:fs/promises';
import process from 'node:process';
import { chromium } from 'playwright-core';
import {
  cleanupAdminBeforeRun,
  cleanupAdminSmoke,
  closeAdminOverlay,
  disconnectEmbeddedBrowser,
  ensureAdminLoggedIn,
  getAdminAccessToken,
  openAdminTab,
  waitForBrowserConnected,
} from './admin-smoke-lib.mjs';
import { DEFAULTS, createLogger, fetchAuthConfig, fetchJson, launchChrome, parseSmokeArgs, poll } from './workflow-smoke-lib.mjs';

async function run() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-admin-mcp-smoke.mjs');
  if (options.pageUrl === DEFAULTS.pageUrl) options.pageUrl = `${DEFAULTS.pageUrl}/admin/`;
  const log = createLogger('admin-mcp-smoke');
  const browser = await launchChrome(chromium, options);
  const context = await browser.newContext({ viewport: { width: 1440, height: 980 } });
  const page = await context.newPage();
  let sessionA = '';
  let sessionB = '';

  try {
    log(`Opening ${options.pageUrl}`);
    await ensureAdminLoggedIn(page, options);
    await cleanupAdminBeforeRun(page, options, log);
    const bridge = await resolveMcpBridge(options);
    await clearBridgeControl(bridge);
    await waitForBridgeControl(bridge, null, options);
    const accessToken = await getAdminAccessToken(page);

    log('Creating and delegating first session through the admin MCP panel.');
    sessionA = await createAndConnectSession(page, options);
    log(`Delegating first session ${sessionA}.`);
    await delegateSelectedSession(page, options, bridge, sessionA);

    log('Switching delegation to a second connected session.');
    await disconnectEmbeddedBrowser(page, options);
    sessionB = await createAndConnectSession(page, options);
    log(`Delegating second session ${sessionB}.`);
    await delegateSelectedSession(page, options, bridge, sessionB);
    await assertSessionDelegate(accessToken, options, sessionA, null);
    await assertSessionDelegate(accessToken, options, sessionB, bridge.clientId);

    log('Clearing MCP bridge delegation through the admin panel.');
    await page.getByTestId('mcp-clear').click();
    await waitForMcpStatus(page, options, 'No delegated session');
    await waitForBridgeControl(bridge, null, options);
    await assertSessionDelegate(accessToken, options, sessionB, null);
    await emitSummary(options, { sessionA, sessionB, uiDelegateSwitch: true, uiClear: true }, log);
  } finally {
    await clearBridgeFromPage(page).catch(() => {});
    await cleanupAdminSmoke(page, options, log);
    await context.close();
    await browser.close();
  }
}

async function resolveMcpBridge(options) {
  const config = await fetchAuthConfig(options);
  const bridge = config?.mcpBridge;
  if (!bridge?.controlUrl || !bridge.clientId) {
    throw new Error('Admin MCP smoke requires auth-config mcpBridge metadata.');
  }
  return bridge;
}

async function createAndConnectSession(page, options) {
  await openAdminTab(page, 'sessions');
  const previousSessionId = await readSelectedSessionId(page);
  await page.getByTestId('session-new').click();
  const sessionId = await resolveSelectedSessionId(page, options, previousSessionId);
  await closeAdminOverlay(page);
  await page.getByTestId('browser-connect').click();
  await waitForBrowserConnected(page, options);
  await page.locator('[data-testid="browser-viewport"] canvas').first().waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await openAdminTab(page, 'sessions');
  return sessionId;
}

async function delegateSelectedSession(page, options, bridge, sessionId) {
  await waitForEnabled(page.getByTestId('mcp-delegate'), options, 'admin MCP delegate');
  await page.getByTestId('mcp-delegate').click();
  await waitForMcpStatus(page, options, 'This session delegated');
  const health = await waitForBridgeControl(bridge, sessionId, options);
  if (health.bridge_alignment !== 'aligned') {
    throw new Error(`Expected aligned bridge health, got ${health.bridge_alignment}`);
  }
}

async function waitForBridgeControl(bridge, sessionId, options) {
  return await poll('MCP bridge control session', async () => {
    try {
      return await fetchJson(healthUrl(bridge));
    } catch {
      return null;
    }
  }, (health) => (health?.control_session_id ?? null) === sessionId, options.connectTimeoutMs);
}

async function assertSessionDelegate(accessToken, options, sessionId, clientId) {
  const session = await fetchJson(`${apiOrigin(options)}/api/v1/sessions/${sessionId}`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
  const actual = session.automation_delegate?.client_id ?? null;
  if (actual !== clientId) {
    throw new Error(`Expected ${sessionId} delegate ${clientId ?? 'none'}, got ${actual ?? 'none'}.`);
  }
}

async function clearBridgeFromPage(page) {
  if (await page.getByTestId('mcp-clear').isEnabled().catch(() => false)) {
    await page.getByTestId('mcp-clear').click();
  }
}

async function clearBridgeControl(bridge) {
  const response = await fetch(bridge.controlUrl, { method: 'DELETE' });
  if (!response.ok) {
    const detail = await response.text().catch(() => '');
    throw new Error(`Could not clear MCP bridge control session: HTTP ${response.status}${detail ? ` ${detail}` : ''}`);
  }
}

async function waitForMcpStatus(page, options, status) {
  await poll('admin MCP status', async () => {
    return await page.getByTestId('mcp-status').textContent().catch(() => '');
  }, (value) => value === status, options.connectTimeoutMs);
}

async function resolveSelectedSessionId(page, options, previousSessionId) {
  return await poll('new admin selected session', async () => {
    return await readSelectedSessionId(page);
  }, (sessionId) => Boolean(sessionId && sessionId !== previousSessionId), options.connectTimeoutMs);
}

async function readSelectedSessionId(page) {
  const row = page.locator('[data-testid="session-row"][aria-pressed="true"]').first();
  if (!await row.isVisible().catch(() => false)) {
    return '';
  }
  return await row.getAttribute('data-session-id') ?? '';
}

async function waitForEnabled(locator, options, description) {
  await poll(description, async () => await locator.isEnabled(), Boolean, options.connectTimeoutMs);
}

async function emitSummary(options, summary, log) {
  console.log(JSON.stringify(summary, null, 2));
  if (options.outputPath) {
    await fs.writeFile(options.outputPath, `${JSON.stringify(summary, null, 2)}\n`, 'utf8');
    log(`Wrote summary to ${options.outputPath}`);
  }
}

function healthUrl(bridge) {
  const url = new URL(bridge.controlUrl);
  url.pathname = '/health';
  url.search = '';
  return url.toString();
}

function apiOrigin(options) {
  return new URL('/', options.pageUrl).origin;
}

run().catch((error) => {
  console.error(`[admin-mcp-smoke] ${error.stack || error.message}`);
  process.exitCode = 1;
});
