import process from 'node:process';
import { chromium } from 'playwright-core';
import {
  ensureAdminLoggedIn,
  getAdminAccessToken,
  openAdminTab,
} from './admin-smoke-lib.mjs';
import {
  DEFAULTS,
  apiOrigin,
  createLogger,
  deleteSession,
  fetchJson,
  launchChrome,
  parseSmokeArgs,
  poll,
} from './workflow-smoke-lib.mjs';

async function run() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-admin-browser-contexts-smoke.mjs');
  if (options.pageUrl === DEFAULTS.pageUrl) {
    options.pageUrl = `${DEFAULTS.pageUrl}/admin/`;
  }
  const log = createLogger('admin-browser-contexts-smoke');
  const browser = await launchChrome(chromium, options);
  const context = await browser.newContext({ viewport: { width: 1440, height: 980 } });
  const page = await context.newPage();
  let referencedContext = null;
  let deletableContext = null;
  let clonedContextId = '';
  let sessionId = '';

  try {
    log(`Opening ${options.pageUrl}`);
    await ensureAdminLoggedIn(page, options);
    const accessToken = await getAdminAccessToken(page);
    referencedContext = await createBrowserContext(accessToken, options, `Referenced context ${Date.now()}`);
    deletableContext = await createBrowserContext(accessToken, options, `Deletable context ${Date.now()}`);
    const session = await createSession(accessToken, options, referencedContext.id);
    sessionId = session.id;

    await verifyLiveCatalog(page, options, referencedContext, sessionId);
    clonedContextId = await verifyRouteCatalog(page, options, accessToken, referencedContext, deletableContext);

    const deleted = await fetchJson(`${apiOrigin(options)}/api/v1/browser-contexts/${deletableContext.id}`, {
      headers: { Authorization: `Bearer ${accessToken}` },
    });
    if (deleted.state !== 'deleted') {
      throw new Error(`Expected route delete to soft-delete ${deletableContext.id}, got ${JSON.stringify(deleted)}`);
    }

    console.log(JSON.stringify({
      referencedContextId: referencedContext.id,
      deletedContextId: deletableContext.id,
      clonedContextId,
      sessionId,
    }, null, 2));
  } finally {
    const accessToken = await getAdminAccessToken(page).catch(() => '');
    if (accessToken && sessionId) {
      await deleteSession(accessToken, options, sessionId).catch((error) => {
        log(`Session cleanup for ${sessionId} failed: ${error.message}`);
      });
    }
    if (accessToken && referencedContext?.id) {
      await fetchJson(`${apiOrigin(options)}/api/v1/browser-contexts/${referencedContext.id}`, {
        method: 'DELETE',
        headers: { Authorization: `Bearer ${accessToken}` },
      }).catch((error) => {
        log(`Browser context cleanup for ${referencedContext.id} failed: ${error.message}`);
      });
    }
    if (accessToken && clonedContextId) {
      await fetchJson(`${apiOrigin(options)}/api/v1/browser-contexts/${clonedContextId}`, {
        method: 'DELETE',
        headers: { Authorization: `Bearer ${accessToken}` },
      }).catch((error) => {
        log(`Browser context cleanup for clone ${clonedContextId} failed: ${error.message}`);
      });
    }
    await context.close();
    await browser.close();
  }
}

async function verifyLiveCatalog(page, options, browserContext, sessionId) {
  await page.goto(options.pageUrl, { waitUntil: 'domcontentloaded' });
  await openAdminTab(page, 'sessions');
  await page.getByTestId('session-refresh').click();
  await page.locator(`[data-testid="session-row"][data-session-id="${sessionId}"]`).waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await openAdminTab(page, 'contexts');
  await page.getByTestId('browser-context-catalog').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await page.getByTestId('browser-context-search').fill(browserContext.name);
  const row = page.locator(`[data-testid="browser-context-row"][data-context-id="${browserContext.id}"]`);
  await row.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  await row.click();
  const detailName = await page.getByTestId('browser-context-detail-name').textContent();
  const references = await poll(
    'live browser context reference count',
    async () => await page.getByTestId('browser-context-detail-references').textContent(),
    (value) => value?.includes('1 visible session') === true,
    options.connectTimeoutMs,
    100,
  );
  const retention = await page.getByTestId('browser-context-detail-retention').textContent();
  const storageLimit = await page.getByTestId('browser-context-detail-storage-limit').textContent();
  const apiExample = await page.getByTestId('browser-context-api-example').textContent();
  const deleteDisabled = await page.getByTestId('browser-context-delete').isDisabled();
  if (!detailName?.includes(browserContext.name)) {
    throw new Error(`Expected live catalog selected context ${browserContext.name}, got ${detailName}`);
  }
  if (!apiExample?.includes(browserContext.id) || !apiExample.includes('POST /api/v1/sessions')) {
    throw new Error(`Expected live catalog API example for ${browserContext.id}, got ${apiExample}`);
  }
  if (!retention?.includes('7 days')) {
    throw new Error(`Expected live catalog retention summary for ${browserContext.id}, got ${retention}`);
  }
  if (!storageLimit?.includes('67.1 MB')) {
    throw new Error(`Expected live catalog storage-limit summary for ${browserContext.id}, got ${storageLimit}`);
  }
  if (!deleteDisabled) {
    throw new Error('Expected delete to be disabled while a visible session references the context.');
  }
}

