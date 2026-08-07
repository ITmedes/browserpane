import fs from 'node:fs/promises';
import process from 'node:process';
import { chromium } from 'playwright-core';
import { ensureAdminLoggedIn } from './admin-smoke-lib.mjs';
import {
  assertNoBodyHorizontalOverflow,
  assertNoHorizontalOverflow,
} from './admin-unified-smoke-lib.mjs';
import {
  DEFAULTS,
  createLogger,
  launchChrome,
  parseSmokeArgs,
  poll,
} from './workflow-smoke-lib.mjs';

async function run() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-admin-unified-dashboard-smoke.mjs');
  if (options.pageUrl === DEFAULTS.pageUrl) {
    options.pageUrl = `${DEFAULTS.pageUrl}/admin-new/`;
  }
  const log = createLogger('admin-unified-dashboard-smoke');
  const browser = await launchChrome(chromium, options);
  const context = await browser.newContext({ viewport: { width: 1440, height: 980 } });
  const page = await context.newPage();
  let summary = null;

  try {
    log(`Opening ${options.pageUrl}`);
    await ensureAdminLoggedIn(page, options);
    await page.getByTestId('dashboard-overview').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });

    await waitForDashboardMetric(page, options, 'dashboard-metric-sessions');
    await waitForDashboardMetric(page, options, 'dashboard-metric-workflow-runs');
    await expectLink(page, 'dashboard-link-sessions', '/admin-new/sessions');
    await expectLink(page, 'dashboard-link-projects', '/admin-new/projects');
    await expectLink(page, 'dashboard-link-runs', '/admin-new/runs');
    await expectLink(page, 'dashboard-link-recordings', '/admin-new/recordings');
    await assertNoBodyHorizontalOverflow(page, 'unified dashboard');
    await assertNoHorizontalOverflow(page, 'dashboard-overview', 'unified dashboard overview');

    summary = {
      pageUrl: options.pageUrl,
      metricsVisible: true,
      quickLinksVisible: true,
    };
    await emitSummary(options, summary, log);
  } finally {
    await context.close();
    await browser.close();
  }
}

async function waitForDashboardMetric(page, options, testId) {
  await poll(
    testId,
    async () => await page.getByTestId(testId).textContent(),
    (value) => typeof value === 'string' && /\d/.test(value),
    options.connectTimeoutMs,
  );
}

async function expectLink(page, testId, expectedPath) {
  const href = await page.getByTestId(testId).getAttribute('href');
  if (!href || !href.includes(expectedPath)) {
    throw new Error(`Expected ${testId} to link to ${expectedPath}, got ${href}`);
  }
}

async function emitSummary(options, summary, log) {
  console.log(JSON.stringify(summary, null, 2));
  if (options.outputPath) {
    await fs.writeFile(options.outputPath, `${JSON.stringify(summary, null, 2)}\n`, 'utf8');
    log(`Wrote summary to ${options.outputPath}`);
  }
}

run().catch((error) => {
  console.error(`[admin-unified-dashboard-smoke] ${error.stack || error.message}`);
  process.exitCode = 1;
});
