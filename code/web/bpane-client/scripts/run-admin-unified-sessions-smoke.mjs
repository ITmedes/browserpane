import process from 'node:process';
import { chromium } from 'playwright-core';
import {
  ensureAdminLoggedIn,
  getAdminAccessToken,
} from './admin-smoke-lib.mjs';
import {
  DEFAULTS,
  apiOrigin,
  createLogger,
  launchChrome,
  parseSmokeArgs,
  poll,
} from './workflow-smoke-lib.mjs';

async function run() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-admin-unified-sessions-smoke.mjs');
  if (options.pageUrl === DEFAULTS.pageUrl) {
    options.pageUrl = `${DEFAULTS.pageUrl}/admin-new/sessions`;
  }
  const log = createLogger('admin-unified-sessions-smoke');
  const browser = await launchChrome(chromium, options);
  const context = await browser.newContext({ viewport: { width: 1440, height: 980 } });
  const page = await context.newPage();
  let accessToken = '';
  let createdSessionId = '';

  try {
    log(`Opening ${options.pageUrl}`);
    await ensureAdminLoggedIn(page, options);
    accessToken = await getAdminAccessToken(page);
    if (!accessToken) {
      throw new Error('No admin access token available after login.');
    }

    await page.goto(adminRouteUrl(options, 'sessions'), { waitUntil: 'domcontentloaded' });
    await page.getByTestId('sessions-overview').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });
    await assertNoBodyHorizontalOverflow(page, 'unified sessions catalog');
    await assertNoHorizontalOverflow(page, 'sessions-overview', 'unified sessions overview');

    await page.getByTestId('sessions-new').click();
    createdSessionId = await poll(
      'created unified session id',
      async () => {
        const text = await page.getByTestId('sessions-action-success').textContent().catch(() => '');
        return text?.match(/Session ([^ ]+) created/)?.[1] ?? '';
      },
      Boolean,
      options.connectTimeoutMs,
    );

    await page.getByTestId('sessions-search').fill(createdSessionId);
    const row = page.locator('[data-testid="sessions-list-row"]').filter({ hasText: createdSessionId });
    await row.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
    const detailLink = row.getByTestId('sessions-detail-link');
    const href = await detailLink.getAttribute('href');
    if (!href?.endsWith(`/admin-new/sessions/${encodeURIComponent(createdSessionId)}`)) {
      throw new Error(`Expected session detail link for ${createdSessionId}, got ${href}`);
    }
    await detailLink.click();

    await page.getByTestId('session-detail-route').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });
    await waitForContains(page, options, 'session-detail-title', shortSessionId(createdSessionId));
    await page.getByTestId('session-detail-lifecycle').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });
    await page.getByTestId('session-detail-runtime-section').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });
    await page.getByTestId('session-detail-capabilities').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });
    await assertNoBodyHorizontalOverflow(page, 'unified session detail');
    await assertNoHorizontalOverflow(page, 'session-detail-route', 'unified session detail route');
    await assertNoHorizontalOverflow(page, 'session-inspector', 'unified session inspector');

    await page.getByTestId('session-detail-refresh').click();
    await waitForContains(page, options, 'session-detail-action-success', 'refreshed');
    await runSafeTerminalAction(page, options);

    console.log(JSON.stringify({
      sessionId: createdSessionId,
      detailVisible: true,
      previewEmbedded: false,
    }, null, 2));
  } finally {
    if (accessToken && createdSessionId) {
      await cleanupSession(accessToken, options, createdSessionId).catch((error) => {
        log(`Session cleanup for ${createdSessionId} failed: ${error.message}`);
      });
    }
    await context.close();
    await browser.close();
  }
}

async function runSafeTerminalAction(page, options) {
  const cancel = page.getByTestId('session-cancel-queue');
  const stop = page.getByTestId('session-stop');
  const kill = page.getByTestId('session-kill');

  if (await cancel.isEnabled().catch(() => false)) {
    await cancel.click();
    await waitForContains(page, options, 'session-detail-action-success', 'cancelled');
    return;
  }
  if (await stop.isEnabled().catch(() => false)) {
    await stop.click();
    await waitForContains(page, options, 'session-detail-action-success', 'stopped');
    return;
  }
  if (await kill.isEnabled().catch(() => false)) {
    await kill.click();
    await waitForContains(page, options, 'session-detail-action-success', 'killed');
    return;
  }
  throw new Error('No safe terminal lifecycle action was enabled for the smoke-created session.');
}

async function waitForContains(page, options, testId, expected) {
  await poll(
    testId,
    async () => await page.getByTestId(testId).textContent().catch(() => ''),
    (value) => value?.includes(expected),
    options.connectTimeoutMs,
  );
}

async function assertNoHorizontalOverflow(page, testId, label) {
  const size = await page.getByTestId(testId).evaluate((element) => ({
    clientWidth: element.clientWidth,
    scrollWidth: element.scrollWidth,
  }));
  if (size.scrollWidth > size.clientWidth + 1) {
    throw new Error(`${label} overflows horizontally: ${JSON.stringify(size)}`);
  }
}

async function assertNoBodyHorizontalOverflow(page, label) {
  const size = await page.evaluate(() => ({
    clientWidth: document.documentElement.clientWidth,
    scrollWidth: document.documentElement.scrollWidth,
  }));
  if (size.scrollWidth > size.clientWidth + 1) {
    throw new Error(`${label} causes document horizontal overflow: ${JSON.stringify(size)}`);
  }
}

async function cleanupSession(accessToken, options, sessionId) {
  const response = await fetch(`${apiOrigin(options)}/api/v1/sessions/${encodeURIComponent(sessionId)}/kill`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${accessToken}` },
  });
  if (response.ok || response.status === 404 || response.status === 409) {
    return;
  }
  const detail = await response.text().catch(() => '');
  throw new Error(`HTTP ${response.status}${detail ? ` ${detail}` : ''}`);
}

function adminRouteUrl(options, routePath) {
  return new URL(`/admin-new/${routePath.replace(/^\/+/, '')}`, apiOrigin(options)).toString();
}

function shortSessionId(sessionId) {
  return sessionId.length > 13 ? `${sessionId.slice(0, 8)}...${sessionId.slice(-4)}` : sessionId;
}

run().catch((error) => {
  console.error(`[admin-unified-sessions-smoke] ${error.stack || error.message}`);
  process.exitCode = 1;
});
