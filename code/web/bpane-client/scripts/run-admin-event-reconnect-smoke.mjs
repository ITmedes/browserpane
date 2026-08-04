import fs from 'node:fs/promises';
import process from 'node:process';
import { execFile as execFileCallback } from 'node:child_process';
import { promisify } from 'node:util';
import { chromium } from 'playwright-core';
import {
  cleanupAdminBeforeRun,
  cleanupAdminSmoke,
  ensureAdminLoggedIn,
  getAdminAccessToken,
  openAdminTab,
} from './admin-smoke-lib.mjs';
import { DEFAULTS, PROJECT_ROOT, createLogger, fetchJson, launchChrome, parseSmokeArgs, poll } from './workflow-smoke-lib.mjs';

const execFile = promisify(execFileCallback);

async function run() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-admin-event-reconnect-smoke.mjs');
  if (options.pageUrl === DEFAULTS.pageUrl) options.pageUrl = `${DEFAULTS.pageUrl}/admin/`;
  const log = createLogger('admin-event-reconnect-smoke');
  const browser = await launchChrome(chromium, options);
  const context = await browser.newContext({ viewport: { width: 1360, height: 920 } });
  const page = await context.newPage();
  const eventStreams = trackAdminEventStreams(page);
  let sessionId = '';

  try {
    log(`Opening ${options.pageUrl}`);
    await ensureAdminLoggedIn(page, options);
    await cleanupAdminBeforeRun(page, options, log);
    const accessToken = await getAdminAccessToken(page);
    const initialStream = await waitForAuthenticatedEventStream(
      eventStreams,
      options.connectTimeoutMs,
    );
    assertCredentialFreeEventStream(initialStream, accessToken);

    await openAdminTab(page, 'logs');
    await page.getByTestId('admin-log-clear').click();
    const reconnectStartIndex = eventStreams.length;
    log('Restarting gateway to force admin event-stream reconnect.');
    await restartGateway();
    await waitForLogText(page, options, 'Admin event stream reconnecting.');
    await waitForLogText(page, options, 'Admin event stream open.');
    const reconnectedStream = await waitForAuthenticatedEventStream(
      eventStreams,
      options.connectTimeoutMs,
      reconnectStartIndex,
    );
    assertCredentialFreeEventStream(reconnectedStream, accessToken);
    if (reconnectedStream.token === initialStream.token) {
      throw new Error('admin event reconnect reused its previous scoped token');
    }

    log('Creating a session after reconnect and waiting for realtime UI sync.');
    const created = await createSessionAfterRestart(accessToken, options);
    sessionId = created.id;
    await waitForRealtimeSessionRow(page, options, sessionId);
    await openAdminTab(page, 'logs');
    await waitForLogText(page, options, 'Gateway session snapshot');
    await assertGatewayLogsExcludeCredentials([
      accessToken,
      initialStream.token,
      reconnectedStream.token,
    ]);
    await emitSummary(options, {
      sessionId,
      eventStreamReconnected: true,
      freshScopedEventToken: true,
      queryCredentialAbsent: true,
      realtimeSessionList: true,
      gatewayLogsExcludeObservedCredentials: true,
    }, log);
  } finally {
    await cleanupAdminSmoke(page, options, log);
    await context.close();
    await browser.close();
  }
}

function trackAdminEventStreams(page) {
  const observations = [];
  page.on('websocket', (socket) => {
    const url = new URL(socket.url());
    if (url.pathname !== '/api/v1/admin/events') return;
    const observation = { url, token: '', authenticated: false };
    observations.push(observation);
    socket.on('framesent', (event) => {
      const message = parseFrame(event.payload);
      if (message?.message_type === 'admin.authenticate' && typeof message.token === 'string') {
        observation.token = message.token;
      }
    });
    socket.on('framereceived', (event) => {
      const message = parseFrame(event.payload);
      if (message?.message_type === 'admin.authenticated') observation.authenticated = true;
    });
  });
  return observations;
}

function parseFrame(payload) {
  try {
    const text = typeof payload === 'string' ? payload : payload.toString('utf8');
    const parsed = JSON.parse(text);
    return parsed && typeof parsed === 'object' ? parsed : null;
  } catch {
    return null;
  }
}

async function waitForAuthenticatedEventStream(observations, timeoutMs, startIndex = 0) {
  return await poll('authenticated admin event stream', async () => {
    return observations.slice(startIndex).find((observation) => (
      observation.authenticated
      && observation.token.length > 0
    )) ?? null;
  }, Boolean, timeoutMs);
}

function assertCredentialFreeEventStream(observation, ownerAccessToken) {
  if (observation.url.search || observation.url.hash) {
    throw new Error('admin event websocket URL contains query or fragment data');
  }
  if (!observation.token.startsWith('v2.admin-events.')) {
    throw new Error('admin event websocket did not use a purpose-scoped v2 token');
  }
  if (observation.token === ownerAccessToken) {
    throw new Error('admin event websocket used the owner access token directly');
  }
}

async function assertGatewayLogsExcludeCredentials(credentials) {
  const { stdout, stderr } = await execFile(
    'docker',
    ['compose', '-f', 'deploy/compose.yml', 'logs', '--since', '10m', 'gateway'],
    { cwd: PROJECT_ROOT, maxBuffer: 10 * 1024 * 1024 },
  );
  const logs = `${stdout}\n${stderr}`;
  for (const credential of credentials) {
    if (credential && logs.includes(credential)) {
      throw new Error('gateway logs contain an observed credential');
    }
  }
}

async function restartGateway() {
  await execFile('docker', ['compose', '-f', 'deploy/compose.yml', 'restart', 'gateway'], {
    cwd: PROJECT_ROOT,
  });
}

async function createSessionAfterRestart(accessToken, options) {
  return await poll('gateway API after restart', async () => {
    try {
      return await fetchJson(`${apiOrigin(options)}/api/v1/sessions`, {
        method: 'POST',
        headers: {
          Authorization: `Bearer ${accessToken}`,
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({ labels: { suite: 'admin-event-reconnect-smoke' } }),
      });
    } catch {
      return null;
    }
  }, Boolean, options.connectTimeoutMs);
}

async function waitForRealtimeSessionRow(page, options, sessionId) {
  await openAdminTab(page, 'sessions');
  const row = page.locator(`[data-testid="session-row"][data-session-id="${sessionId}"]`);
  await row.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
}

async function waitForLogText(page, options, text) {
  await poll(`admin log ${text}`, async () => {
    const entries = await page.locator('[data-testid="admin-log-entry"]').allTextContents();
    return entries.some((entry) => entry.includes(text));
  }, Boolean, options.connectTimeoutMs);
}

async function emitSummary(options, summary, log) {
  console.log(JSON.stringify(summary, null, 2));
  if (options.outputPath) {
    await fs.writeFile(options.outputPath, `${JSON.stringify(summary, null, 2)}\n`, 'utf8');
    log(`Wrote summary to ${options.outputPath}`);
  }
}

function apiOrigin(options) {
  return new URL('/', options.pageUrl).origin;
}

run().catch((error) => {
  console.error(`[admin-event-reconnect-smoke] ${error.stack || error.message}`);
  process.exitCode = 1;
});
