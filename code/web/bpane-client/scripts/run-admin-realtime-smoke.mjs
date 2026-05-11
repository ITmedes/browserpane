import fs from 'node:fs/promises';
import process from 'node:process';
import { chromium } from 'playwright-core';
import {
  cleanupAdminBeforeRun,
  cleanupAdminSmoke,
  ensureAdminLoggedIn,
  getAdminAccessToken,
  openAdminTab,
} from './admin-smoke-lib.mjs';
import { DEFAULTS, createLogger, fetchJson, launchChrome, parseSmokeArgs, poll } from './workflow-smoke-lib.mjs';

async function run() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-admin-realtime-smoke.mjs');
  if (options.pageUrl === DEFAULTS.pageUrl) {
    options.pageUrl = `${DEFAULTS.pageUrl}/admin/`;
  }
  const log = createLogger('admin-realtime-smoke');
  const browser = await launchChrome(chromium, options);
  const context = await browser.newContext({ viewport: { width: 1360, height: 920 } });
  const page = await context.newPage();
  let sessionId = '';
  let accessToken = '';

  try {
    log(`Opening ${options.pageUrl}`);
    await ensureAdminLoggedIn(page, options);
    await cleanupAdminBeforeRun(page, options, log);
    await openAdminTab(page, 'sessions');
    accessToken = await getAdminAccessToken(page);

    log('Creating a session through REST, not through the admin button.');
    const created = await createSession(accessToken, options);
    sessionId = created.id;
    await waitForRealtimeSessionRow(page, options, sessionId);
    await page.locator(`[data-testid="session-row"][data-session-id="${sessionId}"]`).click();

    log('Delegating the selected session through REST and waiting for realtime MCP state.');
    const mcpBridge = await loadMcpBridgeConfig(options);
    await setAutomationDelegate(accessToken, options, sessionId, mcpBridge);
    await waitForRealtimeMcpDelegation(page, options);

    log('Stopping the session through REST and waiting for websocket-driven UI state.');
    await openAdminTab(page, 'lifecycle');
    await stopSession(accessToken, options, sessionId);
    await poll('admin realtime stopped state', async () => {
      return await page.getByTestId('session-state').textContent();
    }, (state) => state === 'stopped', options.connectTimeoutMs);
    await openAdminTab(page, 'logs');
    await waitForGatewayLogEntry(page, options, 'Gateway session snapshot');
    await waitForGatewayLogEntry(page, options, 'Gateway MCP delegation snapshot', '1 delegated');
    await emitSummary(options, sessionId, log, true);
  } finally {
    if (accessToken && sessionId) {
      await clearAutomationDelegate(accessToken, options, sessionId).catch(() => {});
    }
    await cleanupAdminSmoke(page, options, log);
    await context.close();
    await browser.close();
  }
}

async function createSession(accessToken, options) {
  return await fetchJson(`${apiOrigin(options)}/api/v1/sessions`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({ labels: { suite: 'admin-realtime-smoke' } }),
  });
}

async function stopSession(accessToken, options, sessionId) {
  return await fetchJson(`${apiOrigin(options)}/api/v1/sessions/${sessionId}/stop`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function loadMcpBridgeConfig(options) {
  const config = await fetchJson(`${apiOrigin(options)}/auth-config.json`);
  if (!config.mcpBridge?.clientId) {
    throw new Error('Admin realtime smoke requires auth-config mcpBridge metadata.');
  }
  return config.mcpBridge;
}

async function setAutomationDelegate(accessToken, options, sessionId, bridge) {
  return await fetchJson(`${apiOrigin(options)}/api/v1/sessions/${sessionId}/automation-owner`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      client_id: bridge.clientId,
      issuer: bridge.issuer,
      display_name: bridge.displayName,
    }),
  });
}

async function clearAutomationDelegate(accessToken, options, sessionId) {
  const response = await fetch(`${apiOrigin(options)}/api/v1/sessions/${sessionId}/automation-owner`, {
    method: 'DELETE',
    headers: { Authorization: `Bearer ${accessToken}` },
  });
  if (!response.ok && response.status !== 404) {
    const detail = await response.text().catch(() => '');
    throw new Error(`HTTP ${response.status}${detail ? ` ${detail}` : ''}`);
  }
}

async function waitForRealtimeSessionRow(page, options, sessionId) {
  const row = page.locator(`[data-testid="session-row"][data-session-id="${sessionId}"]`);
  await row.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
}

async function waitForRealtimeMcpDelegation(page, options) {
  await openAdminTab(page, 'sessions');
  await poll('admin realtime MCP delegation state', async () => {
    return await page.getByTestId('mcp-status').textContent().catch(() => '');
  }, (status) => status?.startsWith('Authorized'), options.connectTimeoutMs);
}

async function waitForGatewayLogEntry(page, options, ...textParts) {
  await poll('admin realtime gateway log entry', async () => {
    const texts = await page.locator('[data-testid="admin-log-entry"][data-log-source="gateway"]').allTextContents();
    return texts.some((text) => textParts.every((part) => text.includes(part)));
  }, Boolean, options.connectTimeoutMs);
}

async function emitSummary(options, sessionId, log, realtimeMcpDelegation) {
  const summary = {
    pageUrl: options.pageUrl,
    sessionId,
    realtimeSessionList: true,
    realtimeMcpDelegation,
    realtimeLifecycleState: 'stopped',
    realtimeGatewayLogs: true,
  };
  console.log(JSON.stringify(summary, null, 2));
  if (options.outputPath) {
    await fs.writeFile(options.outputPath, JSON.stringify(summary, null, 2));
    log(`Wrote summary to ${options.outputPath}`);
  }
}

function apiOrigin(options) {
  return new URL('/', options.pageUrl).origin;
}

run().catch((error) => {
  console.error(`[admin-realtime-smoke] ${error.stack || error.message}`);
  process.exitCode = 1;
});
