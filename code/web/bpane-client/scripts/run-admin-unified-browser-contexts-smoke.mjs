import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';
import { chromium } from 'playwright-core';
import {
  ensureAdminLoggedIn,
  getAdminAccessToken,
} from './admin-smoke-lib.mjs';
import {
  adminRouteUrl,
  assertEqual,
  assertNoBodyHorizontalOverflow,
  assertNoHorizontalOverflow,
  authJsonHeaders,
} from './admin-unified-smoke-lib.mjs';
import {
  DEFAULTS,
  apiOrigin,
  createLogger,
  deleteSession,
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
  const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), 'bpane-admin-new-context-lifecycle-'));
  const runLabel = `admin-unified-contexts-smoke-${Date.now()}`;
  let accessToken = '';
  let project = null;
  let browserContext = null;
  let clonedContext = null;
  let importedContext = null;
  let boundSession = null;

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
    const lifecycle = await verifyLifecycleThroughUi(
      page,
      accessToken,
      options,
      project,
      browserContext,
      runLabel,
      tempDir,
    );
    clonedContext = lifecycle.clonedContext;
    importedContext = lifecycle.importedContext;
    boundSession = await createSession(accessToken, options, project.id, importedContext.id, runLabel);
    assertEqual(boundSession.project_id, project.id, 'imported context session project binding');
    assertEqual(boundSession.browser_context?.context_id, importedContext.id, 'imported context session binding');
    browserContext = await deleteContextThroughUi(page, accessToken, options, browserContext.id);
    assertEqual(browserContext.state, 'deleted', 'deleted browser context state');

    console.log(JSON.stringify({
      browserContextId: browserContext.id,
      browserContextName: browserContext.name,
      clonedContextId: clonedContext.id,
      importedContextId: importedContext.id,
      boundSessionId: boundSession.id,
      projectId: project.id,
    }, null, 2));
  } finally {
    if (accessToken && boundSession?.id) {
      await deleteSession(accessToken, options, boundSession.id).catch((error) => {
        log(`Session cleanup for ${boundSession.id} failed: ${error.message}`);
      });
    }
    for (const cleanupContext of [importedContext, clonedContext, browserContext]) {
      if (!accessToken || !cleanupContext?.id || cleanupContext.state === 'deleted') {
        continue;
      }
      await deleteBrowserContext(accessToken, options, cleanupContext.id).catch((error) => {
        log(`Browser context cleanup for ${cleanupContext.id} failed: ${error.message}`);
      });
    }
    if (accessToken && project) {
      await archiveProject(accessToken, options, project).catch((error) => {
        log(`Project cleanup for ${project.id} failed: ${error.message}`);
      });
    }
    await fs.rm(tempDir, { recursive: true, force: true }).catch(() => undefined);
    await context.close();
    await browser.close();
  }
}

async function verifyLifecycleThroughUi(page, accessToken, options, project, sourceContext, runLabel, tempDir) {
  const cloneName = `Unified context clone ${runLabel}`;
  await page.goto(adminRouteUrl(options, `browser-contexts/${sourceContext.id}`), {
    waitUntil: 'domcontentloaded',
  });
  await page.getByTestId('browser-context-clone').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  if (!await page.getByTestId('browser-context-export').isEnabled()) {
    throw new Error('Expected export to be enabled for an inactive reusable context.');
  }
  await page.getByTestId('browser-context-clone').click();
  await page.getByTestId('browser-context-clone-route').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await page.getByTestId('browser-context-edit-name').fill(cloneName);
  await assertNoBodyHorizontalOverflow(page, 'unified browser-context clone route');
  await assertNoHorizontalOverflow(page, 'browser-context-clone-route', 'unified browser-context clone route');
  await verifyLifecycleFormMobileLayout(page, 'browser-context-clone-route', 'clone');
  await page.getByTestId('browser-context-edit-save').click();
  const clonedContextId = await waitForContextDetailNavigation(page, options);
  const clonedContext = await fetchBrowserContext(accessToken, options, clonedContextId);
  assertEqual(clonedContext.name, cloneName, 'cloned browser context name');
  assertEqual(clonedContext.project_id, project.id, 'cloned browser context project id');
  assertEqual(clonedContext.persistence_mode, 'reusable', 'cloned browser context persistence mode');

  await page.goto(adminRouteUrl(options, `browser-contexts/${sourceContext.id}`), {
    waitUntil: 'domcontentloaded',
  });
  await page.getByTestId('browser-context-export').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  const downloadPromise = page.waitForEvent('download', { timeout: options.connectTimeoutMs });
  await page.getByTestId('browser-context-export').click();
  const download = await downloadPromise;
  if (!download.suggestedFilename().endsWith('.zip')) {
    throw new Error(`Expected browser-context zip export, got ${download.suggestedFilename()}`);
  }
  const exportPath = path.join(tempDir, download.suggestedFilename());
  await download.saveAs(exportPath);
  const exportStat = await fs.stat(exportPath);
  if (exportStat.size === 0) {
    throw new Error('Expected browser-context export archive to contain bytes.');
  }
  await page.getByTestId('browser-context-action-success').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });

  const importName = `Unified context import ${runLabel}`;
  await page.goto(adminRouteUrl(options, 'browser-contexts/import'), { waitUntil: 'domcontentloaded' });
  await page.getByTestId('browser-context-import-route').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await page.getByTestId('browser-context-import-file').setInputFiles(exportPath);
  await page.getByTestId('browser-context-edit-name').fill(importName);
  await page.getByTestId('browser-context-edit-project-binding').selectOption('project');
  await page.locator(`[data-testid="browser-context-edit-project-id"] option[value="${project.id}"]`).waitFor({
    state: 'attached',
    timeout: options.connectTimeoutMs,
  });
  await page.getByTestId('browser-context-edit-project-id').selectOption(project.id);
  await assertNoBodyHorizontalOverflow(page, 'unified browser-context import route');
  await assertNoHorizontalOverflow(page, 'browser-context-import-route', 'unified browser-context import route');
  await verifyLifecycleFormMobileLayout(page, 'browser-context-import-route', 'import');
  await page.getByTestId('browser-context-edit-save').click();
  const importedContextId = await waitForContextDetailNavigation(page, options);
  const importedContext = await fetchBrowserContext(accessToken, options, importedContextId);
  assertEqual(importedContext.name, importName, 'imported browser context name');
  assertEqual(importedContext.project_id, project.id, 'imported browser context project id');
  assertEqual(importedContext.persistence_mode, 'reusable', 'imported browser context persistence mode');

  await verifyMalformedImportRetry(page, options, tempDir, runLabel);
  return { clonedContext, importedContext };
}

