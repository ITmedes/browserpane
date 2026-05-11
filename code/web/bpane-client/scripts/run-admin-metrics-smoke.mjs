import fs from 'node:fs/promises';
import process from 'node:process';
import { chromium } from 'playwright-core';
import {
  cleanupAdminBeforeRun,
  cleanupAdminSmoke,
  ensureAdminLoggedIn,
  openAdminTab,
  waitForBrowserConnected,
} from './admin-smoke-lib.mjs';
import { DEFAULTS, createLogger, launchChrome, parseSmokeArgs, poll, sleep } from './workflow-smoke-lib.mjs';

async function run() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-admin-metrics-smoke.mjs');
  if (options.pageUrl === DEFAULTS.pageUrl) options.pageUrl = `${DEFAULTS.pageUrl}/admin/`;
  const rootUrl = new URL('/', options.pageUrl).origin;
  const log = createLogger('admin-metrics-smoke');
  const browser = await launchChrome(chromium, options);
  const context = await browser.newContext({ viewport: { width: 1440, height: 980 } });
  await context.grantPermissions(['clipboard-read', 'clipboard-write'], { origin: rootUrl });
  const page = await context.newPage();

  try {
    log(`Opening ${options.pageUrl}`);
    await ensureAdminLoggedIn(page, options);
    await cleanupAdminBeforeRun(page, options, log);
    await openAdminTab(page, 'sessions');
    await page.getByTestId('session-new').click();
    const sessionId = await resolveSelectedSessionId(page, options);
    log(`Waiting for automatic browser join for ${sessionId}.`);
    await waitForBrowserConnected(page, options);

    await openAdminTab(page, 'metrics');
    await waitForEnabled(page.getByTestId('metrics-start'), options, 'metrics start');
    await page.getByTestId('metrics-start').click();
    await waitForText(page, options, 'metrics-sample', (value) => value.startsWith('running'));
    await exerciseBrowserViewport(page, options);
    await waitForEnabled(page.getByTestId('metrics-stop'), options, 'metrics stop');
    await page.getByTestId('metrics-stop').click();
    await waitForEnabled(page.getByTestId('metrics-copy'), options, 'metrics copy');
    await page.getByTestId('metrics-copy').click();
    const payload = await readMetricsClipboard(page, options);
    validateMetricsPayload(payload);
    await emitSummary(options, { sessionId, payload }, log);
  } finally {
    await cleanupAdminSmoke(page, options, log);
    await context.close();
    await browser.close();
  }
}

async function exerciseBrowserViewport(page, options) {
  const canvas = page.locator('[data-testid="browser-viewport"] canvas').first();
  await canvas.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  await canvas.click({ force: true });
  await page.mouse.wheel(0, 900);
  await sleep(700);
  await page.keyboard.press('PageDown');
  await sleep(700);
  await page.keyboard.press('PageUp');
  await sleep(700);
}

async function readMetricsClipboard(page, options) {
  const raw = await poll('metrics clipboard JSON', async () => {
    try {
      return await page.evaluate(() => navigator.clipboard.readText());
    } catch {
      return '';
    }
  }, (value) => value.includes('browserpane.admin.metrics.sample.v1'), options.connectTimeoutMs);
  return JSON.parse(raw);
}

function validateMetricsPayload(payload) {
  if (payload.schema !== 'browserpane.admin.metrics.sample.v1') {
    throw new Error(`Unexpected metrics schema ${payload.schema}`);
  }
  assertNumber(payload.timing?.durationMs, 'timing.durationMs');
  assertNumber(payload.frames?.delta, 'frames.delta');
  assertNumber(payload.transfer?.rxBytes, 'transfer.rxBytes');
  assertNumber(payload.transfer?.txBytes, 'transfer.txBytes');
  assertNumber(payload.transfer?.peakRxRate, 'transfer.peakRxRate');
  assertNumber(payload.transfer?.avgTileRate, 'transfer.avgTileRate');
  assertNumber(payload.tiles?.totalCommands, 'tiles.totalCommands');
  assertNumber(payload.tiles?.cache?.hitRate, 'tiles.cache.hitRate');
  assertNumber(payload.tiles?.batches?.maxPendingCommands, 'tiles.batches.maxPendingCommands');
  assertNumber(payload.scroll?.hostFallbackRate, 'scroll.hostFallbackRate');
  assertNumber(payload.scroll?.hostFallbacks, 'scroll.hostFallbacks');
  assertNumber(payload.video?.datagrams, 'video.datagrams');
  if (!payload.render || typeof payload.render.backend !== 'string') {
    throw new Error('Metrics payload did not include render diagnostics.');
  }
}

function assertNumber(value, label) {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    throw new Error(`Metrics payload ${label} is not a finite number.`);
  }
}

async function resolveSelectedSessionId(page, options) {
  const row = page.getByTestId('session-row').first();
  await row.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  const sessionId = await row.getAttribute('data-session-id') ?? '';
  if (!sessionId) throw new Error('Admin session row did not expose a session id.');
  return sessionId;
}

async function waitForEnabled(locator, options, description) {
  await poll(description, async () => await locator.isEnabled(), Boolean, options.connectTimeoutMs);
}

async function waitForText(page, options, testId, predicate) {
  await poll(testId, async () => await page.getByTestId(testId).textContent(), (value) => predicate(value ?? ''), options.connectTimeoutMs);
}

async function emitSummary(options, summary, log) {
  console.log(JSON.stringify(summary, null, 2));
  if (options.outputPath) {
    await fs.writeFile(options.outputPath, `${JSON.stringify(summary, null, 2)}\n`, 'utf8');
    log(`Wrote summary to ${options.outputPath}`);
  }
}

run().catch((error) => {
  console.error(`[admin-metrics-smoke] ${error.stack || error.message}`);
  process.exitCode = 1;
});
