import fs from 'node:fs/promises';
import process from 'node:process';
import { chromium } from 'playwright-core';
import {
  ensureAdminLoggedIn,
  getAdminAccessToken,
} from './admin-smoke-lib.mjs';
import { createWorkflow, createWorkflowVersion } from './admin-workflow-smoke-lib.mjs';
import {
  DEFAULTS,
  apiOrigin,
  createLogger,
  launchChrome,
  parseSmokeArgs,
  poll,
} from './workflow-smoke-lib.mjs';

async function run() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-admin-unified-workflows-smoke.mjs');
  if (options.pageUrl === DEFAULTS.pageUrl) {
    options.pageUrl = `${DEFAULTS.pageUrl}/admin-new/workflows`;
  }
  const log = createLogger('admin-unified-workflows-smoke');
  const browser = await launchChrome(chromium, options);
  const context = await browser.newContext({ viewport: { width: 1440, height: 980 } });
  const page = await context.newPage();
  let summary = null;

  try {
    log(`Opening ${options.pageUrl}`);
    await ensureAdminLoggedIn(page, options);
    const accessToken = await getAdminAccessToken(page);
    if (!accessToken) {
      throw new Error('No admin access token available after login.');
    }

    const hiddenWorkflow = await createWorkflow(accessToken, apiOrigin(options));
    await createWorkflowVersion(accessToken, apiOrigin(options), hiddenWorkflow.id);

    await page.goto(adminRouteUrl(options, 'workflows'), { waitUntil: 'domcontentloaded' });
    await page.getByTestId('workflows-overview').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });
    await assertNoBodyHorizontalOverflow(page, 'unified workflow catalog');
    await assertNoHorizontalOverflow(page, 'workflows-overview', 'unified workflow overview');

    const hiddenCount = await page.getByText(hiddenWorkflow.name).count();
    if (hiddenCount !== 0) {
      throw new Error(`Hidden smoke workflow ${hiddenWorkflow.name} appeared in the unified catalog.`);
    }
    await waitForContains(page, options, 'workflows-hidden-note', 'internal workflow');
    await page.getByTestId('workflows-search').fill('BrowserPane Tour');
    const tourRow = await waitForCatalogRow(page, options, 'BrowserPane Tour');
    const href = await tourRow.getByTestId('workflows-detail-link').getAttribute('href');
    if (!href?.includes('/admin-new/workflows/')) {
      throw new Error(`Expected BrowserPane Tour detail link, got ${href}`);
    }
    await tourRow.getByTestId('workflows-detail-link').click();
    await page.waitForURL(/\/admin-new\/workflows\/[^/]+$/, { timeout: options.connectTimeoutMs });
    await page.getByTestId('workflow-definition-detail-route').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });
    await waitForContains(page, options, 'workflow-definition-detail-title', 'BrowserPane Tour');
    await waitForContains(page, options, 'workflow-definition-detail-kind', 'Example template');
    await waitForContains(page, options, 'workflow-definition-detail-latest-version', 'v1');
    await waitForContains(page, options, 'workflow-definition-version-entrypoint', 'browserpane-tour');
    await waitForContains(page, options, 'workflow-definition-source', '/workspace');
    await waitForContains(page, options, 'workflow-definition-policy', 'File workspaces');
    await waitForSourceTreePath(page, options, 'dev/workflows/browserpane-tour/run.mjs');
    await waitForSourceTreePath(page, options, 'dev/web-fixtures');
    await waitForContains(page, options, 'workflow-code-preview-code', 'export default async function run');
    await waitForContains(page, options, 'workflow-code-preview-language', 'TypeScript');
    const highlightedKeywordCount = await page
      .getByTestId('workflow-code-preview-code')
      .locator('.hljs-keyword')
      .count();
    if (highlightedKeywordCount === 0) {
      throw new Error('Workflow code preview did not render TypeScript syntax highlighting.');
    }
    await page.locator(sourceTreeSelector('dev/web-fixtures')).click();
    await waitForSourceTreePath(page, options, 'dev/web-fixtures/test-embed.html');
    await page.locator(sourceTreeSelector('dev/web-fixtures/test-embed.html')).click();
    await waitForContains(page, options, 'workflow-code-preview-entrypoint', 'dev/web-fixtures/test-embed.html');
    await waitForContains(page, options, 'workflow-code-preview-code', 'BrowserPane Test Embed');
    await assertNoBodyHorizontalOverflow(page, 'unified workflow detail');
    await assertNoHorizontalOverflow(page, 'workflow-definition-detail-route', 'unified workflow detail route');

    await verifyResponsiveDetailLayout(page, options);

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
    `unified workflow catalog row ${text}`,
    async () => {
      const row = page.getByTestId('workflows-list-row').filter({ hasText: text }).first();
      return await row.isVisible().catch(() => false) ? row : null;
    },
    Boolean,
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

async function waitForSourceTreePath(page, options, path) {
  await poll(
    `source tree path ${path}`,
    async () => await page.locator(sourceTreeSelector(path)).count(),
    (count) => count > 0,
    options.connectTimeoutMs,
  );
}

function sourceTreeSelector(path) {
  return `[data-source-path="${path}"]`;
}

async function verifyResponsiveDetailLayout(page, options) {
  await page.setViewportSize({ width: 390, height: 900 });
  await page.reload({ waitUntil: 'domcontentloaded' });
  await page.getByTestId('workflow-definition-detail-route').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await page.getByTestId('workflow-definition-inspector').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await assertNoBodyHorizontalOverflow(page, 'mobile unified workflow detail');
  await assertNoHorizontalOverflow(page, 'workflow-definition-detail-route', 'mobile unified workflow detail route');
  await assertNoHorizontalOverflow(page, 'workflow-definition-inspector', 'mobile unified workflow inspector');
  await page.setViewportSize({ width: 1440, height: 980 });
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

function adminRouteUrl(options, routePath) {
  return new URL(`/admin-new/${routePath.replace(/^\/+/, '')}`, apiOrigin(options)).toString();
}

async function emitSummary(options, summary, log) {
  console.log(JSON.stringify(summary, null, 2));
  if (options.outputPath) {
    await fs.writeFile(options.outputPath, `${JSON.stringify(summary, null, 2)}\n`, 'utf8');
    log(`Wrote summary to ${options.outputPath}`);
  }
}

run().catch((error) => {
  console.error(`[admin-unified-workflows-smoke] ${error.stack || error.message}`);
  process.exitCode = 1;
});
