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
  let sessionId = '';

  try {
    log(`Opening ${options.pageUrl}`);
    await ensureAdminLoggedIn(page, options);
    await cleanupAdminBeforeRun(page, options, log);
    const accessToken = await getAdminAccessToken(page);

    await openAdminTab(page, 'logs');
    await page.getByTestId('admin-log-clear').click();
    log('Restarting gateway to force admin event-stream reconnect.');
    await restartGateway();
    await waitForLogText(page, options, 'Admin event stream reconnecting.');
    await waitForLogText(page, options, 'Admin event stream open.');

    log('Creating a session after reconnect and waiting for realtime UI sync.');
    const created = await createSessionAfterRestart(accessToken, options);
    sessionId = created.id;
    await waitForRealtimeSessionRow(page, options, sessionId);
    await openAdminTab(page, 'logs');
    await waitForLogText(page, options, 'Gateway session snapshot');
    await emitSummary(options, { sessionId, eventStreamReconnected: true, realtimeSessionList: true }, log);
  } finally {
    await cleanupAdminSmoke(page, options, log);
    await context.close();
    await browser.close();
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
