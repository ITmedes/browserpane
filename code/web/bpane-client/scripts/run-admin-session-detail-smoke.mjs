import fs from 'node:fs/promises';
import process from 'node:process';
import { chromium } from 'playwright-core';
import {
  cleanupAdminBeforeRun,
  cleanupAdminSmoke,
  ensureAdminLoggedIn,
  openAdminTab,
} from './admin-smoke-lib.mjs';
import { DEFAULTS, createLogger, launchChrome, parseSmokeArgs, poll } from './workflow-smoke-lib.mjs';

async function run() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-admin-session-detail-smoke.mjs');
  if (options.pageUrl === DEFAULTS.pageUrl) {
    options.pageUrl = `${DEFAULTS.pageUrl}/admin/`;
  }
  const log = createLogger('admin-session-detail-smoke');
  const browser = await launchChrome(chromium, options);
  const context = await browser.newContext({ viewport: { width: 1440, height: 980 } });
  const page = await context.newPage();
  let sessionId = '';

  try {
    log(`Opening ${options.pageUrl}`);
    await ensureAdminLoggedIn(page, options);
    await cleanupAdminBeforeRun(page, options, log);

    log('Creating a session from the route-level session list.');
    await page.goto(adminRouteUrl(options, 'sessions'), { waitUntil: 'domcontentloaded' });
    await page.getByTestId('session-inspector-list').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });
    await page.getByTestId('session-inspector-new').click();
    sessionId = await waitForSessionDetailUrl(page, options);

    await verifySessionDetail(page, options, sessionId);
    await verifyListDeepLink(page, options, sessionId);
    await verifyLiveWorkspaceDetailLink(page, options, sessionId);
    await emitSummary(page, options, sessionId, log);
  } finally {
    await cleanupAdminSmoke(page, options, log);
    await context.close();
    await browser.close();
  }
}

async function verifySessionDetail(page, options, sessionId) {
  await page.getByTestId('session-inspector-detail').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await page.getByTestId('session-state').waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  await page.getByTestId('session-runtime-state').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await page.getByTestId('session-inspector-files-count').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await page.getByTestId('session-inspector-recording-count').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  const title = await page.getByTestId('session-inspector-title').textContent();
  if (!title?.includes(sessionId)) {
    throw new Error(`Expected session detail title to include ${sessionId}, got ${title}`);
  }
  const disconnectAllDisabled = await page.getByTestId('session-disconnect-all').isDisabled();
  if (!disconnectAllDisabled) {
    throw new Error('Expected disconnect-all to be disabled for a newly created unconnected session.');
  }
  await page.getByTestId('session-inspector-detail-refresh').click();
  await poll('session detail refresh timestamp', async () => {
    return await page.getByTestId('session-inspector-last-refresh').textContent();
  }, (value) => Boolean(value && !value.includes('not refreshed')), options.connectTimeoutMs);
}

async function verifyListDeepLink(page, options, sessionId) {
  await page.goto(adminRouteUrl(options, 'sessions'), { waitUntil: 'domcontentloaded' });
  const row = page.locator(`[data-testid="session-inspector-row"][data-session-id="${sessionId}"]`);
  await row.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  await row.click();
  await waitForSessionDetailUrl(page, options, sessionId);
}

async function verifyLiveWorkspaceDetailLink(page, options, sessionId) {
  await page.goto(options.pageUrl, { waitUntil: 'domcontentloaded' });
  await openAdminTab(page, 'sessions');
  const row = page.locator(`[data-testid="session-row"][data-session-id="${sessionId}"]`);
  await row.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  await row.click();
  const detailLink = page.getByTestId('session-detail-link');
  await detailLink.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  const href = await detailLink.getAttribute('href');
  if (!href?.includes(`/sessions/${sessionId}`)) {
    throw new Error(`Expected live workspace detail link for ${sessionId}, got ${href}`);
  }
}

async function waitForSessionDetailUrl(page, options, expectedSessionId = '') {
  await page.waitForURL(/\/sessions\/[^/]+$/, { timeout: options.connectTimeoutMs });
  const sessionId = decodeURIComponent(new URL(page.url()).pathname.split('/').filter(Boolean).at(-1) ?? '');
  if (!sessionId) {
    throw new Error(`Could not resolve session id from ${page.url()}`);
  }
  if (expectedSessionId && sessionId !== expectedSessionId) {
    throw new Error(`Expected route session ${expectedSessionId}, got ${sessionId}`);
  }
  return sessionId;
}

async function emitSummary(page, options, sessionId, log) {
  const detailUrl = adminRouteUrl(options, `sessions/${sessionId}`);
  await page.goto(detailUrl, { waitUntil: 'domcontentloaded' });
  await page.getByTestId('session-state').waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  const summary = {
    pageUrl: options.pageUrl,
    sessionId,
    detailUrl,
    state: await page.getByTestId('session-state').textContent(),
  };
  console.log(JSON.stringify(summary, null, 2));
  if (options.outputPath) {
    await fs.writeFile(options.outputPath, JSON.stringify(summary, null, 2));
    log(`Wrote summary to ${options.outputPath}`);
  }
}

function adminRouteUrl(options, routePath) {
  const baseUrl = new URL(options.pageUrl);
  if (!baseUrl.pathname.endsWith('/')) {
    baseUrl.pathname = `${baseUrl.pathname}/`;
  }
  return new URL(routePath, baseUrl).toString();
}

run().catch((error) => {
  console.error(`[admin-session-detail-smoke] ${error.stack || error.message}`);
  process.exitCode = 1;
});
