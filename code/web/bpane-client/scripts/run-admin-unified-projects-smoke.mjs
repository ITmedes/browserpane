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
  const options = parseSmokeArgs(process.argv.slice(2), 'run-admin-unified-projects-smoke.mjs');
  if (options.pageUrl === DEFAULTS.pageUrl) {
    options.pageUrl = `${DEFAULTS.pageUrl}/admin-new/projects`;
  }
  const log = createLogger('admin-unified-projects-smoke');
  const browser = await launchChrome(chromium, options);
  const context = await browser.newContext({ viewport: { width: 1440, height: 980 } });
  const page = await context.newPage();
  const runLabel = `admin-unified-projects-smoke-${Date.now()}`;
  let accessToken = '';
  let createdProject = null;
  let browserContext = null;

  try {
    log(`Opening ${options.pageUrl}`);
    await ensureAdminLoggedIn(page, options);
    accessToken = await getAdminAccessToken(page);
    const resources = await createPolicyResources(accessToken, options, runLabel);
    browserContext = resources.browserContext;

    await page.goto(adminRouteUrl(options, 'projects'), { waitUntil: 'domcontentloaded' });
    await page.getByTestId('projects-new-link').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });
    await assertNoHorizontalOverflow(page, 'projects-list', 'unified project catalog');
    await page.getByTestId('projects-new-link').click();
    await page.getByTestId('project-create-route').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });
    await assertNoHorizontalOverflow(page, 'project-create-route', 'unified project create route');

    await verifyCreateValidation(page);
    await configureProjectCreate(page, resources, runLabel);
    await page.getByTestId('project-edit-save').click();
    await page.waitForURL((url) => {
      const segments = url.pathname.split('/').filter(Boolean);
      return segments.at(-3) === 'admin-new'
        && segments.at(-2) === 'projects'
        && Boolean(segments.at(-1))
        && segments.at(-1) !== 'new';
    }, { timeout: options.connectTimeoutMs });

    const projectId = decodeURIComponent(new URL(page.url()).pathname.split('/').filter(Boolean).at(-1) ?? '');
    if (!projectId || projectId === 'new') {
      throw new Error(`Expected create flow to navigate to a project detail route, got ${page.url()}`);
    }
    createdProject = await fetchProject(accessToken, options, projectId);
    verifyCreatedProject(createdProject, resources, runLabel);

    await page.getByTestId('project-detail-route').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });
    await page.goto(adminRouteUrl(options, 'projects'), { waitUntil: 'domcontentloaded' });
    await page.getByTestId('projects-search').fill(runLabel);
    const row = page.locator('[data-testid="projects-list-row"]').filter({ hasText: runLabel });
    await row.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });

    console.log(JSON.stringify({
      projectId: createdProject.id,
      projectName: createdProject.name,
      templateId: resources.template.id,
      browserContextId: resources.browserContext.id,
      egressProfileId: resources.egressProfile.id,
      extensionId: resources.extension.id,
      fileWorkspaceId: resources.fileWorkspace.id,
    }, null, 2));
  } finally {
    if (accessToken && createdProject?.id) {
      await archiveProject(accessToken, options, createdProject, log);
    }
    if (accessToken && browserContext?.id) {
      await deleteBrowserContext(accessToken, options, browserContext.id, log);
    }
    await context.close();
    await browser.close();
  }
}

async function verifyCreateValidation(page) {
  if (!await page.getByTestId('project-edit-save').isDisabled()) {
    throw new Error('Expected new project save to be disabled before required fields are entered.');
  }
  await page.getByTestId('project-edit-name').fill('Invalid labels project');
  await page.getByTestId('project-edit-labels').fill('broken-label');
  const labelsError = await page.getByTestId('project-edit-labels-error').textContent();
  if (!labelsError?.includes('key=value')) {
    throw new Error(`Expected label validation error, got ${labelsError}`);
  }
  if (!await page.getByTestId('project-edit-save').isDisabled()) {
    throw new Error('Expected new project save to stay disabled while labels are invalid.');
  }
  await page.getByTestId('project-edit-name').fill('');
  await page.getByTestId('project-edit-labels').fill('');
}

