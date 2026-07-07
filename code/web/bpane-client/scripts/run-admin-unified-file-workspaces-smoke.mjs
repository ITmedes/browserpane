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
  DEFAULTS,
  apiOrigin,
  createLogger,
  fetchJson,
  launchChrome,
  parseSmokeArgs,
} from './workflow-smoke-lib.mjs';

async function run() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-admin-unified-file-workspaces-smoke.mjs');
  if (options.pageUrl === DEFAULTS.pageUrl) {
    options.pageUrl = `${DEFAULTS.pageUrl}/admin-new/files/workspaces`;
  }
  const log = createLogger('admin-unified-file-workspaces-smoke');
  const browser = await launchChrome(chromium, options);
  const context = await browser.newContext({
    acceptDownloads: true,
    viewport: { width: 1440, height: 980 },
  });
  const page = await context.newPage();
  const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), 'bpane-admin-unified-file-workspaces-'));
  const uploadPath = path.join(tempDir, 'support-fixture.csv');
  const uploadText = `customer,total\nunified-file-workspace-${Date.now()},42\n`;
  const runLabel = `admin-unified-file-workspaces-smoke-${Date.now()}`;
  let accessToken = '';
  let project = null;
  let workspace = null;
  let fileId = '';

  try {
    await fs.writeFile(uploadPath, uploadText, 'utf8');
    log(`Opening ${options.pageUrl}`);
    await ensureAdminLoggedIn(page, options);
    accessToken = await getAdminAccessToken(page);
    if (!accessToken) {
      throw new Error('No admin access token available after login.');
    }

    project = await createProject(accessToken, options, runLabel);

    await page.goto(adminRouteUrl(options, 'files/workspaces'), { waitUntil: 'domcontentloaded' });
    await page.getByTestId('file-workspaces-new-link').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });
    await assertNoBodyHorizontalOverflow(page, 'unified file-workspace catalog');
    await assertNoHorizontalOverflow(page, 'file-workspaces-overview', 'unified file-workspace overview');

    workspace = await createWorkspaceThroughUi(page, accessToken, options, project, runLabel);
    verifyCreatedWorkspace(workspace, project, runLabel);

    fileId = await uploadFileThroughUi(page, accessToken, options, workspace.id, uploadPath, uploadText);
    await verifyCatalogSearch(page, options, workspace.name, workspace.id);
    await verifyResponsiveDetailLayout(page, options, workspace.id);
    await deleteFileThroughUi(page, accessToken, options, workspace.id, fileId);
    fileId = '';

    const summary = {
      workspaceId: workspace.id,
      workspaceName: workspace.name,
      projectId: project.id,
      uploadedBytes: Buffer.byteLength(uploadText),
    };
    console.log(JSON.stringify(summary, null, 2));
    if (options.outputPath) {
      await fs.writeFile(options.outputPath, `${JSON.stringify(summary, null, 2)}\n`, 'utf8');
      log(`Wrote summary to ${options.outputPath}`);
    }
  } finally {
    if (accessToken && workspace?.id && fileId) {
      await deleteWorkspaceFile(accessToken, options, workspace.id, fileId).catch((error) => {
        log(`Workspace file cleanup for ${fileId} failed: ${error.message}`);
      });
    }
    if (accessToken && project) {
      await archiveProject(accessToken, options, project).catch((error) => {
        log(`Project cleanup for ${project.id} failed: ${error.message}`);
      });
    }
    await fs.rm(tempDir, { recursive: true, force: true }).catch(() => {});
    await context.close();
    await browser.close();
  }
}

async function createWorkspaceThroughUi(page, accessToken, options, project, runLabel) {
  await page.goto(adminRouteUrl(options, 'files/workspaces/new'), { waitUntil: 'domcontentloaded' });
  await page.getByTestId('file-workspace-create-route').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await assertNoBodyHorizontalOverflow(page, 'unified file-workspace create route');
  await assertNoHorizontalOverflow(page, 'file-workspace-create-route', 'unified file-workspace create route');
  await verifyCreateValidation(page, options, project);

  await page.goto(adminRouteUrl(options, 'files/workspaces/new'), { waitUntil: 'domcontentloaded' });
  await page.getByTestId('file-workspace-create-route').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await page.getByTestId('file-workspace-edit-name').fill(`Unified file workspace ${runLabel}`);
  await page.getByTestId('file-workspace-edit-description').fill('Created by the unified admin file-workspace smoke.');
  await page.getByTestId('file-workspace-edit-labels').fill(`suite=admin-unified-file-workspaces-smoke\nrun=${runLabel}`);
  await page.getByTestId('file-workspace-edit-project-binding').selectOption('project');
  await page.locator(`[data-testid="file-workspace-edit-project-id"] option[value="${project.id}"]`).waitFor({
    state: 'attached',
    timeout: options.connectTimeoutMs,
  });
  await page.getByTestId('file-workspace-edit-project-id').selectOption(project.id);
  await assertNoBodyHorizontalOverflow(page, 'unified file-workspace populated create route');
  await assertNoHorizontalOverflow(page, 'file-workspace-edit-form', 'unified file-workspace edit form');

  if (!await page.getByTestId('file-workspace-edit-save').isEnabled()) {
    throw new Error('Expected file-workspace save to be enabled after all required fields were filled.');
  }
  await page.getByTestId('file-workspace-edit-save').click();

  const workspaceId = await waitForWorkspaceDetailNavigation(page, options);
  await page.getByTestId('file-workspace-detail-route').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  return await fetchWorkspace(accessToken, options, workspaceId);
}

