import fs from 'node:fs/promises';
import process from 'node:process';
import { chromium } from 'playwright-core';
import {
  cleanupAdminBeforeRun,
  cleanupAdminSmoke,
  disconnectEmbeddedBrowser,
  ensureAdminLoggedIn,
  getAdminAccessToken,
  joinSelectedSession,
  openAdminTab,
} from './admin-smoke-lib.mjs';
import { resolveNewSessionId } from './new-session-selection.mjs';
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
	    const accessToken = await getAdminAccessToken(page);
	    const bridge = await resolveMcpBridge(options);
	    await clearBridgeControl(bridge, accessToken);
	    await waitForBridgeControl(bridge, null, options);

    log('Creating and authorizing first session through the admin MCP panel.');
    sessionA = await createAndConnectSession(page, options);
    await assertMcpEndpoint(page, options, bridge, sessionA);
    await clickMcpAction(page, options, 'mcp-delegate', 'Authorized');
    await assertSessionDelegate(accessToken, options, sessionA, bridge.clientId);
    await clickMcpAction(page, options, 'mcp-set-default', 'Authorized default');
    await waitForBridgeControl(bridge, sessionA, options);

    log('Authorizing a second connected session without clearing the first.');
    await disconnectEmbeddedBrowser(page, options);
    await ensureAdminLoggedIn(page, options);
    sessionB = await createAndConnectSession(page, options);
    await assertMcpEndpoint(page, options, bridge, sessionB);
    await clickMcpAction(page, options, 'mcp-delegate', 'Authorized');
    await waitForBridgeControl(bridge, sessionA, options);
    await assertSessionDelegate(accessToken, options, sessionA, bridge.clientId);
    await assertSessionDelegate(accessToken, options, sessionB, bridge.clientId);

    log('Moving only the default MCP session, then clearing it.');
    await clickMcpAction(page, options, 'mcp-set-default', 'Authorized default');
    await waitForBridgeControl(bridge, sessionB, options);
    await assertSessionDelegate(accessToken, options, sessionA, bridge.clientId);
    await clickMcpAction(page, options, 'mcp-clear', 'Authorized');
    await waitForBridgeControl(bridge, null, options);
    await assertSessionDelegate(accessToken, options, sessionB, bridge.clientId);
    await clickMcpAction(page, options, 'mcp-revoke', 'Not authorized');
    await assertSessionDelegate(accessToken, options, sessionB, null);
    await selectSession(page, options, sessionA);
    await clickMcpAction(page, options, 'mcp-revoke', 'Not authorized');
    await assertSessionDelegate(accessToken, options, sessionA, null);
    await emitSummary(options, { sessionA, sessionB, multiAuthorize: true, defaultSwitch: true }, log);
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
  await waitForEnabled(page.getByTestId('session-new'), options, 'admin new session');
  const previousSessionIds = await readVisibleSessionIds(page);
  await page.getByTestId('session-new').click();
  const sessionId = await resolveCreatedSessionId(page, options, previousSessionIds);
  await selectSession(page, options, sessionId);
  await joinSelectedSession(page, options);
  await page.locator('[data-testid="browser-viewport"] canvas').first().waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await openAdminTab(page, 'sessions');
  return sessionId;
}

async function clickMcpAction(page, options, testId, status) {
  await waitForEnabled(page.getByTestId(testId), options, `admin ${testId}`);
  await page.getByTestId(testId).click();
  await waitForMcpStatus(page, options, status);
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

async function assertMcpEndpoint(page, options, bridge, sessionId) {
  const expected = sessionMcpUrl(bridge, sessionId);
  await poll('admin MCP endpoint', async () => ({
    text: await page.getByTestId('mcp-endpoint-url').textContent().catch(() => ''),
    enabled: await page.getByTestId('mcp-copy-endpoint').isEnabled().catch(() => false),
  }), (state) => state.text === expected && state.enabled, options.connectTimeoutMs);
}

async function clearBridgeFromPage(page) {
  if (await page.getByTestId('mcp-clear').isEnabled().catch(() => false)) {
    await page.getByTestId('mcp-clear').click();
  }
  if (await page.getByTestId('mcp-revoke').isEnabled().catch(() => false)) {
    await page.getByTestId('mcp-revoke').click();
  }
}

async function clearBridgeControl(bridge, accessToken) {
  const response = await fetch(bridge.controlUrl, {
    method: 'DELETE',
    headers: { Authorization: `Bearer ${accessToken}` },
  });
  if (!response.ok) {
    const detail = await response.text().catch(() => '');
    throw new Error(`Could not clear MCP bridge control session: HTTP ${response.status}${detail ? ` ${detail}` : ''}`);
  }
}

async function waitForMcpStatus(page, options, status) {
  let latest = '';
  try {
    await poll('admin MCP status', async () => {
      latest = await page.getByTestId('mcp-status').textContent().catch(() => '');
      return latest;
    }, (value) => value === status || (status === 'Authorized' && value.startsWith('Authorized')), options.connectTimeoutMs);
  } catch (error) {
    throw new Error(`${error instanceof Error ? error.message : error}; expected ${status}, last status ${latest || 'empty'}`);
  }
}

async function resolveCreatedSessionId(page, options, previousSessionIds) {
  return await poll('new admin session row', async () => {
    return resolveNewSessionId(previousSessionIds, await readVisibleSessionIds(page));
  }, Boolean, options.connectTimeoutMs);
}

async function selectSession(page, options, sessionId) {
  await openAdminTab(page, 'sessions');
  if (await readSelectedSessionId(page) !== sessionId) {
    await page.locator(`[data-testid="session-row"][data-session-id="${sessionId}"]`).click();
  }
  await poll('selected admin session', async () => {
    return await readSelectedSessionId(page);
  }, (selectedId) => selectedId === sessionId, options.connectTimeoutMs);
}

async function readSelectedSessionId(page) {
  const row = page.locator('[data-testid="session-row"][aria-pressed="true"]').first();
  if (!await row.isVisible().catch(() => false)) {
    return '';
  }
  return await row.getAttribute('data-session-id') ?? '';
}

async function readVisibleSessionIds(page) {
  return await page.locator('[data-testid="session-row"]').evaluateAll((rows) => rows
    .map((row) => row.getAttribute('data-session-id'))
    .filter((sessionId) => typeof sessionId === 'string' && sessionId.length > 0));
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

function healthUrl(bridge) { return bridgeUrl(bridge, '/health'); }
function sessionMcpUrl(bridge, sessionId) { return bridgeUrl(bridge, `/sessions/${encodeURIComponent(sessionId)}/mcp`); }
function apiOrigin(options) { return new URL('/', options.pageUrl).origin; }
function bridgeUrl(bridge, pathname) {
  const url = new URL(bridge.endpointBaseUrl ?? bridge.controlUrl);
  url.pathname = pathname;
  url.search = '';
  url.hash = '';
  return url.toString();
}

run().catch((error) => {
  console.error(`[admin-mcp-smoke] ${error.stack || error.message}`);
  process.exitCode = 1;
});
