import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';
import { chromium } from 'playwright-core';
import { ensureAdminLoggedIn, getAdminAccessToken } from './admin-smoke-lib.mjs';
import {
  createRecordingSession,
  disconnectAndWaitForRetainedRecording,
  waitForActiveRecording,
} from './admin-recording-smoke-lib.mjs';
import {
  adminRouteUrl,
  assertNoBodyHorizontalOverflow,
  assertNoHorizontalOverflow,
  waitForContains,
} from './admin-unified-smoke-lib.mjs';
import {
  DEFAULTS,
  apiOrigin,
  createLogger,
  deleteSession,
  fetchJson,
  launchChrome,
  parseSmokeArgs,
  poll,
  sleep,
} from './workflow-smoke-lib.mjs';

async function run() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-admin-unified-recordings-smoke.mjs');
  if (options.pageUrl === DEFAULTS.pageUrl) {
    options.pageUrl = `${DEFAULTS.pageUrl}/admin-new/recordings`;
  }
  const rootUrl = apiOrigin(options);
  const log = createLogger('admin-unified-recordings-smoke');
  const browser = await launchChrome(chromium, options);
  const context = await browser.newContext({
    acceptDownloads: true,
    viewport: { width: 1440, height: 980 },
  });
  const page = await context.newPage();
  const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), 'bpane-admin-unified-recordings-'));
  let accessToken = '';
  let sessionId = '';

  try {
    log(`Opening ${options.pageUrl}`);
    await ensureAdminLoggedIn(page, options);
    accessToken = await getAdminAccessToken(page);
    if (!accessToken) {
      throw new Error('No admin access token available after login.');
    }
    await cleanupStaleRecordingSessions(accessToken, options, log);
    await waitForCatalogReady(page, options);
    await assertNoBodyHorizontalOverflow(page, 'unified recordings catalog');
    await assertNoHorizontalOverflow(page, 'recordings-overview', 'unified recordings overview');

    const session = await createRecordingSession(accessToken, rootUrl);
    sessionId = session.id;
    const popup = await connectSessionPreview(page, options, sessionId);
    await waitForActiveRecording(accessToken, rootUrl, sessionId, options.connectTimeoutMs);
    await popup.getByTestId('session-preview-viewport').click({ position: { x: 100, y: 100 } });
    await popup.mouse.wheel(0, 900);
    await sleep(2_500);
    await popup.close({ runBeforeUnload: true });
    const recording = await disconnectAndWaitForRetainedRecording(
      accessToken,
      rootUrl,
      sessionId,
      options.connectTimeoutMs * 2,
    );
    if (!recording.bytes || recording.bytes <= 1024) {
      throw new Error(`Unified retained recording was unexpectedly small (${recording.bytes ?? 0} bytes).`);
    }

    await page.goto(adminRouteUrl(options, 'recordings'), { waitUntil: 'domcontentloaded' });
    await waitForCatalogReady(page, options);
    await page.getByTestId('recordings-refresh').click();
    const row = page.getByTestId('recordings-list-row').filter({ hasText: recording.id });
    await row.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
    await waitForContains(page, options, 'recordings-list-count', 'of');
    await page.getByTestId('recordings-search').fill(recording.id);
    await poll(
      'recording search result',
      async () => await row.isVisible().catch(() => false),
      Boolean,
      options.connectTimeoutMs,
    );
    await page.getByTestId('recordings-lens-downloadable').click();
    if (await page.getByTestId('recordings-lens-downloadable').getAttribute('aria-pressed') !== 'true') {
      throw new Error('Downloadable recording lens did not become active.');
    }

    const [download] = await Promise.all([
      page.waitForEvent('download'),
      row.getByTestId('recordings-download').click(),
    ]);
    const downloadPath = path.join(tempDir, download.suggestedFilename());
    await download.saveAs(downloadPath);
    const downloaded = await fs.stat(downloadPath);
    if (downloaded.size !== recording.bytes || !download.suggestedFilename().endsWith('.webm')) {
      throw new Error(
        `Unexpected unified recording download ${download.suggestedFilename()} (${downloaded.size} bytes).`,
      );
    }
    await waitForContains(page, options, 'recordings-action-success', 'Download started');

    await page.setViewportSize({ width: 760, height: 900 });
    await assertNoBodyHorizontalOverflow(page, 'narrow unified recordings catalog');
    await assertNoHorizontalOverflow(page, 'recordings-overview', 'narrow unified recordings overview');

    console.log(JSON.stringify({
      sessionId,
      recordingId: recording.id,
      downloadedBytes: downloaded.size,
      responsive: true,
    }, null, 2));
  } finally {
    if (accessToken && sessionId) {
      await deleteSession(accessToken, options, sessionId).catch((error) => {
        log(`cleanup warning: failed to remove ${sessionId}: ${error instanceof Error ? error.message : String(error)}`);
      });
    }
    await fs.rm(tempDir, { recursive: true, force: true });
    await context.close();
    await browser.close();
  }
}

async function connectSessionPreview(page, options, sessionId) {
  await page.goto(adminRouteUrl(options, `sessions/${sessionId}`), {
    waitUntil: 'domcontentloaded',
  });
  await page.getByTestId('session-detail-route').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await poll(
    'recording session preview action',
    async () => {
      const action = page.getByTestId('session-connect-preview');
      if (await action.isEnabled().catch(() => false)) return true;
      await page.getByTestId('session-detail-refresh').click().catch(() => {});
      return false;
    },
    Boolean,
    options.connectTimeoutMs,
    500,
  );
  const popupPromise = page.waitForEvent('popup');
  await page.getByTestId('session-connect-preview').click();
  const popup = await popupPromise;
  await popup.getByTestId('session-preview-route').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  const status = await poll(
    'recording session preview connection',
    async () => await popup.getByTestId('session-preview-status').textContent().catch(() => ''),
    (value) => value === 'Connected' || value === 'Connection failed',
    options.connectTimeoutMs,
  );
  if (status !== 'Connected') {
    const detail = await popup.getByTestId('session-preview-error').textContent().catch(() => '');
    throw new Error(`Unified recording preview failed to connect${detail ? `: ${detail}` : ''}`);
  }
  return popup;
}

async function waitForCatalogReady(page, options) {
  await page.getByTestId('recordings-overview').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  const state = await poll(
    'unified recording catalog result',
    async () => ({
      ready: await page.getByTestId('recordings-refresh').isEnabled().catch(() => false),
      error: (await page.getByTestId('recordings-error').allTextContents()).join(' '),
    }),
    (value) => value.ready || Boolean(value.error),
    options.connectTimeoutMs,
  );
  if (state.error) {
    throw new Error(`Unified recording catalog failed: ${state.error}`);
  }
}

async function cleanupStaleRecordingSessions(accessToken, options, log) {
  const catalog = await fetchJson(`${apiOrigin(options)}/api/v1/sessions`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
  const sessionIds = catalog.sessions.flatMap((session) =>
    session?.state !== 'stopped' && session?.labels?.suite === 'admin-recording-smoke'
      ? [session.id]
      : []);
  for (const staleSessionId of sessionIds) {
    await deleteSession(accessToken, options, staleSessionId);
  }
  if (sessionIds.length > 0) {
    log(`Removed ${sessionIds.length} stale recording smoke runtime(s).`);
  }
}

run().catch((error) => {
  console.error(`[admin-unified-recordings-smoke] ${error.stack || error.message}`);
  process.exitCode = 1;
});