async function verifyCreateValidation(page, options, project) {
  if (!await page.getByTestId('file-workspace-edit-save').isDisabled()) {
    throw new Error('Expected new file-workspace save to be disabled before required fields are entered.');
  }
  await page.getByTestId('file-workspace-edit-name').fill('Invalid workspace');
  await page.getByTestId('file-workspace-edit-labels').fill('broken-label');
  const labelsError = await page.getByTestId('file-workspace-edit-labels-error').textContent();
  if (!labelsError?.includes('key=value')) {
    throw new Error(`Expected label validation error, got ${labelsError}`);
  }
  if (!await page.getByTestId('file-workspace-edit-save').isDisabled()) {
    throw new Error('Expected save to stay disabled while labels are invalid.');
  }

  await page.getByTestId('file-workspace-edit-labels').fill('');
  await page.getByTestId('file-workspace-edit-project-binding').selectOption('project');
  const projectError = await page.getByTestId('file-workspace-edit-project-id-error').textContent();
  if (!projectError?.includes('Project-scoped workspaces need a project')) {
    throw new Error(`Expected project binding validation error, got ${projectError}`);
  }
  await page.locator(`[data-testid="file-workspace-edit-project-id"] option[value="${project.id}"]`).waitFor({
    state: 'attached',
    timeout: options.connectTimeoutMs,
  });
  await page.getByTestId('file-workspace-edit-project-id').selectOption(project.id);
  await page.getByTestId('file-workspace-edit-labels').fill('broken-label');
  if (!await page.getByTestId('file-workspace-edit-save').isDisabled()) {
    throw new Error('Expected save to stay disabled after reintroducing invalid labels.');
  }
  await assertNoBodyHorizontalOverflow(page, 'unified file-workspace validation messages');
  await assertNoHorizontalOverflow(page, 'file-workspace-edit-form', 'unified file-workspace validation form');
}

async function uploadFileThroughUi(page, accessToken, options, workspaceId, uploadPath, uploadText) {
  await page.getByTestId('file-workspace-upload-input').setInputFiles(uploadPath);
  await page.getByTestId('file-workspace-upload-submit').click();
  const row = page.getByTestId('file-workspace-file-row').filter({ hasText: 'support-fixture.csv' }).first();
  await row.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  const fileId = await row.getAttribute('data-file-id');
  if (!fileId) {
    throw new Error('Unified file-workspace row did not expose the uploaded file id.');
  }

  const files = await fetchWorkspaceFiles(accessToken, options, workspaceId);
  const uploaded = files.files.find((file) => file.id === fileId);
  if (!uploaded) {
    throw new Error(`Uploaded workspace file ${fileId} was not visible through the API.`);
  }
  assertEqual(uploaded.name, 'support-fixture.csv', 'uploaded file name');
  assertEqual(uploaded.byte_count, Buffer.byteLength(uploadText), 'uploaded byte count');

  const downloadPromise = page.waitForEvent('download');
  await row.getByTestId('file-workspace-file-download').click();
  const downloaded = await readDownloadText(await downloadPromise);
  assertEqual(downloaded, uploadText, 'downloaded file content');
  return fileId;
}

async function verifyCatalogSearch(page, options, workspaceName, workspaceId) {
  await page.goto(adminRouteUrl(options, 'files/workspaces'), { waitUntil: 'domcontentloaded' });
  await page.getByTestId('file-workspaces-new-link').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await page.getByTestId('file-workspaces-search').fill(workspaceName);
  const row = page.locator('[data-testid="file-workspaces-list-row"]').filter({ hasText: workspaceName });
  await row.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  const link = row.getByTestId('file-workspaces-detail-link');
  const href = await link.getAttribute('href');
  if (!href?.endsWith(`/admin-new/files/workspaces/${encodeURIComponent(workspaceId)}`)) {
    throw new Error(`Expected file-workspace catalog detail link for ${workspaceId}, got ${href}`);
  }
  await assertNoBodyHorizontalOverflow(page, 'unified file-workspace filtered catalog');
}