async function verifyRouteCatalog(page, options, accessToken, referencedContext, deletableContext) {
  await page.goto(adminRouteUrl(options, 'browser-contexts'), { waitUntil: 'domcontentloaded' });
  await page.getByTestId('browser-context-route').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });

  await page.getByTestId('browser-context-search').fill(referencedContext.name);
  const referencedRow = page.locator(`[data-testid="browser-context-row"][data-context-id="${referencedContext.id}"]`);
  await referencedRow.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  await referencedRow.click();
  if (!await page.getByTestId('browser-context-delete').isDisabled()) {
    throw new Error('Expected route delete to be disabled for referenced context.');
  }

  await page.getByTestId('browser-context-search').fill(deletableContext.name);
  const deletableRow = page.locator(`[data-testid="browser-context-row"][data-context-id="${deletableContext.id}"]`);
  await deletableRow.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  await deletableRow.click();
  const cloneName = `${deletableContext.name} clone`;
  await page.getByTestId('browser-context-clone-name').fill(cloneName);
  await page.getByTestId('browser-context-clone').click();
  await page.getByTestId('browser-context-clone-message').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  const listed = await fetchJson(`${apiOrigin(options)}/api/v1/browser-contexts`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
  const cloned = listed.contexts?.find((context) => context.name === cloneName);
  if (!cloned?.id || cloned.id === deletableContext.id) {
    throw new Error(`Expected browser context clone ${cloneName}, got ${JSON.stringify(listed)}`);
  }
  await page.getByTestId('browser-context-search').fill(cloneName);
  await page.locator(`[data-testid="browser-context-row"][data-context-id="${cloned.id}"]`).waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await page.getByTestId('browser-context-search').fill(deletableContext.name);
  await deletableRow.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  await deletableRow.click();
  await poll(
    'browser context delete enabled',
    async () => await page.getByTestId('browser-context-delete').isEnabled(),
    Boolean,
    options.connectTimeoutMs,
    100,
  );
  await page.getByTestId('browser-context-delete').click();
  await poll(
    'browser context deleted state',
    async () => await page.getByTestId('browser-context-detail-state').textContent(),
    (state) => state === 'deleted',
    options.connectTimeoutMs,
    100,
  );
  return cloned.id;
}

async function createBrowserContext(accessToken, options, name) {
  return await fetchJson(`${apiOrigin(options)}/api/v1/browser-contexts`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'content-type': 'application/json',
    },
    body: JSON.stringify({
      name,
      description: 'Admin browser context catalog smoke',
      labels: { suite: 'admin-browser-contexts-smoke' },
      retention_sec: 604800,
      max_profile_storage_bytes: 67108864,
    }),
  });
}

async function createSession(accessToken, options, contextId) {
  return await fetchJson(`${apiOrigin(options)}/api/v1/sessions`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'content-type': 'application/json',
    },
    body: JSON.stringify({
      browser_context: {
        mode: 'reusable',
        context_id: contextId,
      },
      labels: { suite: 'admin-browser-contexts-smoke' },
    }),
  });
}

function adminRouteUrl(options, routePath) {
  const baseUrl = new URL(options.pageUrl);
  if (!baseUrl.pathname.endsWith('/')) {
    baseUrl.pathname = `${baseUrl.pathname}/`;
  }
  return new URL(routePath, baseUrl).toString();
}

run().catch((error) => {
  console.error(`[admin-browser-contexts-smoke] ${error.stack || error.message}`);
  process.exitCode = 1;
});
