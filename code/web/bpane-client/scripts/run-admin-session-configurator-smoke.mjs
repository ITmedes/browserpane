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

  try {
    log(`Opening ${options.pageUrl}`);
    await ensureAdminLoggedIn(page, options);
    await cleanupAdminBeforeRun(page, options, log);

    await verifyCompactPayloadToggle(page, options);

    await page.goto(adminRouteUrl(options, 'sessions'), { waitUntil: 'domcontentloaded' });
    await page.getByTestId('session-create-configurator').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });

    await verifyClientValidation(page);
    await configureCollaborativeSession(page);
    await verifyPayloadPreview(page);
    await page.getByTestId('session-inspector-new').click();
    sessionId = await waitForSessionDetailUrl(page, options);

    const session = await fetchSession(page, options, sessionId);
    verifyCreatedSession(session, sessionId);
    await verifyDetailUi(page, options, sessionId);
    await emitSummary(page, options, session, log);
  } finally {
    await cleanupCreatedSession(page, options, sessionId, log);
    await context.close();
    await browser.close();
  }
}

async function verifyCompactPayloadToggle(page, options) {
  await page.goto(options.pageUrl, { waitUntil: 'domcontentloaded' });
  await openAdminTab(page, 'sessions');
  const toggle = page.getByTestId('session-create-preview-toggle');
  await toggle.click();
  await page.getByTestId('session-create-preview').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
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

async function configureCollaborativeSession(page) {
  await page.getByTestId('session-create-owner-mode').selectOption('collaborative');
  await page.getByTestId('session-create-idle-timeout').fill('1800');
  await page.getByTestId('session-create-labels').fill('case=1234\npurpose=import-repro');
}

async function verifyPayloadPreview(page) {
  const previewText = await page.getByTestId('session-create-preview').textContent();
  const preview = JSON.parse(previewText ?? '{}');
  const expectedLabels = { case: '1234', purpose: 'import-repro' };
  if (
    preview.owner_mode !== 'collaborative'
    || preview.idle_timeout_sec !== 1800
    || JSON.stringify(preview.labels) !== JSON.stringify(expectedLabels)
  ) {
    throw new Error(`Unexpected session create payload preview: ${previewText}`);
  }
}

async function verifyDetailUi(page, options, sessionId) {
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
  const labels = await page.getByTestId('session-labels').textContent();
  if (ownerMode !== 'collaborative' || idleTimeout !== '1800' || !labels?.includes('purpose=import-repro')) {
    throw new Error(`Unexpected configured session detail facts: ${ownerMode} / ${idleTimeout} / ${labels}`);
  }
  await page.getByTestId('session-file-bindings').waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
}

function verifyCreatedSession(session, sessionId) {
  if (session.id !== sessionId) {
    throw new Error(`Expected API session ${sessionId}, got ${session.id}`);
  }
  if (session.owner_mode !== 'collaborative') {
    throw new Error(`Expected collaborative owner mode, got ${session.owner_mode}`);
  }
  if (session.idle_timeout_sec !== 1800) {
    throw new Error(`Expected idle timeout 1800, got ${session.idle_timeout_sec}`);
  }
  if (session.labels?.case !== '1234' || session.labels?.purpose !== 'import-repro') {
    throw new Error(`Expected configured labels, got ${JSON.stringify(session.labels)}`);
  }
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

async function emitSummary(page, options, session, log) {
  const summary = {
    pageUrl: options.pageUrl,
    sessionId: session.id,
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
