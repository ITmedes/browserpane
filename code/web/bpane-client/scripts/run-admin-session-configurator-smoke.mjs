import fs from 'node:fs/promises';
import process from 'node:process';
import { chromium } from 'playwright-core';
import {
  cleanupAdminBeforeRun,
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
  const options = parseSmokeArgs(process.argv.slice(2), 'run-admin-session-configurator-smoke.mjs');
  if (options.pageUrl === DEFAULTS.pageUrl) {
    options.pageUrl = `${DEFAULTS.pageUrl}/admin/`;
  }
  const log = createLogger('admin-session-configurator-smoke');
  const browser = await launchChrome(chromium, options);
  const context = await browser.newContext({ viewport: { width: 1440, height: 980 } });
  const page = await context.newPage();
  let sessionId = '';
  let template = null;

  try {
    log(`Opening ${options.pageUrl}`);
    await ensureAdminLoggedIn(page, options);
    await cleanupAdminBeforeRun(page, options, log);
    template = await createSessionTemplate(page, options);

    await verifyCompactPayloadToggle(page, options, template);

    await page.goto(adminRouteUrl(options, 'sessions'), { waitUntil: 'domcontentloaded' });
    await page.getByTestId('session-create-configurator').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });

    await verifyClientValidation(page);
    await configureTemplatedSession(page, options, template);
    await verifyPayloadPreview(page, template);
    await page.getByTestId('session-inspector-new').click();
    sessionId = await waitForSessionDetailUrl(page, options);

    const session = await fetchSession(page, options, sessionId);
    verifyCreatedSession(session, sessionId, template);
    await verifyDetailUi(page, options, sessionId, template);
    await verifyInspectorTemplateFilter(page, options, sessionId, template);
    await emitSummary(page, options, session, template, log);
  } finally {
    await cleanupCreatedSession(page, options, sessionId, log);
    await context.close();
    await browser.close();
  }
}

async function verifyCompactPayloadToggle(page, options, template) {
  await page.goto(options.pageUrl, { waitUntil: 'domcontentloaded' });
  await openAdminTab(page, 'sessions');
  await selectSessionTemplate(page, template, options);
  await assertNoHorizontalOverflow(page, 'session-create-configurator', 'live session create configurator');
  const templateSummary = await page.getByTestId('session-create-template-summary').textContent();
  if (!templateSummary?.includes(template.name) || !templateSummary.includes('team=support')) {
    throw new Error(`Expected live configurator template summary for ${template.name}, got ${templateSummary}`);
  }
  const toggle = page.getByTestId('session-create-preview-toggle');
  await toggle.click();
  await page.getByTestId('session-create-preview').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  const previewText = await page.getByTestId('session-create-preview').textContent();
  const preview = JSON.parse(previewText ?? '{}');
  if (preview.template_id !== template.id) {
    throw new Error(`Expected live configurator preview to include template_id ${template.id}, got ${previewText}`);
  }
  if (Object.hasOwn(preview, 'owner_mode')) {
    throw new Error(`Expected template default owner mode to remain unoverridden, got ${previewText}`);
  }
  await page.waitForTimeout(1500);
  const stillVisible = await page.getByTestId('session-create-preview').isVisible().catch(() => false);
  if (!stillVisible) {
    throw new Error('Expected compact API payload preview to stay expanded after opening.');
  }
  await toggle.click();
}

