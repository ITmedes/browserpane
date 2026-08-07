import process from 'node:process';
import { chromium } from 'playwright-core';
import {
  ensureAdminLoggedIn,
  getAdminAccessToken,
} from './admin-smoke-lib.mjs';
import {
  adminRouteUrl,
  assertEqual,
  assertNoHorizontalOverflow,
  authJsonHeaders,
} from './admin-unified-smoke-lib.mjs';
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
  let updatedProject = null;
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
    const initialCatalog = page.locator('[data-testid="projects-list"], [data-testid="projects-empty"]');
    await initialCatalog.waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });
    const initialCatalogTestId = await initialCatalog.getAttribute('data-testid');
    if (!initialCatalogTestId) {
      throw new Error('Expected the unified project catalog or its empty state to expose a test id.');
    }
    await assertNoHorizontalOverflow(page, initialCatalogTestId, 'unified project catalog');
    await page.getByTestId('projects-new-link').click();
    await page.getByTestId('project-create-route').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });
    await assertNoHorizontalOverflow(page, 'project-create-route', 'unified project create route');

    await verifyCreateValidation(page);
    await configureProjectCreate(page, resources, runLabel);
    await verifyConflictFeedback(page, options, runLabel);
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

    updatedProject = await createUpdateSeedProject(accessToken, options, runLabel);
    updatedProject = await verifyProjectUpdateFlow(page, accessToken, options, resources, updatedProject, runLabel);

    console.log(JSON.stringify({
      projectId: createdProject.id,
      projectName: createdProject.name,
      updatedProjectId: updatedProject.id,
      updatedProjectName: updatedProject.name,
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
    if (accessToken && updatedProject?.id) {
      await archiveProject(accessToken, options, updatedProject, log);
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

async function verifyConflictFeedback(page, options, runLabel) {
  let intercepted = false;
  const handler = async (route) => {
    if (route.request().method() !== 'POST') {
      await route.fallback();
      return;
    }
    intercepted = true;
    await route.fulfill({
      status: 409,
      contentType: 'application/json',
      body: JSON.stringify({
        error: 'Project policy changed while this draft was open.',
        code: 'project_revision_conflict',
        category: 'conflict',
        recovery_hint: 'Review the latest policy and retry.',
      }),
    });
  };
  await page.route('**/api/v1/projects', handler);
  try {
    await page.getByTestId('project-edit-save').click();
    await page.getByTestId('project-create-error').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });
    const feedback = page.getByTestId('project-create-error');
    const message = await feedback.textContent();
    if (!message?.includes('Project policy changed') || !message.includes('Review the latest policy')) {
      throw new Error(`Expected structured project conflict guidance, got ${message}`);
    }
    if (await feedback.getAttribute('role') !== 'alert') {
      throw new Error('Expected project conflict feedback to use an alert role.');
    }
    const retainedName = await page.getByTestId('project-edit-name').inputValue();
    if (retainedName !== `Unified smoke project ${runLabel}`) {
      throw new Error(`Project conflict did not retain the draft name: ${retainedName}`);
    }
    const [feedbackBox, formBox] = await Promise.all([
      feedback.boundingBox(),
      page.getByTestId('project-edit-form').boundingBox(),
    ]);
    if (!feedbackBox || !formBox || feedbackBox.y + feedbackBox.height > formBox.y + 1) {
      throw new Error(`Project conflict feedback overlaps the form: ${JSON.stringify({ feedbackBox, formBox })}`);
    }
    await assertNoHorizontalOverflow(page, 'project-create-route', 'project conflict feedback route');
  } finally {
    await page.unroute('**/api/v1/projects', handler);
  }
  if (!intercepted) {
    throw new Error('Project conflict response was not intercepted.');
  }
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

async function createUpdateSeedProject(accessToken, options, runLabel) {
  return await fetchJson(`${apiOrigin(options)}/api/v1/projects`, {
    method: 'POST',
    headers: authJsonHeaders(accessToken),
    body: JSON.stringify({
      name: `Unified update seed ${runLabel}`,
      description: 'Initial project created by the unified admin Projects smoke.',
      labels: {
        suite: 'admin-unified-projects-smoke',
        run: runLabel,
        phase: 'seed',
      },
      state: 'active',
      policy: {
        allowed_session_template_ids: [],
        allowed_egress_profile_ids: [],
        allowed_extension_ids: [],
        allowed_browser_context_ids: [],
        allowed_file_workspace_ids: [],
        allow_browser_uploads: false,
        allow_browser_downloads: false,
        allow_session_file_bindings: false,
        allow_manual_recordings: false,
        usage_budget_enforcement: 'warning_only',
      },
      quotas: {},
    }),
  });
}

async function verifyProjectUpdateFlow(page, accessToken, options, resources, project, runLabel) {
  await page.goto(adminRouteUrl(options, `projects/${project.id}`), { waitUntil: 'domcontentloaded' });
  await page.getByTestId('project-detail-route').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await page.getByTestId('project-edit-form').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await assertNoHorizontalOverflow(page, 'project-detail-route', 'unified project detail route');

  await configureProjectUpdate(page, resources, runLabel);
  if (!await page.getByTestId('project-edit-save').isEnabled()) {
    throw new Error('Expected project update save to be enabled after editing every field.');
  }
  await page.getByTestId('project-edit-save').click();
  await page.getByTestId('project-action-success').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  const successText = await page.getByTestId('project-action-success').textContent();
  if (!successText?.includes('Project saved')) {
    throw new Error(`Expected project update success message, got ${successText}`);
  }

  const refreshed = await fetchProject(accessToken, options, project.id);
  verifyUpdatedProject(refreshed, resources, runLabel);
  await verifyUpdatedProjectForm(page, options, resources, refreshed.id, runLabel);
  return refreshed;
}

async function configureProjectUpdate(page, resources, runLabel) {
  await page.getByTestId('project-edit-name').fill(`Unified updated project ${runLabel}`);
  await page.getByTestId('project-edit-description').fill('Updated through the unified admin Projects detail smoke.');
  await page.getByTestId('project-edit-state').selectOption('archived');
  await page.getByTestId('project-edit-labels').fill(`suite=admin-unified-projects-smoke\nrun=${runLabel}\nphase=updated`);
  await page.getByTestId('project-policy-browser-uploads').setChecked(true);
  await page.getByTestId('project-policy-browser-downloads').setChecked(true);
  await page.getByTestId('project-policy-session-file-bindings').setChecked(true);
  await page.getByTestId('project-policy-manual-recordings').setChecked(true);
  await page.getByTestId('project-policy-budget-enforcement').selectOption('block_session_creation');

  await restrictAndSelect(page, 'session-templates', resources.template.id);
  await restrictAndSelect(page, 'browser-contexts', resources.browserContext.id);
  await restrictAndSelect(page, 'egress-profiles', resources.egressProfile.id);
  await restrictAndSelect(page, 'extensions', resources.extension.id);
  await restrictAndSelect(page, 'file-workspaces', resources.fileWorkspace.id);

  await enableAndFillQuota(page, 'max-active-sessions', '5');
  await enableAndFillQuota(page, 'max-active-workflow-runs', '6');
  await enableAndFillQuota(page, 'max-retained-storage-bytes', '2147483648');
  await enableAndFillQuota(page, 'max-session-creations', '200');
  await page.getByTestId('project-quota-session-creation-rate-enabled').setChecked(true);
  await page.getByTestId('project-quota-max-session-creations-per-window-value').fill('40');
  await page.getByTestId('project-quota-session-creation-window-sec-value').fill('86400');
  await enableAndFillQuota(page, 'max-runtime-usage-ms', '28800000');
  await enableAndFillQuota(page, 'max-egress-total-bytes', '3221225472');
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

function verifyUpdatedProject(project, resources, runLabel) {
  assertEqual(project.name, `Unified updated project ${runLabel}`, 'updated project name');
  assertEqual(project.description, 'Updated through the unified admin Projects detail smoke.', 'updated project description');
  assertEqual(project.state, 'archived', 'updated project state');
  assertEqual(project.labels?.suite, 'admin-unified-projects-smoke', 'updated project suite label');
  assertEqual(project.labels?.run, runLabel, 'updated project run label');
  assertEqual(project.labels?.phase, 'updated', 'updated project phase label');
  assertEqual(project.policy?.allow_browser_uploads, true, 'updated browser uploads policy');
  assertEqual(project.policy?.allow_browser_downloads, true, 'updated browser downloads policy');
  assertEqual(project.policy?.allow_session_file_bindings, true, 'updated session file bindings policy');
  assertEqual(project.policy?.allow_manual_recordings, true, 'updated manual recordings policy');
  assertEqual(project.policy?.usage_budget_enforcement, 'block_session_creation', 'updated budget enforcement');
  assertSameIds(project.policy?.allowed_session_template_ids, [resources.template.id], 'updated session template allow-list');
  assertSameIds(project.policy?.allowed_browser_context_ids, [resources.browserContext.id], 'updated browser context allow-list');
  assertSameIds(project.policy?.allowed_egress_profile_ids, [resources.egressProfile.id], 'updated egress profile allow-list');
  assertSameIds(project.policy?.allowed_extension_ids, [resources.extension.id], 'updated extension allow-list');
  assertSameIds(project.policy?.allowed_file_workspace_ids, [resources.fileWorkspace.id], 'updated file workspace allow-list');
  assertEqual(project.quotas?.max_active_sessions, 5, 'updated max active sessions');
  assertEqual(project.quotas?.max_active_workflow_runs, 6, 'updated max active workflow runs');
  assertEqual(project.quotas?.max_retained_storage_bytes, 2147483648, 'updated max retained storage bytes');
  assertEqual(project.quotas?.max_session_creations, 200, 'updated max session creations');
  assertEqual(project.quotas?.max_session_creations_per_window, 40, 'updated max session creations per window');
  assertEqual(project.quotas?.session_creation_window_sec, 86400, 'updated session creation window seconds');
  assertEqual(project.quotas?.max_runtime_usage_ms, 28800000, 'updated max runtime usage ms');
  assertEqual(project.quotas?.max_egress_total_bytes, 3221225472, 'updated max egress total bytes');
}

async function verifyUpdatedProjectForm(page, options, resources, projectId, runLabel) {
  await page.goto(adminRouteUrl(options, `projects/${projectId}`), { waitUntil: 'domcontentloaded' });
  await page.getByTestId('project-edit-form').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await assertInputValue(page, 'project-edit-name', `Unified updated project ${runLabel}`);
  await assertInputValue(page, 'project-edit-description', 'Updated through the unified admin Projects detail smoke.');
  await assertInputValue(page, 'project-edit-state', 'archived');
  await assertInputValue(page, 'project-edit-labels', `phase=updated\nrun=${runLabel}\nsuite=admin-unified-projects-smoke`);
  await assertCheckbox(page, 'project-policy-browser-uploads', true);
  await assertCheckbox(page, 'project-policy-browser-downloads', true);
  await assertCheckbox(page, 'project-policy-session-file-bindings', true);
  await assertCheckbox(page, 'project-policy-manual-recordings', true);
  await assertInputValue(page, 'project-policy-budget-enforcement', 'block_session_creation');
  await assertCheckbox(page, 'project-policy-session-templates-restrict', true);
  await assertCheckbox(page, 'project-policy-browser-contexts-restrict', true);
  await assertCheckbox(page, 'project-policy-egress-profiles-restrict', true);
  await assertCheckbox(page, 'project-policy-extensions-restrict', true);
  await assertCheckbox(page, 'project-policy-file-workspaces-restrict', true);
  await assertPolicyOption(page, 'session-templates', resources.template.id);
  await assertPolicyOption(page, 'browser-contexts', resources.browserContext.id);
  await assertPolicyOption(page, 'egress-profiles', resources.egressProfile.id);
  await assertPolicyOption(page, 'extensions', resources.extension.id);
  await assertPolicyOption(page, 'file-workspaces', resources.fileWorkspace.id);
  await assertInputValue(page, 'project-quota-max-active-sessions-value', '5');
  await assertInputValue(page, 'project-quota-max-active-workflow-runs-value', '6');
  await assertInputValue(page, 'project-quota-max-retained-storage-bytes-value', '2147483648');
  await assertInputValue(page, 'project-quota-max-session-creations-value', '200');
  await assertInputValue(page, 'project-quota-max-session-creations-per-window-value', '40');
  await assertInputValue(page, 'project-quota-session-creation-window-sec-value', '86400');
  await assertInputValue(page, 'project-quota-max-runtime-usage-ms-value', '28800000');
  await assertInputValue(page, 'project-quota-max-egress-total-bytes-value', '3221225472');
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

async function assertInputValue(page, testId, expected) {
  const actual = await page.getByTestId(testId).inputValue();
  assertEqual(actual, expected, `${testId} value`);
}

async function assertCheckbox(page, testId, expected) {
  const actual = await page.getByTestId(testId).isChecked();
  assertEqual(actual, expected, `${testId} checked state`);
}

async function assertPolicyOption(page, group, resourceId) {
  const option = page.locator(`[data-testid="project-policy-${group}-option"][data-option-id="${resourceId}"]`);
  await option.waitFor({ state: 'visible' });
  const checked = await option.isChecked();
  assertEqual(checked, true, `${group} policy option ${resourceId} checked state`);
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