async function configureProjectCreate(page, resources, runLabel) {
  await page.getByTestId('project-edit-name').fill(`Unified smoke project ${runLabel}`);
  await page.getByTestId('project-edit-description').fill('Created by the unified admin Projects smoke.');
  await page.getByTestId('project-edit-state').selectOption('archived');
  await page.getByTestId('project-edit-state').selectOption('active');
  await page.getByTestId('project-edit-labels').fill(`suite=admin-unified-projects-smoke\nrun=${runLabel}`);
  await page.getByTestId('project-policy-browser-uploads').setChecked(false);
  await page.getByTestId('project-policy-browser-downloads').setChecked(false);
  await page.getByTestId('project-policy-session-file-bindings').setChecked(false);
  await page.getByTestId('project-policy-manual-recordings').setChecked(false);
  await page.getByTestId('project-policy-budget-enforcement').selectOption('block_session_creation');

  await restrictAndSelect(page, 'session-templates', resources.template.id);
  await restrictAndSelect(page, 'browser-contexts', resources.browserContext.id);
  await restrictAndSelect(page, 'egress-profiles', resources.egressProfile.id);
  await restrictAndSelect(page, 'extensions', resources.extension.id);
  await restrictAndSelect(page, 'file-workspaces', resources.fileWorkspace.id);

  await enableAndFillQuota(page, 'max-active-sessions', '3');
  await enableAndFillQuota(page, 'max-active-workflow-runs', '4');
  await enableAndFillQuota(page, 'max-retained-storage-bytes', '1073741824');
  await enableAndFillQuota(page, 'max-session-creations', '100');
  await page.getByTestId('project-quota-session-creation-rate-enabled').setChecked(true);
  await page.getByTestId('project-quota-max-session-creations-per-window-value').fill('20');
  await page.getByTestId('project-quota-session-creation-window-sec-value').fill('3600');
  await enableAndFillQuota(page, 'max-runtime-usage-ms', '14400000');
  await enableAndFillQuota(page, 'max-egress-total-bytes', '2147483648');
}

async function restrictAndSelect(page, group, resourceId) {
  await page.getByTestId(`project-policy-${group}-restrict`).setChecked(true);
  const option = page.locator(`[data-testid="project-policy-${group}-option"][data-option-id="${resourceId}"]`);
  await option.waitFor({ state: 'visible' });
  await option.setChecked(true);
}

async function enableAndFillQuota(page, quotaTestId, value) {
  await page.getByTestId(`project-quota-${quotaTestId}-enabled`).setChecked(true);
  await page.getByTestId(`project-quota-${quotaTestId}-value`).fill(value);
}

async function createPolicyResources(accessToken, options, runLabel) {
  const template = await fetchJson(`${apiOrigin(options)}/api/v1/session-templates`, {
    method: 'POST',
    headers: authJsonHeaders(accessToken),
    body: JSON.stringify({
      name: `Unified smoke template ${runLabel}`,
      description: 'Selectable template for unified project smoke',
      labels: { suite: 'admin-unified-projects-smoke', run: runLabel },
      defaults: {
        labels: { suite: 'admin-unified-projects-smoke' },
      },
    }),
  });
  const browserContext = await fetchJson(`${apiOrigin(options)}/api/v1/browser-contexts`, {
    method: 'POST',
    headers: authJsonHeaders(accessToken),
    body: JSON.stringify({
      name: `Unified smoke context ${runLabel}`,
      description: 'Selectable browser context for unified project smoke',
      labels: { suite: 'admin-unified-projects-smoke', run: runLabel },
      retention_sec: 86400,
      max_profile_storage_bytes: 67108864,
    }),
  });
  const egressProfile = await fetchJson(`${apiOrigin(options)}/api/v1/egress-profiles`, {
    method: 'POST',
    headers: authJsonHeaders(accessToken),
    body: JSON.stringify({
      name: `Unified smoke egress ${runLabel}`,
      description: 'Selectable egress profile for unified project smoke',
      labels: { suite: 'admin-unified-projects-smoke', run: runLabel },
      proxy: { url: 'https://proxy.example:8443' },
    }),
  });
  const extension = await fetchJson(`${apiOrigin(options)}/api/v1/extensions`, {
    method: 'POST',
    headers: authJsonHeaders(accessToken),
    body: JSON.stringify({
      name: `Unified smoke extension ${runLabel}`,
      description: 'Selectable extension for unified project smoke',
      labels: { suite: 'admin-unified-projects-smoke', run: runLabel },
    }),
  });
  await fetchJson(`${apiOrigin(options)}/api/v1/extensions/${extension.id}/versions`, {
    method: 'POST',
    headers: authJsonHeaders(accessToken),
    body: JSON.stringify({
      version: '1.0.0',
      install_path: '/home/bpane/bpane-test-extension',
    }),
  });
  const fileWorkspace = await fetchJson(`${apiOrigin(options)}/api/v1/file-workspaces`, {
    method: 'POST',
    headers: authJsonHeaders(accessToken),
    body: JSON.stringify({
      name: `Unified smoke workspace ${runLabel}`,
      description: 'Selectable file workspace for unified project smoke',
      labels: { suite: 'admin-unified-projects-smoke', run: runLabel },
    }),
  });
  return { template, browserContext, egressProfile, extension, fileWorkspace };
}

