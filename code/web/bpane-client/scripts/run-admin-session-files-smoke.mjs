import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';
import { chromium } from 'playwright-core';
import { ensureAdminLoggedIn } from './admin-smoke-lib.mjs';
import {
  cleanupWorkflowSmokeSessions,
  configurePage,
  createLogger,
  DEFAULTS,
  ensureLoggedIn,
  getAccessToken,
  launchChrome,
  parseSmokeArgs,
  poll,
} from './workflow-smoke-lib.mjs';

async function run() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-admin-session-files-smoke.mjs');
  const rootOptions = { ...options, pageUrl: new URL('/', options.pageUrl).origin };
  const adminOptions = {
    ...options,
    pageUrl: options.pageUrl === DEFAULTS.pageUrl ? `${DEFAULTS.pageUrl}/admin/` : options.pageUrl,
  };
  const log = createLogger('admin-session-files-smoke');
  const browser = await launchChrome(chromium, options);
  const context = await browser.newContext({
    acceptDownloads: true,
    viewport: { width: 1440, height: 980 },
  });
  const harnessPage = await context.newPage();
  const adminPage = await context.newPage();
  const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), 'bpane-admin-session-files-'));
  const uploadPath = path.join(tempDir, 'session-upload.txt');
  const uploadText = `BrowserPane admin session file smoke ${Date.now()}\n`;
  let accessToken = '';

  try {
    await fs.writeFile(uploadPath, uploadText, 'utf8');
    const controlState = await createFileArtifact(harnessPage, rootOptions, uploadPath, log);
    accessToken = await getAccessToken(harnessPage);

    log(`Opening ${adminOptions.pageUrl}`);
    await ensureAdminLoggedIn(adminPage, adminOptions);
    await selectSession(adminPage, controlState.sessionId, adminOptions);
    const fileRow = await waitForFileRow(adminPage, adminOptions);
    const downloadedText = await downloadFileRow(adminPage, fileRow);
    if (downloadedText !== uploadText) {
      throw new Error('Admin session file download did not match uploaded payload.');
    }

    const summary = {
      pageUrl: adminOptions.pageUrl,
      sessionId: controlState.sessionId,
      fileName: 'session-upload.txt',
      downloadedBytes: Buffer.byteLength(downloadedText),
    };
    console.log(JSON.stringify(summary, null, 2));
    if (options.outputPath) {
      await fs.writeFile(options.outputPath, `${JSON.stringify(summary, null, 2)}\n`, 'utf8');
      log(`Wrote summary to ${options.outputPath}`);
    }
  } finally {
    await disconnectHarness(harnessPage).catch(() => {});
    if (accessToken) {
      await cleanupWorkflowSmokeSessions(accessToken, rootOptions, log).catch(() => {});
    }
    await fs.rm(tempDir, { recursive: true, force: true }).catch(() => {});
    await context.close();
    await browser.close();
  }
}

async function createFileArtifact(page, options, uploadPath, log) {
  log(`Opening ${options.pageUrl}`);
  await configurePage(page, options);
  await page.waitForFunction(() => Boolean(window.__bpaneControl && window.__bpaneSessionFiles), {
    timeout: options.connectTimeoutMs,
  });
  await ensureLoggedIn(page, options);
  const accessToken = await getAccessToken(page);
  if (!accessToken) {
    throw new Error('Failed to acquire an access token from the test page.');
  }

  await cleanupWorkflowSmokeSessions(accessToken, options, log);
  await page.evaluate(async () => {
    await window.__bpaneControl.refreshSessions({ preserveSelection: true, silent: true });
  });
  await page.click('#btn-new-session');
  const controlState = await poll(
    'connected session for admin file smoke',
    async () => await page.evaluate(() => window.__bpaneControl.getState()),
    (state) => state?.connected === true && Boolean(state?.sessionId),
    options.connectTimeoutMs,
  );

  const chooserPromise = page.waitForEvent('filechooser');
  await page.click('#btn-upload');
  const chooser = await chooserPromise;
  await chooser.setFiles(uploadPath);
  await poll(
    'uploaded file to appear in admin source session',
    async () => await page.evaluate(async () => {
      await window.__bpaneSessionFiles.refresh({ force: true, silent: true });
      return window.__bpaneSessionFiles.getState();
    }),
    (state) => state?.files?.some((file) => file.name === 'session-upload.txt'),
    options.connectTimeoutMs,
  );
  return controlState;
}

async function selectSession(page, sessionId, options) {
  const row = page.locator(`[data-testid="session-row"][data-session-id="${sessionId}"]`);
  await row.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  await row.click();
}

async function waitForFileRow(page, options) {
  const refresh = page.getByTestId('session-files-refresh');
  await refresh.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  if (await refresh.isEnabled()) {
    await refresh.click();
  }
  const row = page.getByTestId('session-files-row').filter({ hasText: 'session-upload.txt' }).first();
  await row.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  return row;
}

async function downloadFileRow(page, row) {
  const downloadPromise = page.waitForEvent('download');
  await row.getByTestId('session-file-download').click();
  const download = await downloadPromise;
  const downloadPath = await download.path();
  if (!downloadPath) {
    throw new Error('Admin session file download did not produce a local file.');
  }
  return await fs.readFile(downloadPath, 'utf8');
}

async function disconnectHarness(page) {
  await page.evaluate(async () => {
    const state = window.__bpaneControl?.getState?.();
    if (state?.connected) {
      await window.__bpaneControl.disconnect();
    }
  });
}

run().catch((error) => {
  console.error(`[admin-session-files-smoke] ${error.stack || error.message}`);
  process.exitCode = 1;
});
