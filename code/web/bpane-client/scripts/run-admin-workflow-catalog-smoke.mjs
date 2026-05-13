import fs from 'node:fs/promises';
import process from 'node:process';
import { chromium } from 'playwright-core';
import {
  cleanupAdminBeforeRun,
  ensureAdminLoggedIn,
  getAdminAccessToken,
} from './admin-smoke-lib.mjs';
import { createWorkflow, createWorkflowVersion } from './admin-workflow-smoke-lib.mjs';
import { DEFAULTS, createLogger, launchChrome, parseSmokeArgs, poll } from './workflow-smoke-lib.mjs';

async function run() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-admin-workflow-catalog-smoke.mjs');
  if (options.pageUrl === DEFAULTS.pageUrl) {
    options.pageUrl = `${DEFAULTS.pageUrl}/admin/`;
  }
  const rootUrl = new URL('/', options.pageUrl).origin;
  const log = createLogger('admin-workflow-catalog-smoke');
  const browser = await launchChrome(chromium, options);
  const context = await browser.newContext({ viewport: { width: 1440, height: 980 } });
  const page = await context.newPage();
  let summary = null;

  try {
    log(`Opening ${options.pageUrl}`);
    await ensureAdminLoggedIn(page, options);
    await cleanupAdminBeforeRun(page, options, log);
    const accessToken = await getAdminAccessToken(page);
    const hiddenWorkflow = await createWorkflow(accessToken, rootUrl);
    await createWorkflowVersion(accessToken, rootUrl, hiddenWorkflow.id);

    await page.goto(adminRouteUrl(options, 'workflows'), { waitUntil: 'domcontentloaded' });
    await page.getByTestId('workflow-catalog').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });
    const tourRow = await waitForCatalogRow(page, options, 'BrowserPane Tour');
    const hiddenCount = await page.getByText(hiddenWorkflow.name).count();
    if (hiddenCount !== 0) {
      throw new Error(`Hidden smoke workflow ${hiddenWorkflow.name} appeared in the catalog.`);
    }
    await tourRow.click();
    await page.waitForURL(/\/workflows\/[^/]+$/, { timeout: options.connectTimeoutMs });
    await page.getByTestId('workflow-definition-detail').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });
    await waitForText(page, options, 'workflow-definition-detail-title', 'BrowserPane Tour');
    await waitForText(page, options, 'workflow-definition-detail-kind', 'Example template');
    await waitForText(page, options, 'workflow-definition-detail-latest-version', 'v1');
    await page.getByTestId('workflow-definition-version-row').first().waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });
    await waitForContains(page, options, 'workflow-definition-version-entrypoint', 'browserpane-tour');
    await waitForContains(page, options, 'workflow-definition-source', '/workspace');

    summary = {
      pageUrl: options.pageUrl,
      hiddenWorkflowId: hiddenWorkflow.id,
      catalogTemplate: 'BrowserPane Tour',
      detailVisible: true,
    };
    await emitSummary(options, summary, log);
  } finally {
    await context.close();
    await browser.close();
  }
}

async function waitForCatalogRow(page, options, text) {
  return await poll(
    `workflow catalog row ${text}`,
    async () => {
      const row = page.getByTestId('workflow-catalog-row').filter({ hasText: text }).first();
      return await row.isVisible().catch(() => false) ? row : null;
    },
    Boolean,
    options.connectTimeoutMs,
  );
}

async function waitForText(page, options, testId, expected) {
  await poll(
    testId,
    async () => await page.getByTestId(testId).textContent(),
    (value) => value === expected,
    options.connectTimeoutMs,
  );
}

async function waitForContains(page, options, testId, expected) {
  await poll(
    testId,
    async () => await page.getByTestId(testId).textContent(),
    (value) => value?.includes(expected),
    options.connectTimeoutMs,
  );
}

function adminRouteUrl(options, routePath) {
  const baseUrl = new URL(options.pageUrl);
  if (!baseUrl.pathname.endsWith('/')) {
    baseUrl.pathname = `${baseUrl.pathname}/`;
  }
  return new URL(routePath, baseUrl).toString();
}

async function emitSummary(options, summary, log) {
  console.log(JSON.stringify(summary, null, 2));
  if (options.outputPath) {
    await fs.writeFile(options.outputPath, `${JSON.stringify(summary, null, 2)}\n`, 'utf8');
    log(`Wrote summary to ${options.outputPath}`);
  }
}

run().catch((error) => {
  console.error(`[admin-workflow-catalog-smoke] ${error.stack || error.message}`);
  process.exitCode = 1;
});
