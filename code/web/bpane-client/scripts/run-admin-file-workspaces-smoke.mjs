import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';
import { chromium } from 'playwright-core';
import { ensureAdminLoggedIn, getAdminAccessToken } from './admin-smoke-lib.mjs';
import {
  createLogger,
  deleteSession,
  fetchJson,
  launchChrome,
  parseSmokeArgs,
  poll,
} from './workflow-smoke-lib.mjs';

async function run() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-admin-file-workspaces-smoke.mjs');
  const rootOptions = { ...options, pageUrl: new URL('/', options.pageUrl).origin };
  const adminOptions = {
    ...options,
    pageUrl: new URL('/admin/files/workspaces', rootOptions.pageUrl).toString(),
  };
  const log = createLogger('admin-file-workspaces-smoke');
  const browser = await launchChrome(chromium, options);
  const context = await browser.newContext({
    acceptDownloads: true,
    viewport: { width: 1440, height: 980 },
  });
  const page = await context.newPage();
  const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), 'bpane-admin-file-workspaces-'));
  const uploadPath = path.join(tempDir, 'customer-sample.csv');
  const deleteProbePath = path.join(tempDir, 'delete-probe.txt');
  const uploadText = `customer,total\nadmin-smoke-${Date.now()},42\n`;
  const deleteProbeText = `delete probe ${Date.now()}\n`;
  const workspaceName = `Admin file workspace smoke ${Date.now()}`;
  const mountPath = `uploads/admin-file-workspace-smoke-${Date.now()}.csv`;
  let accessToken = '';
  let sessionId = '';
  let workspaceId = '';
  let fileId = '';
  let bindingId = '';
  let verifiedBindingId = '';
  let boundWorkspaceFile = false;

  try {
    await fs.writeFile(uploadPath, uploadText, 'utf8');
    await fs.writeFile(deleteProbePath, deleteProbeText, 'utf8');
    log(`Opening ${adminOptions.pageUrl}`);
    await ensureAdminLoggedIn(page, adminOptions);
    accessToken = await getAdminAccessToken(page);
    if (!accessToken) {
      throw new Error('No admin access token available after login.');
    }

    await createWorkspaceFromUi(page, options, workspaceName);
    workspaceId = workspaceIdFromUrl(page.url());
    await uploadWorkspaceFileFromUi(page, options, uploadPath);
    fileId = await workspaceFileIdFromUi(page, options, 'customer-sample.csv');
    const downloadedWorkspaceText = await downloadWorkspaceFileFromUi(page, 'customer-sample.csv');
    if (downloadedWorkspaceText !== uploadText) {
      throw new Error('Workspace file download did not match uploaded payload.');
    }
    await uploadWorkspaceFileFromUi(page, options, deleteProbePath, 'delete-probe.txt');
    const deleteProbeFileId = await workspaceFileIdFromUi(page, options, 'delete-probe.txt');
    await deleteWorkspaceFileFromUi(page, options, deleteProbeFileId);

    sessionId = await createSession(accessToken, rootOptions);
    await openSessionDetail(page, rootOptions, sessionId, options);
    await createBindingFromUi(page, options, workspaceId, fileId, mountPath);
    bindingId = await bindingIdFromUi(page, options, mountPath);
    verifiedBindingId = bindingId;
    boundWorkspaceFile = true;

    await page.reload({ waitUntil: 'domcontentloaded' });
    await waitForBinding(page, options, mountPath);
    const downloadedBindingText = await downloadBindingFromUi(page, mountPath);
    if (downloadedBindingText !== uploadText) {
      throw new Error('Session binding download did not match uploaded workspace payload.');
    }

    await removeBindingFromUi(page, options, bindingId);
    bindingId = '';
    await deleteSession(accessToken, rootOptions, sessionId);
    sessionId = '';

    const summary = {
      pageUrl: adminOptions.pageUrl,
      workspaceId,
      fileId,
      bindingId: verifiedBindingId,
      mountPath,
      uploadedBytes: Buffer.byteLength(uploadText),
      bindingVerifiedAfterReload: true,
    };
    console.log(JSON.stringify(summary, null, 2));
    if (options.outputPath) {
      await fs.writeFile(options.outputPath, `${JSON.stringify(summary, null, 2)}\n`, 'utf8');
      log(`Wrote summary to ${options.outputPath}`);
    }
  } finally {
    if (accessToken && bindingId && sessionId) {
      await removeBinding(accessToken, rootOptions, sessionId, bindingId).catch(() => {});
    }
    if (accessToken && workspaceId && fileId && !boundWorkspaceFile) {
      await deleteWorkspaceFile(accessToken, rootOptions, workspaceId, fileId).catch(() => {});
    }
    if (accessToken && sessionId) {
      await deleteSession(accessToken, rootOptions, sessionId).catch(() => {});
    }
    await fs.rm(tempDir, { recursive: true, force: true }).catch(() => {});
    await context.close();
    await browser.close();
  }
}

async function createWorkspaceFromUi(page, options, workspaceName) {
  await page.getByTestId('file-workspace-create-name').fill(workspaceName);
  await page.getByTestId('file-workspace-create-description').fill('Admin smoke reusable inputs');
  await page.getByTestId('file-workspace-create-labels').fill('suite=smoke');
  await page.getByTestId('file-workspace-create-submit').click();
  await poll(
    'workspace detail navigation',
    async () => page.url(),
    (url) => url.includes('/admin/files/workspaces/') && !url.endsWith('/workspaces'),
    options.connectTimeoutMs,
  );
  await page.getByTestId('file-workspace-detail-title').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
}

