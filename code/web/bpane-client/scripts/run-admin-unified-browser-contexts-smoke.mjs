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
  fetchJson,
  launchChrome,
  parseSmokeArgs,
} from './workflow-smoke-lib.mjs';

async function run() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-admin-unified-browser-contexts-smoke.mjs');
  if (options.pageUrl === DEFAULTS.pageUrl) {
    options.pageUrl = `${DEFAULTS.pageUrl}/admin-new/browser-contexts`;
  }
  const log = createLogger('admin-unified-browser-contexts-smoke');
  const browser = await launchChrome(chromium, options);
  const context = await browser.newContext({ viewport: { width: 1440, height: 980 } });
  const page = await context.newPage();
  const runLabel = `admin-unified-contexts-smoke-${Date.now()}`;
  let accessToken = '';
  let project = null;
  let browserContext = null;

  try {
    log(`Opening ${options.pageUrl}`);
    await ensureAdminLoggedIn(page, options);
    accessToken = await getAdminAccessToken(page);
    if (!accessToken) {
      throw new Error('No admin access token available after login.');
    }

    project = await createProject(accessToken, options, runLabel);

    await page.goto(adminRouteUrl(options, 'browser-contexts'), { waitUntil: 'domcontentloaded' });
    await page.getByTestId('browser-contexts-new-link').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });
    await assertNoBodyHorizontalOverflow(page, 'unified browser-context catalog');
    await assertNoHorizontalOverflow(page, 'browser-contexts-overview', 'unified browser-context overview');

    browserContext = await createContextThroughUi(page, accessToken, options, project, runLabel);
    verifyCreatedContext(browserContext, project, runLabel);
    await verifyCatalogSearch(page, options, browserContext.name, browserContext.id);
    await verifyResponsiveDetailLayout(page, options, browserContext.id);
    browserContext = await deleteContextThroughUi(page, accessToken, options, browserContext.id);
    assertEqual(browserContext.state, 'deleted', 'deleted browser context state');

    console.log(JSON.stringify({
      browserContextId: browserContext.id,
      browserContextName: browserContext.name,
      projectId: project.id,
    }, null, 2));
  } finally {
    if (accessToken && browserContext?.id && browserContext.state !== 'deleted') {
      await deleteBrowserContext(accessToken, options, browserContext.id).catch((error) => {
        log(`Browser context cleanup for ${browserContext.id} failed: ${error.message}`);
      });
    }
    if (accessToken && project) {
      await archiveProject(accessToken, options, project).catch((error) => {
        log(`Project cleanup for ${project.id} failed: ${error.message}`);
      });
    }
    await context.close();
    await browser.close();
  }
}

async function createContextThroughUi(page, accessToken, options, project, runLabel) {
  await page.goto(adminRouteUrl(options, 'browser-contexts/new'), { waitUntil: 'domcontentloaded' });
  await page.getByTestId('browser-context-create-route').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await assertNoBodyHorizontalOverflow(page, 'unified browser-context create route');
  await assertNoHorizontalOverflow(page, 'browser-context-create-route', 'unified browser-context create route');
  await verifyCreateValidation(page);

  await page.goto(adminRouteUrl(options, 'browser-contexts/new'), { waitUntil: 'domcontentloaded' });
  await page.getByTestId('browser-context-create-route').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await page.getByTestId('browser-context-edit-name').fill(`Unified browser context ${runLabel}`);
  await page.getByTestId('browser-context-edit-description').fill('Created by the unified admin browser-context smoke.');
  await page.getByTestId('browser-context-edit-labels').fill(`suite=admin-unified-browser-contexts-smoke\nrun=${runLabel}`);
  await page.getByTestId('browser-context-edit-project-binding').selectOption('project');
  await page.locator(`[data-testid="browser-context-edit-project-id"] option[value="${project.id}"]`).waitFor({
    state: 'attached',
    timeout: options.connectTimeoutMs,
  });
  await page.getByTestId('browser-context-edit-project-id').selectOption(project.id);
  await page.getByTestId('browser-context-edit-retention-sec').fill('86400');
  await page.getByTestId('browser-context-edit-storage-limit-enabled').setChecked(true);
  await page.getByTestId('browser-context-edit-max-profile-storage-bytes').fill('67108864');
  await assertNoBodyHorizontalOverflow(page, 'unified browser-context populated create route');
  await assertNoHorizontalOverflow(page, 'browser-context-edit-form', 'unified browser-context edit form');

  if (!await page.getByTestId('browser-context-edit-save').isEnabled()) {
    throw new Error('Expected browser-context save to be enabled after all required fields were filled.');
  }
  await page.getByTestId('browser-context-edit-save').click();

  const contextId = await waitForContextDetailNavigation(page, options);
  await page.getByTestId('browser-context-detail-route').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  return await fetchBrowserContext(accessToken, options, contextId);
}