async function verifyLifecycleFormMobileLayout(page, routeTestId, label) {
  await page.setViewportSize({ width: 390, height: 900 });
  await assertNoBodyHorizontalOverflow(page, `mobile unified browser-context ${label} route`);
  await assertNoHorizontalOverflow(
    page,
    routeTestId,
    `mobile unified browser-context ${label} route`,
  );
  await assertNoHorizontalOverflow(
    page,
    'browser-context-edit-form',
    `mobile unified browser-context ${label} form`,
  );
  await page.setViewportSize({ width: 1440, height: 980 });
}

async function verifyMalformedImportRetry(page, options, tempDir, runLabel) {
  const malformedPath = path.join(tempDir, 'malformed-context.zip');
  const malformedName = `Malformed import ${runLabel}`;
  await fs.writeFile(malformedPath, 'not-a-browserpane-archive', 'utf8');
  await page.goto(adminRouteUrl(options, 'browser-contexts/import'), { waitUntil: 'domcontentloaded' });
  await page.getByTestId('browser-context-import-route').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await page.getByTestId('browser-context-import-file').setInputFiles(malformedPath);
  await page.getByTestId('browser-context-edit-name').fill(malformedName);
  await page.getByTestId('browser-context-edit-save').click();
  const message = page.getByTestId('browser-context-import-error');
  await message.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  if (!(await message.textContent())?.includes('HTTP 400')) {
    throw new Error(`Expected malformed import HTTP 400 feedback, got ${await message.textContent()}`);
  }
  assertEqual(
    await page.getByTestId('browser-context-edit-name').inputValue(),
    malformedName,
    'malformed import preserved name',
  );
  if (!(await page.getByTestId('browser-context-import-file-summary').textContent())?.includes('malformed-context.zip')) {
    throw new Error('Expected malformed import archive selection to remain visible after rejection.');
  }
  if (!await page.getByTestId('browser-context-edit-save').isEnabled()) {
    throw new Error('Expected malformed import to remain retryable after rejection.');
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
    const expectedPrefix = '/admin-new/browser-contexts/';
    if (!url.pathname.startsWith(expectedPrefix)) {
      return false;
    }
    const suffix = url.pathname.slice(expectedPrefix.length).replace(/\/$/, '');
    return suffix.length > 0 && !suffix.includes('/') && !['new', 'import'].includes(suffix);
  }, { timeout: options.connectTimeoutMs });
  const contextId = decodeURIComponent(new URL(page.url()).pathname.split('/').filter(Boolean).at(-1) ?? '');
  if (!contextId || ['new', 'import', 'clone'].includes(contextId)) {
    throw new Error(`Expected create flow to navigate to browser-context detail route, got ${page.url()}`);
  }
  return contextId;
}

async function createSession(accessToken, options, projectId, contextId, runLabel) {
  return await fetchJson(`${apiOrigin(options)}/api/v1/sessions`, {
    method: 'POST',
    headers: authJsonHeaders(accessToken),
    body: JSON.stringify({
      project_id: projectId,
      browser_context: {
        mode: 'reusable',
        context_id: contextId,
      },
      labels: {
        suite: 'admin-unified-browser-contexts-smoke',
        run: runLabel,
      },
    }),
  });
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

run().catch((error) => {
  console.error(`[admin-unified-browser-contexts-smoke] ${error.stack || error.message}`);
  process.exit(1);
});