async function uploadWorkspaceFileFromUi(page, options, uploadPath, fileName = 'customer-sample.csv') {
  await page.getByTestId('file-workspace-upload-input').setInputFiles(uploadPath);
  await page.getByTestId('file-workspace-upload-submit').click();
  await page.getByTestId('file-workspace-file-row').filter({ hasText: fileName }).waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
}

async function workspaceFileIdFromUi(page, options, fileName) {
  const row = page.getByTestId('file-workspace-file-row').filter({ hasText: fileName }).first();
  await row.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  const fileId = await row.getAttribute('data-file-id');
  if (!fileId) {
    throw new Error('Workspace file row did not expose a file id.');
  }
  return fileId;
}

async function downloadWorkspaceFileFromUi(page, fileName) {
  const row = page.getByTestId('file-workspace-file-row').filter({ hasText: fileName }).first();
  const downloadPromise = page.waitForEvent('download');
  await row.getByTestId('file-workspace-file-download').click();
  return await readDownloadText(await downloadPromise);
}

async function deleteWorkspaceFileFromUi(page, options, fileId) {
  const row = page.locator(`[data-testid="file-workspace-file-row"][data-file-id="${fileId}"]`);
  await row.getByTestId('file-workspace-file-delete').click();
  await row.waitFor({ state: 'detached', timeout: options.connectTimeoutMs });
}

async function createSession(accessToken, options) {
  const session = await fetchJson(`${options.pageUrl}/api/v1/sessions`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({ labels: { suite: 'admin-file-workspaces-smoke' } }),
  });
  return session.id;
}

async function openSessionDetail(page, rootOptions, sessionId, options) {
  await page.goto(new URL(`/admin/sessions/${sessionId}`, rootOptions.pageUrl).toString(), {
    waitUntil: 'domcontentloaded',
  });
  await page.getByTestId('session-file-bindings').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
}

async function createBindingFromUi(page, options, workspaceId, fileId, mountPath) {
  await page.getByTestId('session-file-binding-workspace').selectOption(workspaceId);
  await poll(
    'workspace file option',
    async () => await page.getByTestId('session-file-binding-file').locator(`option[value="${fileId}"]`).count(),
    (count) => count > 0,
    options.connectTimeoutMs,
  );
  await page.getByTestId('session-file-binding-file').selectOption(fileId);
  await page.getByTestId('session-file-binding-mount-path').fill(mountPath);
  await page.getByTestId('session-file-binding-create').click();
  await waitForBinding(page, options, mountPath);
}

async function waitForBinding(page, options, mountPath) {
  await page.getByTestId('session-file-binding-row').filter({ hasText: mountPath }).waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
}

async function bindingIdFromUi(page, options, mountPath) {
  const row = page.getByTestId('session-file-binding-row').filter({ hasText: mountPath }).first();
  await row.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  const bindingId = await row.getAttribute('data-binding-id');
  if (!bindingId) {
    throw new Error('Session file binding row did not expose a binding id.');
  }
  return bindingId;
}

async function downloadBindingFromUi(page, mountPath) {
  const row = page.getByTestId('session-file-binding-row').filter({ hasText: mountPath }).first();
  const downloadPromise = page.waitForEvent('download');
  await row.getByTestId('session-file-binding-download').click();
  return await readDownloadText(await downloadPromise);
}

async function removeBindingFromUi(page, options, bindingId) {
  const row = page.locator(`[data-testid="session-file-binding-row"][data-binding-id="${bindingId}"]`);
  await row.getByTestId('session-file-binding-remove').click();
  await row.waitFor({ state: 'detached', timeout: options.connectTimeoutMs });
}

async function readDownloadText(download) {
  const downloadPath = await download.path();
  if (!downloadPath) {
    throw new Error('Download did not produce a local file.');
  }
  return await fs.readFile(downloadPath, 'utf8');
}

async function removeBinding(accessToken, options, sessionId, bindingId) {
  const response = await fetch(`${options.pageUrl}/api/v1/sessions/${sessionId}/file-bindings/${bindingId}`, {
    method: 'DELETE',
    headers: { Authorization: `Bearer ${accessToken}` },
  });
  if (!response.ok && response.status !== 404) {
    const detail = await response.text().catch(() => '');
    throw new Error(`HTTP ${response.status}${detail ? ` ${detail}` : ''}`);
  }
}

async function deleteWorkspaceFile(accessToken, options, workspaceId, fileId) {
  const response = await fetch(`${options.pageUrl}/api/v1/file-workspaces/${workspaceId}/files/${fileId}`, {
    method: 'DELETE',
    headers: { Authorization: `Bearer ${accessToken}` },
  });
  if (!response.ok && response.status !== 404) {
    const detail = await response.text().catch(() => '');
    throw new Error(`HTTP ${response.status}${detail ? ` ${detail}` : ''}`);
  }
}

function workspaceIdFromUrl(rawUrl) {
  const url = new URL(rawUrl);
  const parts = url.pathname.split('/').filter(Boolean);
  const workspaceId = parts.at(-1);
  if (!workspaceId || workspaceId === 'workspaces') {
    throw new Error(`Could not parse workspace id from ${rawUrl}`);
  }
  return decodeURIComponent(workspaceId);
}

run().catch((error) => {
  console.error(`[admin-file-workspaces-smoke] ${error.stack || error.message}`);
  process.exitCode = 1;
});