async function verifyCreateValidation(page) {
  if (!await page.getByTestId('browser-context-edit-save').isDisabled()) {
    throw new Error('Expected new browser-context save to be disabled before required fields are entered.');
  }
  await page.getByTestId('browser-context-edit-name').fill('Invalid context');
  await page.getByTestId('browser-context-edit-labels').fill('broken-label');
  const labelsError = await page.getByTestId('browser-context-edit-labels-error').textContent();
  if (!labelsError?.includes('key=value')) {
    throw new Error(`Expected label validation error, got ${labelsError}`);
  }
  if (!await page.getByTestId('browser-context-edit-save').isDisabled()) {
    throw new Error('Expected save to stay disabled while labels are invalid.');
  }

  await page.getByTestId('browser-context-edit-labels').fill('');
  await page.getByTestId('browser-context-edit-project-binding').selectOption('project');
  const projectError = await page.getByTestId('browser-context-edit-project-id-error').textContent();
  if (!projectError?.includes('Project-scoped contexts need a project')) {
    throw new Error(`Expected project binding validation error, got ${projectError}`);
  }
  await page.getByTestId('browser-context-edit-project-binding').selectOption('owner');
  await page.getByTestId('browser-context-edit-retention-sec').fill('0');
  const retentionError = await page.getByTestId('browser-context-edit-retention-sec-error').textContent();
  if (!retentionError?.includes('greater than zero')) {
    throw new Error(`Expected retention validation error, got ${retentionError}`);
  }
  if (!await page.getByTestId('browser-context-edit-save').isDisabled()) {
    throw new Error('Expected save to stay disabled while retention is invalid.');
  }
  await assertNoBodyHorizontalOverflow(page, 'unified browser-context validation messages');
  await assertNoHorizontalOverflow(page, 'browser-context-edit-form', 'unified browser-context validation form');
}

async function verifyCatalogSearch(page, options, contextName, contextId) {
  await page.goto(adminRouteUrl(options, 'browser-contexts'), { waitUntil: 'domcontentloaded' });
  await page.getByTestId('browser-contexts-new-link').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await page.getByTestId('browser-contexts-search').fill(contextName);
  const row = page.locator('[data-testid="browser-contexts-list-row"]').filter({ hasText: contextName });
  await row.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  const link = row.getByTestId('browser-contexts-detail-link');
  const href = await link.getAttribute('href');
  if (!href?.endsWith(`/admin-new/browser-contexts/${encodeURIComponent(contextId)}`)) {
    throw new Error(`Expected browser-context catalog detail link for ${contextId}, got ${href}`);
  }
  await assertNoBodyHorizontalOverflow(page, 'unified browser-context filtered catalog');
}

async function verifyResponsiveDetailLayout(page, options, contextId) {
  await page.setViewportSize({ width: 390, height: 900 });
  await page.goto(adminRouteUrl(options, `browser-contexts/${contextId}`), { waitUntil: 'domcontentloaded' });
  await page.getByTestId('browser-context-detail-route').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await page.getByTestId('browser-context-inspector').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await assertNoBodyHorizontalOverflow(page, 'mobile unified browser-context detail route');
  await assertNoHorizontalOverflow(page, 'browser-context-detail-route', 'mobile unified browser-context detail route');
  await assertNoHorizontalOverflow(page, 'browser-context-inspector', 'mobile unified browser-context inspector');
  await page.setViewportSize({ width: 1440, height: 980 });
}