async function verifyResponsiveDetailLayout(page, options, workspaceId) {
  await page.setViewportSize({ width: 390, height: 900 });
  await page.goto(adminRouteUrl(options, `files/workspaces/${workspaceId}`), { waitUntil: 'domcontentloaded' });
  await page.getByTestId('file-workspace-detail-route').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await page.getByTestId('file-workspace-inspector').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await assertNoBodyHorizontalOverflow(page, 'mobile unified file-workspace detail route');
  await assertNoHorizontalOverflow(page, 'file-workspace-detail-route', 'mobile unified file-workspace detail route');
  await assertNoHorizontalOverflow(page, 'file-workspace-inspector', 'mobile unified file-workspace inspector');
  await page.setViewportSize({ width: 1440, height: 980 });
}

async function deleteFileThroughUi(page, accessToken, options, workspaceId, fileId) {
  await page.goto(adminRouteUrl(options, `files/workspaces/${workspaceId}`), { waitUntil: 'domcontentloaded' });
  await page.getByTestId('file-workspace-detail-route').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  const row = page.locator(`[data-testid="file-workspace-file-row"][data-file-id="${fileId}"]`);
  await row.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  await row.getByTestId('file-workspace-file-delete').click();
  await row.waitFor({ state: 'detached', timeout: options.connectTimeoutMs });
  const files = await fetchWorkspaceFiles(accessToken, options, workspaceId);
  if (files.files.some((file) => file.id === fileId)) {
    throw new Error(`Deleted workspace file ${fileId} was still visible through the API.`);
  }
}

async function waitForWorkspaceDetailNavigation(page, options) {
  await page.waitForURL((url) => {
    const expectedPrefix = new URL('/admin-new/files/workspaces/', apiOrigin(options)).toString();
    return url.toString().startsWith(expectedPrefix) && !url.pathname.endsWith('/new');
  }, { timeout: options.connectTimeoutMs });
  const workspaceId = decodeURIComponent(new URL(page.url()).pathname.split('/').filter(Boolean).at(-1) ?? '');
  if (!workspaceId || workspaceId === 'new') {
    throw new Error(`Expected create flow to navigate to file-workspace detail route, got ${page.url()}`);
  }
  return workspaceId;
}

async function readDownloadText(download) {
  const downloadPath = await download.path();
  if (!downloadPath) {
    throw new Error('Download did not produce a local file.');
  }
  return await fs.readFile(downloadPath, 'utf8');
}

async function createProject(accessToken, options, runLabel) {
  return await fetchJson(`${apiOrigin(options)}/api/v1/projects`, {
    method: 'POST',
    headers: authJsonHeaders(accessToken),
    body: JSON.stringify({
      name: `Unified file-workspace smoke project ${runLabel}`,
      description: 'Project binding target for the unified admin file-workspace smoke.',
      labels: { suite: 'admin-unified-file-workspaces-smoke', run: runLabel },
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

async function fetchWorkspace(accessToken, options, workspaceId) {
  return await fetchJson(`${apiOrigin(options)}/api/v1/file-workspaces/${workspaceId}`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function fetchWorkspaceFiles(accessToken, options, workspaceId) {
  return await fetchJson(`${apiOrigin(options)}/api/v1/file-workspaces/${workspaceId}/files`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function deleteWorkspaceFile(accessToken, options, workspaceId, fileId) {
  const response = await fetch(`${apiOrigin(options)}/api/v1/file-workspaces/${workspaceId}/files/${fileId}`, {
    method: 'DELETE',
    headers: { Authorization: `Bearer ${accessToken}` },
  });
  if (!response.ok && response.status !== 404) {
    const detail = await response.text().catch(() => '');
    throw new Error(`HTTP ${response.status}${detail ? ` ${detail}` : ''}`);
  }
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

function verifyCreatedWorkspace(workspace, project, runLabel) {
  assertEqual(workspace.name, `Unified file workspace ${runLabel}`, 'file workspace name');
  assertEqual(workspace.description, 'Created by the unified admin file-workspace smoke.', 'file workspace description');
  assertEqual(workspace.project_id, project.id, 'file workspace project id');
  assertEqual(workspace.labels?.suite, 'admin-unified-file-workspaces-smoke', 'file workspace suite label');
  assertEqual(workspace.labels?.run, runLabel, 'file workspace run label');
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

async function assertNoBodyHorizontalOverflow(page, label) {
  const size = await page.evaluate(() => ({
    clientWidth: document.documentElement.clientWidth,
    scrollWidth: document.documentElement.scrollWidth,
  }));
  if (size.scrollWidth > size.clientWidth + 1) {
    throw new Error(`${label} causes document horizontal overflow: ${JSON.stringify(size)}`);
  }
}

function adminRouteUrl(options, routePath) {
  return new URL(`/admin-new/${routePath.replace(/^\/+/, '')}`, apiOrigin(options)).toString();
}

function authJsonHeaders(accessToken) {
  return {
    Authorization: `Bearer ${accessToken}`,
    'content-type': 'application/json',
  };
}

function assertEqual(actual, expected, label) {
  if (actual !== expected) {
    throw new Error(`Expected ${label} to be ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`);
  }
}

run().catch((error) => {
  console.error(`[admin-unified-file-workspaces-smoke] ${error.stack || error.message}`);
  process.exit(1);
});