async function verifyClientValidation(page) {
  await page.getByTestId('session-create-idle-timeout').fill('0');
  await page.getByTestId('session-create-labels').fill('case=1234\ncase=5678');
  const disabled = await page.getByTestId('session-inspector-new').isDisabled();
  if (!disabled) {
    throw new Error('Expected configured session create to be disabled for invalid idle timeout and duplicate labels.');
  }
  const errorText = await page.getByTestId('session-create-error').textContent();
  if (!errorText?.includes('Idle timeout') || !errorText.includes('duplicated')) {
    throw new Error(`Expected validation errors for idle timeout and duplicate labels, got ${errorText}`);
  }
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

async function configureTemplatedSession(page, options, template) {
  await selectSessionTemplate(page, template, options);
  await page.getByTestId('session-create-owner-mode').selectOption('collaborative');
  await page.getByTestId('session-create-owner-mode').selectOption('');
  await page.getByTestId('session-create-idle-timeout').fill('');
  await page.getByTestId('session-create-labels').fill('case=1234\npurpose=import-repro');
}

async function verifyPayloadPreview(page, template) {
  const previewText = await page.getByTestId('session-create-preview').textContent();
  const preview = JSON.parse(previewText ?? '{}');
  const expectedLabels = { case: '1234', purpose: 'import-repro' };
  if (
    preview.template_id !== template.id
    || Object.hasOwn(preview, 'owner_mode')
    || Object.hasOwn(preview, 'idle_timeout_sec')
    || JSON.stringify(preview.labels) !== JSON.stringify(expectedLabels)
  ) {
    throw new Error(`Unexpected session create payload preview: ${previewText}`);
  }
}

async function verifyDetailUi(page, options, sessionId, template) {
  await page.getByTestId('session-inspector-detail').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  const title = await page.getByTestId('session-inspector-title').textContent();
  if (!title?.includes(sessionId)) {
    throw new Error(`Expected session detail title to include ${sessionId}, got ${title}`);
  }
  await page.getByTestId('session-owner-mode').waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  const ownerMode = await page.getByTestId('session-owner-mode').textContent();
  const idleTimeout = await page.getByTestId('session-idle-timeout').textContent();
  const detailTemplate = await page.getByTestId('session-template').textContent();
  const labels = await page.getByTestId('session-labels').textContent();
  const integration = await page.getByTestId('session-integration-context').textContent();
  if (
    ownerMode !== 'collaborative'
    || idleTimeout !== '1200'
    || !detailTemplate?.includes(template.name)
    || !labels?.includes('purpose=import-repro')
    || !labels.includes('team=support')
    || !integration?.includes('source=admin-smoke-template')
  ) {
    throw new Error(`Unexpected configured session detail facts: ${ownerMode} / ${idleTimeout} / ${detailTemplate} / ${labels} / ${integration}`);
  }
  await page.getByTestId('session-file-bindings').waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
}

async function verifyInspectorTemplateFilter(page, options, sessionId, template) {
  await page.goto(adminRouteUrl(options, 'sessions'), { waitUntil: 'domcontentloaded' });
  await page.getByTestId('session-inspector-list').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await selectOptionWhenAvailable(page, page.getByTestId('session-inspector-template-filter'), template.id, options);
  const row = page.locator(`[data-testid="session-inspector-row"][data-session-id="${sessionId}"]`);
  await row.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  const rowTemplate = await row.getByTestId('session-inspector-row-template').textContent();
  if (!rowTemplate?.includes(template.name)) {
    throw new Error(`Expected inspector row template ${template.name}, got ${rowTemplate}`);
  }
  const count = await page.getByTestId('session-inspector-count').textContent();
  if (!count?.includes('visible sessions')) {
    throw new Error(`Expected inspector filter count to be visible, got ${count}`);
  }

  await page.goto(options.pageUrl, { waitUntil: 'domcontentloaded' });
  await openAdminTab(page, 'sessions');
  const liveRow = page.locator(`[data-testid="session-row"][data-session-id="${sessionId}"]`);
  await liveRow.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  await liveRow.click();
  const selectedTemplate = await page.getByTestId('session-selected-template').textContent();
  if (!selectedTemplate?.includes(template.name)) {
    throw new Error(`Expected live selected session template ${template.name}, got ${selectedTemplate}`);
  }
}

function verifyCreatedSession(session, sessionId, template) {
  if (session.id !== sessionId) {
    throw new Error(`Expected API session ${sessionId}, got ${session.id}`);
  }
  if (session.template_id !== template.id) {
    throw new Error(`Expected template_id ${template.id}, got ${session.template_id}`);
  }
  if (session.owner_mode !== 'collaborative') {
    throw new Error(`Expected collaborative owner mode, got ${session.owner_mode}`);
  }
  if (session.idle_timeout_sec !== 1200) {
    throw new Error(`Expected template idle timeout 1200, got ${session.idle_timeout_sec}`);
  }
  if (session.labels?.case !== '1234' || session.labels?.purpose !== 'import-repro' || session.labels?.team !== 'support') {
    throw new Error(`Expected configured labels, got ${JSON.stringify(session.labels)}`);
  }
  if (session.integration_context?.source !== 'admin-smoke-template') {
    throw new Error(`Expected template integration context, got ${JSON.stringify(session.integration_context)}`);
  }
}

async function createSessionTemplate(page, options) {
  const accessToken = await getAdminAccessToken(page);
  const name = `Admin smoke template ${Date.now()}`;
  return await fetchJson(`${apiOrigin(options)}/api/v1/session-templates`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'content-type': 'application/json',
    },
    body: JSON.stringify({
      name,
      description: 'Admin configurator smoke template',
      labels: { suite: 'admin-session-configurator-smoke' },
      defaults: {
        owner_mode: 'collaborative',
        idle_timeout_sec: 1200,
        labels: { team: 'support' },
        integration_context: { source: 'admin-smoke-template' },
      },
    }),
  });
}

async function selectSessionTemplate(page, template, options) {
  const selector = page.getByTestId('session-create-template');
  await selectOptionWhenAvailable(page, selector, template.id, options);
}

async function selectOptionWhenAvailable(page, selector, value, options) {
  await selector.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  await poll(
    `select option ${value}`,
    async () => await selector.locator(`option[value="${value}"]`).count(),
    (count) => count > 0,
    options.connectTimeoutMs,
    100,
  );
  await selector.selectOption(value);
}

async function fetchSession(page, options, sessionId) {
  const accessToken = await getAdminAccessToken(page);
  return await fetchJson(`${apiOrigin(options)}/api/v1/sessions/${sessionId}`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function cleanupCreatedSession(page, options, sessionId, log) {
  if (!sessionId) {
    return;
  }
  const accessToken = await getAdminAccessToken(page).catch(() => '');
  if (!accessToken) {
    log(`Skipped cleanup for ${sessionId}; no admin access token is available.`);
    return;
  }
  await deleteSession(accessToken, options, sessionId);
}

async function waitForSessionDetailUrl(page, options) {
  await page.waitForURL(/\/sessions\/[^/]+$/, { timeout: options.connectTimeoutMs });
  const sessionId = decodeURIComponent(new URL(page.url()).pathname.split('/').filter(Boolean).at(-1) ?? '');
  if (!sessionId) {
    throw new Error(`Could not resolve session id from ${page.url()}`);
  }
  return sessionId;
}

async function emitSummary(page, options, session, template, log) {
  const summary = {
    pageUrl: options.pageUrl,
    sessionId: session.id,
    templateId: template.id,
    templateName: template.name,
    ownerMode: session.owner_mode,
    idleTimeoutSec: session.idle_timeout_sec,
    labels: session.labels ?? {},
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
  console.error(`[admin-session-configurator-smoke] ${error.stack || error.message}`);
  process.exitCode = 1;
});
