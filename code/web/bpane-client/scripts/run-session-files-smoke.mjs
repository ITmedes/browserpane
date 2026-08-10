import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';
import { spawnSync } from 'node:child_process';
import { chromium } from 'playwright-core';
import {
  cleanupWorkflowSmokeSessions,
  configurePage,
  createLogger,
  fetchBytes,
  fetchJson,
  getAccessToken,
  launchChrome,
  parseSmokeArgs,
  poll,
  ensureLoggedIn,
} from './workflow-smoke-lib.mjs';

function runBpaneCli(args, accessToken, options) {
  const result = spawnSync(
    process.execPath,
    [path.join(process.cwd(), 'scripts', 'bpane-cli.mjs'), ...args],
    {
      cwd: process.cwd(),
      env: {
        ...process.env,
        BPANE_ACCESS_TOKEN: accessToken,
        BPANE_BASE_URL: options.pageUrl,
      },
      encoding: 'utf8',
    },
  );
  if (result.status !== 0) {
    throw new Error(
      `bpane CLI failed with code ${result.status ?? 'unknown'}: ${result.stderr || result.stdout}`,
    );
  }
  return result.stdout.trim() ? JSON.parse(result.stdout) : null;
}

async function createWorkspaceBinding(accessToken, options, sessionId, text) {
  const workspace = await fetchJson(`${options.pageUrl}/api/v1/file-workspaces`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      name: `session-file-cli-smoke-${Date.now()}`,
      labels: { suite: 'session-files-smoke' },
    }),
  });
  const workspaceFile = await fetchJson(
    `${options.pageUrl}/api/v1/file-workspaces/${workspace.id}/files`,
    {
      method: 'POST',
      headers: {
        Authorization: `Bearer ${accessToken}`,
        'Content-Type': 'text/plain',
        'x-bpane-file-name': 'bound-evidence.txt',
      },
      body: text,
    },
  );
  const binding = await fetchJson(
    `${options.pageUrl}/api/v1/sessions/${sessionId}/file-bindings`,
    {
      method: 'POST',
      headers: {
        Authorization: `Bearer ${accessToken}`,
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        workspace_id: workspace.id,
        file_id: workspaceFile.id,
        mount_path: 'inputs/bound-evidence.txt',
        mode: 'read_only',
        labels: { suite: 'session-files-smoke' },
      }),
    },
  );
  return { workspace, workspaceFile, binding };
}

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
  const bindingText = `BrowserPane bound evidence ${Date.now()}\n`;
  await fs.writeFile(uploadPath, uploadText, 'utf8');
  let accessToken = '';
  let sessionId = '';
  let workspaceId = '';
  let workspaceFileId = '';
  let bindingId = '';

  try {
    log(`Opening ${options.pageUrl}`);
    await configurePage(page, options);
    await waitForEmbedControl(page, options);
    await ensureLoggedIn(page, options);
    accessToken = await getAccessToken(page);
    if (!accessToken) {
      throw new Error('Failed to acquire an access token from the test page.');
    }

    const controlState = await startSession(page, accessToken, options, log);
    sessionId = controlState.sessionId;
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

    const cliFileList = runBpaneCli(
      ['session', 'file', 'list', sessionId],
      accessToken,
      options,
    );
    if (!cliFileList.files?.some((entry) => entry.id === file.id)) {
      throw new Error(`CLI session file list did not include ${file.id}.`);
    }
    const cliFile = runBpaneCli(
      ['session', 'file', 'get', sessionId, file.id],
      accessToken,
      options,
    );
    if (cliFile.id !== file.id) {
      throw new Error(`CLI session file get returned ${cliFile.id ?? 'no id'}.`);
    }
    const cliFileOutput = path.join(tempDir, 'cli-session-upload.txt');
    const cliFileDownload = runBpaneCli(
      ['session', 'file', 'download', sessionId, file.id, '--output', cliFileOutput],
      accessToken,
      options,
    );
    if (
      cliFileDownload.byte_count !== Buffer.byteLength(uploadText)
      || await fs.readFile(cliFileOutput, 'utf8') !== uploadText
    ) {
      throw new Error('CLI session file download did not preserve exact content.');
    }

    const bindingResources = await createWorkspaceBinding(
      accessToken,
      options,
      sessionId,
      bindingText,
    );
    workspaceId = bindingResources.workspace.id;
    workspaceFileId = bindingResources.workspaceFile.id;
    bindingId = bindingResources.binding.id;
    const cliBindingList = runBpaneCli(
      ['session', 'file-binding', 'list', sessionId],
      accessToken,
      options,
    );
    if (!cliBindingList.bindings?.some((entry) => entry.id === bindingId)) {
      throw new Error(`CLI session file-binding list did not include ${bindingId}.`);
    }
    const cliBinding = runBpaneCli(
      ['session', 'file-binding', 'get', sessionId, bindingId],
      accessToken,
      options,
    );
    if (cliBinding.id !== bindingId || cliBinding.mount_path !== 'inputs/bound-evidence.txt') {
      throw new Error(`CLI session file-binding get returned unexpected data.`);
    }
    const cliBindingOutput = path.join(tempDir, 'cli-bound-evidence.txt');
    const cliBindingDownload = runBpaneCli(
      [
        'session',
        'file-binding',
        'download',
        sessionId,
        bindingId,
        '--output',
        cliBindingOutput,
      ],
      accessToken,
      options,
    );
    if (
      cliBindingDownload.byte_count !== Buffer.byteLength(bindingText)
      || await fs.readFile(cliBindingOutput, 'utf8') !== bindingText
    ) {
      throw new Error('CLI session file-binding download did not preserve exact content.');
    }

    const summary = {
      pageUrl: options.pageUrl,
      sessionId,
      file,
      downloadedBytes: downloaded.length,
      cli: {
        session_file: cliFileDownload,
        file_binding: cliBindingDownload,
      },
    };
    console.log(JSON.stringify(summary, null, 2));
    if (options.outputPath) {
      await fs.writeFile(options.outputPath, `${JSON.stringify(summary, null, 2)}\n`, 'utf8');
      log(`Wrote summary to ${options.outputPath}`);
    }
  } finally {
    if (accessToken && sessionId && bindingId) {
      await fetch(
        `${options.pageUrl}/api/v1/sessions/${sessionId}/file-bindings/${bindingId}`,
        {
          method: 'DELETE',
          headers: { Authorization: `Bearer ${accessToken}` },
        },
      ).catch(() => {});
    }
    if (accessToken && workspaceId && workspaceFileId) {
      await fetch(
        `${options.pageUrl}/api/v1/file-workspaces/${workspaceId}/files/${workspaceFileId}`,
        {
          method: 'DELETE',
          headers: { Authorization: `Bearer ${accessToken}` },
        },
      ).catch(() => {});
    }
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
