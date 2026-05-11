import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';
import { chromium } from 'playwright-core';
import {
  cleanupWorkflowSmokeSessions,
  configurePage,
  createLogger,
  fetchBytes,
  getAccessToken,
  launchChrome,
  parseSmokeArgs,
  poll,
  ensureLoggedIn,
} from './workflow-smoke-lib.mjs';

async function waitForEmbedControl(page, options) {
  await page.waitForFunction(
    () => Boolean(window.__bpaneControl && window.__bpaneSessionFiles),
    { timeout: options.connectTimeoutMs },
  );
}

async function startSession(page, accessToken, options, log) {
  await cleanupWorkflowSmokeSessions(accessToken, options, log);
  await page.evaluate(async () => {
    await window.__bpaneControl.refreshSessions({ preserveSelection: true, silent: true });
  });
  await page.click('#btn-new-session');
  return await poll(
    'connected session for session-file smoke',
    async () => await page.evaluate(() => window.__bpaneControl.getState()),
    (state) => state?.connected === true && Boolean(state?.sessionId),
    options.connectTimeoutMs,
  );
}

async function uploadThroughHarness(page, filePath) {
  const chooserPromise = page.waitForEvent('filechooser');
  await page.click('#btn-upload');
  const chooser = await chooserPromise;
  await chooser.setFiles(filePath);
}

async function run() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-session-files-smoke.mjs');
  const log = createLogger('session-files-smoke');
  const browser = await launchChrome(chromium, options);
  const context = await browser.newContext({
    viewport: { width: 1440, height: 980 },
  });
  const page = await context.newPage();
  const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), 'bpane-session-files-'));
  const uploadPath = path.join(tempDir, 'session-upload.txt');
  const uploadText = `BrowserPane session file smoke ${Date.now()}\n`;
  await fs.writeFile(uploadPath, uploadText, 'utf8');

  try {
    log(`Opening ${options.pageUrl}`);
    await configurePage(page, options);
    await waitForEmbedControl(page, options);
    await ensureLoggedIn(page, options);
    const accessToken = await getAccessToken(page);
    if (!accessToken) {
      throw new Error('Failed to acquire an access token from the test page.');
    }

    const controlState = await startSession(page, accessToken, options, log);
    await uploadThroughHarness(page, uploadPath);
    const filesState = await poll(
      'uploaded session file to appear in control plane',
      async () => await page.evaluate(async () => {
        await window.__bpaneSessionFiles.refresh({ force: true, silent: true });
        return window.__bpaneSessionFiles.getState();
      }),
      (state) => state?.files?.some((file) => (
        file.name === 'session-upload.txt'
        && file.source === 'browser_upload'
        && file.byte_count > 0
      )),
      options.connectTimeoutMs,
    );
    const file = filesState.files.find((entry) => entry.name === 'session-upload.txt');
    const downloaded = await fetchBytes(new URL(file.content_path, options.pageUrl), {
      headers: { Authorization: `Bearer ${accessToken}` },
    });
    if (downloaded.toString('utf8') !== uploadText) {
      throw new Error('Downloaded session file content did not match uploaded payload.');
    }

    const summary = {
      pageUrl: options.pageUrl,
      sessionId: controlState.sessionId,
      file,
      downloadedBytes: downloaded.length,
    };
    console.log(JSON.stringify(summary, null, 2));
    if (options.outputPath) {
      await fs.writeFile(options.outputPath, `${JSON.stringify(summary, null, 2)}\n`, 'utf8');
      log(`Wrote summary to ${options.outputPath}`);
    }
  } finally {
    try {
      await page.evaluate(async () => {
        const state = window.__bpaneControl?.getState?.();
        if (state?.connected) {
          await window.__bpaneControl.disconnect();
        }
      });
    } catch {
      // Ignore cleanup failures.
    }
    try {
      const accessToken = await getAccessToken(page);
      if (accessToken) {
        await cleanupWorkflowSmokeSessions(accessToken, options, log);
      }
    } catch {
      // Ignore cleanup failures.
    }
    await fs.rm(tempDir, { recursive: true, force: true });
    await context.close();
    await browser.close();
  }
}

run().catch((error) => {
  console.error(`[session-files-smoke] ${error.stack || error.message}`);
  process.exitCode = 1;
});