function verifyCreatedProject(project, resources, runLabel) {
  assertEqual(project.name, `Unified smoke project ${runLabel}`, 'project name');
  assertEqual(project.description, 'Created by the unified admin Projects smoke.', 'project description');
  assertEqual(project.state, 'active', 'project state');
  assertEqual(project.labels?.suite, 'admin-unified-projects-smoke', 'project suite label');
  assertEqual(project.labels?.run, runLabel, 'project run label');
  assertEqual(project.policy?.allow_browser_uploads, false, 'browser uploads policy');
  assertEqual(project.policy?.allow_browser_downloads, false, 'browser downloads policy');
  assertEqual(project.policy?.allow_session_file_bindings, false, 'session file bindings policy');
  assertEqual(project.policy?.allow_manual_recordings, false, 'manual recordings policy');
  assertEqual(project.policy?.usage_budget_enforcement, 'block_session_creation', 'budget enforcement');
  assertSameIds(project.policy?.allowed_session_template_ids, [resources.template.id], 'session template allow-list');
  assertSameIds(project.policy?.allowed_browser_context_ids, [resources.browserContext.id], 'browser context allow-list');
  assertSameIds(project.policy?.allowed_egress_profile_ids, [resources.egressProfile.id], 'egress profile allow-list');
  assertSameIds(project.policy?.allowed_extension_ids, [resources.extension.id], 'extension allow-list');
  assertSameIds(project.policy?.allowed_file_workspace_ids, [resources.fileWorkspace.id], 'file workspace allow-list');
  assertEqual(project.quotas?.max_active_sessions, 3, 'max active sessions');
  assertEqual(project.quotas?.max_active_workflow_runs, 4, 'max active workflow runs');
  assertEqual(project.quotas?.max_retained_storage_bytes, 1073741824, 'max retained storage bytes');
  assertEqual(project.quotas?.max_session_creations, 100, 'max session creations');
  assertEqual(project.quotas?.max_session_creations_per_window, 20, 'max session creations per window');
  assertEqual(project.quotas?.session_creation_window_sec, 3600, 'session creation window seconds');
  assertEqual(project.quotas?.max_runtime_usage_ms, 14400000, 'max runtime usage ms');
  assertEqual(project.quotas?.max_egress_total_bytes, 2147483648, 'max egress total bytes');
}

async function fetchProject(accessToken, options, projectId) {
  return await fetchJson(`${apiOrigin(options)}/api/v1/projects/${projectId}`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function archiveProject(accessToken, options, project, log) {
  await fetchJson(`${apiOrigin(options)}/api/v1/projects/${project.id}`, {
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
  }).catch((error) => {
    log(`Project cleanup for ${project.id} failed: ${error.message}`);
  });
}

async function deleteBrowserContext(accessToken, options, contextId, log) {
  await fetchJson(`${apiOrigin(options)}/api/v1/browser-contexts/${contextId}`, {
    method: 'DELETE',
    headers: { Authorization: `Bearer ${accessToken}` },
  }).catch((error) => {
    log(`Browser context cleanup for ${contextId} failed: ${error.message}`);
  });
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

function adminRouteUrl(options, path) {
  return new URL(`/admin-new/${path.replace(/^\/+/, '')}`, apiOrigin(options)).toString();
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

function assertSameIds(actual, expected, label) {
  const actualIds = [...(actual ?? [])].sort();
  const expectedIds = [...expected].sort();
  if (JSON.stringify(actualIds) !== JSON.stringify(expectedIds)) {
    throw new Error(`Expected ${label} to be ${JSON.stringify(expectedIds)}, got ${JSON.stringify(actualIds)}`);
  }
}

run().catch((error) => {
  console.error(`[admin-unified-projects-smoke] ${error.stack || error.message}`);
  process.exit(1);
});
