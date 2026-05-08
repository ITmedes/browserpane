import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';
import { chromium } from 'playwright-core';
import {
  cleanupAdminBeforeRun,
  cleanupAdminSmoke,
  closeAdminOverlay,
  ensureAdminLoggedIn,
  ensureAdminOverlayOpen,
  getAdminAccessToken,
  openAdminTab,
  waitForBrowserConnected,
} from './admin-smoke-lib.mjs';
import { createRecordingSession, seedRetainedRecording } from './admin-recording-smoke-lib.mjs';
import { DEFAULTS, createLogger, launchChrome, parseSmokeArgs, poll, sleep } from './workflow-smoke-lib.mjs';

async function run() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-admin-recording-smoke.mjs');
  if (options.pageUrl === DEFAULTS.pageUrl) options.pageUrl = `${DEFAULTS.pageUrl}/admin/`;
  const rootUrl = new URL('/', options.pageUrl).origin;
  const log = createLogger('admin-recording-smoke');
  const browser = await launchChrome(chromium, options);
  const context = await browser.newContext({ acceptDownloads: true, viewport: { width: 1440, height: 980 } });
  const page = await context.newPage();

  try {
    log(`Opening ${options.pageUrl}`);
    await ensureAdminLoggedIn(page, options);
    await cleanupAdminBeforeRun(page, options, log);
    const accessToken = await getAdminAccessToken(page);
    const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), 'bpane-admin-recording-smoke-'));

    log('Creating a recording-enabled session.');
    const session = await createRecordingSession(accessToken, rootUrl);
    const sessionId = session.id;
    log(`Selecting session ${sessionId}.`);
    await selectSession(page, options, sessionId);
    log(`Connecting embedded browser for ${sessionId}.`);
    await connectBrowser(page, options);

    log('Capturing a local WebM through the admin recording controls.');
    const localRecording = await captureLocalRecording(page, options, tempDir, sessionId);
    await openAdminTab(page, 'recording');
    log(`Seeding retained recording metadata for ${sessionId}.`);
    const retained = await seedRetainedRecording(accessToken, rootUrl, sessionId, localRecording.path, localRecording.bytes);
    log('Verifying retained segment and playback export downloads from the admin library.');
    await verifyRecordingLibrary(page, options, tempDir, sessionId, retained, localRecording.bytes);
    await emitSummary(options, { sessionId, localRecording, retained, realtimeRecordingRefresh: true }, log);
  } finally {
    await cleanupAdminSmoke(page, options, log);
    await context.close();
    await browser.close();
  }
}

async function selectSession(page, options, sessionId) {
  await openAdminTab(page, 'sessions');
  await page.getByTestId('session-refresh').click();
  const row = page.locator(`[data-testid="session-row"][data-session-id="${sessionId}"]`);
  await row.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  await row.click();
}

async function connectBrowser(page, options) {
  await openAdminTab(page, 'sessions');
  await page.getByTestId('session-join').click();
  await waitForBrowserConnected(page, options);
  await page.locator('[data-testid="browser-viewport"] canvas').first().waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  await openAdminTab(page, 'recording');
}

async function captureLocalRecording(page, options, tempDir, sessionId) {
  await page.getByTestId('recording-auto-download').setChecked(false);
  await verifyRecordingStateSurvivesOverlayToggle(page, options);
  await waitForEnabled(page.getByTestId('recording-start'), options, 'recording start');
  await page.getByTestId('recording-start').click();
  await waitForRecordingStarted(page, options);
  await sleep(1800);
  await waitForEnabled(page.getByTestId('recording-stop'), options, 'recording stop');
  await page.getByTestId('recording-stop').click();
  await waitForEnabled(page.getByTestId('recording-download'), options, 'recording download');

  const targetPath = path.join(tempDir, `browserpane-${sessionId}-admin-local.webm`);
  const [download] = await Promise.all([page.waitForEvent('download'), page.getByTestId('recording-download').click()]);
  const saved = await saveDownload(download, targetPath);
  if (saved.bytes <= 1024) {
    throw new Error(`Admin local recording was unexpectedly small (${saved.bytes} bytes).`);
  }
  return saved;
}

async function verifyRecordingStateSurvivesOverlayToggle(page, options) {
  await waitForEnabled(page.getByTestId('recording-start'), options, 'recording start before overlay toggle');
  await page.getByTestId('recording-start').click();
  await waitForRecordingStarted(page, options);
  await closeAdminOverlay(page);
  await ensureAdminOverlayOpen(page);
  await page.getByTestId('workspace-panel-recording').waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  await waitForRecordingStarted(page, options);
  await waitForEnabled(page.getByTestId('recording-stop'), options, 'recording stop after overlay toggle');
  await page.getByTestId('recording-stop').click();
  await waitForEnabled(page.getByTestId('recording-download'), options, 'recording stopped after overlay toggle');
}

async function verifyRecordingLibrary(page, options, tempDir, sessionId, retained, expectedBytes) {
  await openAdminTab(page, 'recording');
  await page.getByTestId('recording-library-refresh').click();
  const row = page.locator(`[data-testid="recording-library-row"][data-recording-id="${retained.id}"]`);
  await row.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  await waitForEnabled(row.getByTestId('recording-segment-download'), options, 'segment download');
  const segment = await downloadFromButton(page, row.getByTestId('recording-segment-download'), path.join(tempDir, 'retained-segment.webm'));
  if (segment.bytes !== expectedBytes) {
    throw new Error(`Retained segment size mismatch: expected ${expectedBytes}, got ${segment.bytes}.`);
  }
  await waitForEnabled(page.getByTestId('recording-playback-download'), options, 'playback export');
  const exportBundle = await downloadFromButton(page, page.getByTestId('recording-playback-download'), path.join(tempDir, 'recording-playback.zip'));
  if (!exportBundle.suggestedFilename.endsWith('.zip') || exportBundle.bytes <= segment.bytes) {
    throw new Error('Admin playback export download did not produce a non-empty zip bundle.');
  }
}

async function downloadFromButton(page, button, targetPath) {
  const [download] = await Promise.all([page.waitForEvent('download'), button.click()]);
  return await saveDownload(download, targetPath);
}

async function saveDownload(download, targetPath) {
  await fs.mkdir(path.dirname(targetPath), { recursive: true });
  await download.saveAs(targetPath);
  const stats = await fs.stat(targetPath);
  return { path: targetPath, bytes: stats.size, suggestedFilename: download.suggestedFilename() };
}

async function waitForEnabled(locator, options, description) {
  await poll(description, async () => await locator.isEnabled(), Boolean, options.connectTimeoutMs);
}

async function waitForRecordingStarted(page, options) {
  const state = await poll('recording start result', async () => ({
    status: await page.getByTestId('recording-status').textContent().catch(() => ''),
    error: await page.getByTestId('recording-error').textContent().catch(() => ''),
  }), (value) => value.status === 'recording' || Boolean(value.error), options.connectTimeoutMs);
  if (state.error) throw new Error(`Admin local recording failed to start: ${state.error}`);
}

async function emitSummary(options, summary, log) {
  console.log(JSON.stringify(summary, null, 2));
  if (options.outputPath) {
    await fs.writeFile(options.outputPath, `${JSON.stringify(summary, null, 2)}\n`, 'utf8');
    log(`Wrote summary to ${options.outputPath}`);
  }
}

run().catch((error) => {
  console.error(`[admin-recording-smoke] ${error.stack || error.message}`);
  process.exitCode = 1;
});