async function deleteContextThroughUi(page, accessToken, options, contextId) {
  await page.goto(adminRouteUrl(options, `browser-contexts/${contextId}`), { waitUntil: 'domcontentloaded' });
  await page.getByTestId('browser-context-detail-route').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  if (!await page.getByTestId('browser-context-delete').isEnabled()) {
    throw new Error('Expected browser-context delete to be enabled for an unreferenced context.');
  }
  await page.getByTestId('browser-context-refresh-detail').click();
  await page.getByTestId('browser-context-action-success').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await page.getByTestId('browser-context-delete').click();
  await page.getByTestId('browser-context-action-success').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await page.waitForFunction(() =>
    document.querySelector('[data-testid="browser-context-detail-state"]')?.textContent?.includes('deleted') === true,
    null,
    { timeout: options.connectTimeoutMs },
  );
  return await fetchBrowserContext(accessToken, options, contextId);
}

async function waitForContextDetailNavigation(page, options) {
  await page.waitForURL((url) => {
    const expectedPrefix = new URL('/admin-new/browser-contexts/', apiOrigin(options)).toString();
    return url.toString().startsWith(expectedPrefix) && !url.pathname.endsWith('/new');
  }, { timeout: options.connectTimeoutMs });
  const contextId = decodeURIComponent(new URL(page.url()).pathname.split('/').filter(Boolean).at(-1) ?? '');
  if (!contextId || contextId === 'new') {
    throw new Error(`Expected create flow to navigate to browser-context detail route, got ${page.url()}`);
  }
  return contextId;
}

async function createProject(accessToken, options, runLabel) {
  return await fetchJson(`${apiOrigin(options)}/api/v1/projects`, {
    method: 'POST',
    headers: authJsonHeaders(accessToken),
    body: JSON.stringify({
      name: `Unified browser-context smoke project ${runLabel}`,
      description: 'Project binding target for the unified admin browser-context smoke.',
      labels: { suite: 'admin-unified-browser-contexts-smoke', run: runLabel },
      state: 'active',
      policy: {
        allowed_session_template_ids: [],
        allowed_egress_profile_ids: [],
        allowed_extension_ids: [],
        allowed_browser_context_ids: [],
        allowed_file_workspace_ids: [],
        allow_browser_uploads: true,
        allow_browser_downloads: true,
        allow_session_file_bindings: true,
        allow_manual_recordings: true,
        usage_budget_enforcement: 'warning_only',
      },
      quotas: {},
    }),
  });
}

async function fetchBrowserContext(accessToken, options, contextId) {
  return await fetchJson(`${apiOrigin(options)}/api/v1/browser-contexts/${contextId}`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function deleteBrowserContext(accessToken, options, contextId) {
  return await fetchJson(`${apiOrigin(options)}/api/v1/browser-contexts/${contextId}`, {
    method: 'DELETE',
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function archiveProject(accessToken, options, project) {
  return await fetchJson(`${apiOrigin(options)}/api/v1/projects/${project.id}`, {
    method: 'PUT',
    headers: authJsonHeaders(accessToken),
    body: JSON.stringify({
      name: project.name,
      description: project.description,
      labels: project.labels ?? {},
      quotas: project.quotas ?? {},
      policy: project.policy ?? {},
      state: 'archived',
    }),
  });
}

function verifyCreatedContext(context, project, runLabel) {
  assertEqual(context.name, `Unified browser context ${runLabel}`, 'browser context name');
  assertEqual(context.description, 'Created by the unified admin browser-context smoke.', 'browser context description');
  assertEqual(context.project_id, project.id, 'browser context project id');
  assertEqual(context.persistence_mode, 'reusable', 'browser context persistence mode');
  assertEqual(context.retention_sec, 86400, 'browser context retention seconds');
  assertEqual(context.max_profile_storage_bytes, 67108864, 'browser context storage limit');
  assertEqual(context.labels?.suite, 'admin-unified-browser-contexts-smoke', 'browser context suite label');
  assertEqual(context.labels?.run, runLabel, 'browser context run label');
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

function adminRouteUrl(options, path) {
  return new URL(`/admin-new/${path.replace(/^\/+/, '')}`, apiOrigin(options)).toString();
}

function authJsonHeaders(accessToken) {
  return {
    Authorization: `Bearer ${accessToken}`,
    'content-type': 'application/json',
  };
}

function assertEqual(actual, expected, label) {
  if (actual !== expected) {
    throw new Error(`Expected ${label} to be ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`);
  }
}

run().catch((error) => {
  console.error(`[admin-unified-browser-contexts-smoke] ${error.stack || error.message}`);
  process.exit(1);
});
