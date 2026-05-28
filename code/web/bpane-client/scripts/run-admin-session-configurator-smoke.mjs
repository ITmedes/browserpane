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
  poll,
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
  let project = null;
  let template = null;
  let browserContext = null;
  let egressProfile = null;

  try {
    log(`Opening ${options.pageUrl}`);
    await ensureAdminLoggedIn(page, options);
    await cleanupAdminBeforeRun(page, options, log);
    project = await createProject(page, options);
    egressProfile = await createEgressProfile(page, options);
    template = await createSessionTemplate(page, options, egressProfile);

    browserContext = await verifyCompactPayloadToggle(page, options, template, project);

    await page.goto(adminRouteUrl(options, 'sessions'), { waitUntil: 'domcontentloaded' });
    await page.getByTestId('session-create-configurator').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });

    await verifyClientValidation(page);
    await configureTemplatedSession(page, options, template, project, browserContext, egressProfile);
    await verifyPayloadPreview(page, template, project, browserContext, egressProfile);
    await page.getByTestId('session-inspector-new').click();
    sessionId = await waitForSessionDetailUrl(page, options);

    const session = await fetchSession(page, options, sessionId);
    verifyCreatedSession(session, sessionId, template, project, browserContext, egressProfile);
    await verifyDetailUi(page, options, sessionId, template, project, browserContext, egressProfile);
    await verifyInspectorTemplateFilter(page, options, sessionId, template, project, browserContext, egressProfile);
    await emitSummary(page, options, session, template, project, log);
  } finally {
    await cleanupCreatedSession(page, options, sessionId, log);
    await cleanupCreatedBrowserContext(page, options, browserContext?.id ?? '', log);
    await cleanupCreatedProject(page, options, project, log);
    await context.close();
    await browser.close();
  }
}

async function verifyCompactPayloadToggle(page, options, template, project) {
  await page.goto(options.pageUrl, { waitUntil: 'domcontentloaded' });
  await openAdminTab(page, 'sessions');
  const browserContext = await createBrowserContextThroughUi(page, options);
  await selectProject(page, project, options);
  await selectSessionTemplate(page, template, options);
  await selectBrowserContext(page, browserContext, options);
  await assertNoHorizontalOverflow(page, 'session-create-configurator', 'live session create configurator');
  const projectSummary = await page.getByTestId('session-create-project-summary').textContent();
  if (!projectSummary?.includes(project.name) || !projectSummary.includes('sessions=0/1')) {
    throw new Error(`Expected live configurator project summary for ${project.name}, got ${projectSummary}`);
  }
  const templateSummary = await page.getByTestId('session-create-template-summary').textContent();
  if (!templateSummary?.includes(template.name) || !templateSummary.includes('team=support') || !templateSummary.includes('locale=de-DE')) {
    throw new Error(`Expected live configurator template summary for ${template.name}, got ${templateSummary}`);
  }
  const toggle = page.getByTestId('session-create-preview-toggle');
  await toggle.click();
  await page.getByTestId('session-create-preview').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  const previewText = await page.getByTestId('session-create-preview').textContent();
  const preview = JSON.parse(previewText ?? '{}');
  if (preview.project_id !== project.id || preview.template_id !== template.id) {
    throw new Error(`Expected live configurator preview to include project/template ids ${project.id}/${template.id}, got ${previewText}`);
  }
  if (preview.browser_context?.mode !== 'reusable' || preview.browser_context?.context_id !== browserContext.id) {
    throw new Error(`Expected live configurator preview to include reusable browser context ${browserContext.id}, got ${previewText}`);
  }
  if (Object.hasOwn(preview, 'owner_mode')) {
    throw new Error(`Expected template default owner mode to remain unoverridden, got ${previewText}`);
  }
  await page.waitForTimeout(1500);
  const stillVisible = await page.getByTestId('session-create-preview').isVisible().catch(() => false);
  if (!stillVisible) {
    throw new Error('Expected compact API payload preview to stay expanded after opening.');
  }
  await toggle.click();
  return browserContext;
}

async function verifyClientValidation(page, options) {
  await page.getByTestId('session-create-browser-context-mode').selectOption('reusable');
  const missingContextDisabled = await page.getByTestId('session-inspector-new').isDisabled();
  if (!missingContextDisabled) {
    throw new Error('Expected configured session create to be disabled when reusable context mode has no selected context.');
  }
  const missingContextError = await page.getByTestId('session-create-error').textContent();
  if (!missingContextError?.includes('Reusable browser context')) {
    throw new Error(`Expected reusable context validation error, got ${missingContextError}`);
  }
  await page.getByTestId('session-create-browser-context-mode').selectOption('fresh');

  await page.getByTestId('session-create-context-name').fill('Invalid context');
  await page.getByTestId('session-create-context-labels').fill('malformed-label');
  await page.getByTestId('session-create-context-max-profile-mb').fill('0');
  const contextCreateDisabled = await page.getByTestId('session-create-context-create').isDisabled();
  if (!contextCreateDisabled) {
    throw new Error('Expected browser context quick-create to be disabled for malformed labels.');
  }
  const contextCreateError = await page.getByTestId('session-create-context-create-error').textContent();
  if (!contextCreateError?.includes('key=value') || !contextCreateError.includes('Max profile storage')) {
    throw new Error(`Expected browser context quick-create validation error, got ${contextCreateError}`);
  }
  await page.getByTestId('session-create-context-name').fill('');
  await page.getByTestId('session-create-context-labels').fill('');
  await page.getByTestId('session-create-context-max-profile-mb').fill('');

  await page.getByTestId('session-create-idle-timeout').fill('0');
  await page.getByTestId('session-create-labels').fill('case=1234\ncase=5678');
  await page.getByTestId('session-create-geolocation-latitude').fill('91');
  await page.getByTestId('session-create-geolocation-longitude').fill('13.405');
  const disabled = await page.getByTestId('session-inspector-new').isDisabled();
  if (!disabled) {
    throw new Error('Expected configured session create to be disabled for invalid idle timeout and duplicate labels.');
  }
  const errorText = await page.getByTestId('session-create-error').textContent();
  if (!errorText?.includes('Idle timeout') || !errorText.includes('duplicated') || !errorText.includes('Latitude')) {
    throw new Error(`Expected validation errors for idle timeout and duplicate labels, got ${errorText}`);
  }
  await page.getByTestId('session-create-idle-timeout').fill('');
  await page.getByTestId('session-create-labels').fill('');
  await page.getByTestId('session-create-geolocation-latitude').fill('');
  await page.getByTestId('session-create-geolocation-longitude').fill('');
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

async function configureTemplatedSession(page, options, template, project, browserContext, egressProfile) {
  await selectProject(page, project, options);
  await selectSessionTemplate(page, template, options);
  await selectBrowserContext(page, browserContext, options);
  await page.getByTestId('session-create-locale').fill('de-DE');
  await page.getByTestId('session-create-languages').fill('de-DE, en-US');
  await page.getByTestId('session-create-timezone').fill('Europe/Berlin');
  await page.getByTestId('session-create-geolocation-latitude').fill('52.52');
  await page.getByTestId('session-create-geolocation-longitude').fill('13.405');
  await page.getByTestId('session-create-geolocation-accuracy').fill('100');
  await page.getByTestId('session-create-browser-identity').fill('desktop-chromium-stable');
  await selectOptionWhenAvailable(page, page.getByTestId('session-create-egress-profile'), egressProfile.id, options);
  await page.getByTestId('session-create-owner-mode').selectOption('collaborative');
  await page.getByTestId('session-create-owner-mode').selectOption('');
  await page.getByTestId('session-create-idle-timeout').fill('');
  await page.getByTestId('session-create-labels').fill('case=1234\npurpose=import-repro');
}

async function verifyPayloadPreview(page, template, project, browserContext, egressProfile) {
  const previewText = await page.getByTestId('session-create-preview').textContent();
  const preview = JSON.parse(previewText ?? '{}');
  const expectedLabels = { case: '1234', purpose: 'import-repro' };
  if (
    preview.project_id !== project.id
    || preview.template_id !== template.id
    || Object.hasOwn(preview, 'owner_mode')
    || Object.hasOwn(preview, 'idle_timeout_sec')
    || preview.browser_context?.mode !== 'reusable'
    || preview.browser_context?.context_id !== browserContext.id
    || preview.network_identity?.locale !== 'de-DE'
    || preview.network_identity?.timezone !== 'Europe/Berlin'
    || preview.network_identity?.geolocation?.latitude !== 52.52
    || preview.network_identity?.egress_profile_id !== egressProfile.id
    || JSON.stringify(preview.labels) !== JSON.stringify(expectedLabels)
  ) {
    throw new Error(`Unexpected session create payload preview: ${previewText}`);
  }
}

async function verifyDetailUi(page, options, sessionId, template, project, browserContext, egressProfile) {
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
  const detailProject = await poll(
    'session detail project name',
    async () => await page.getByTestId('session-project').textContent(),
    (value) => value?.includes(project.name) === true,
    options.connectTimeoutMs,
    100,
  );
  const detailAdmission = await page.getByTestId('session-admission').textContent();
  const detailTemplate = await poll(
    'session detail template name',
    async () => await page.getByTestId('session-template').textContent(),
    (value) => value?.includes(template.name) === true,
    options.connectTimeoutMs,
    100,
  );
  const detailBrowserContext = await poll(
    'session detail browser context name',
    async () => await page.getByTestId('session-browser-context').textContent(),
    (value) => value?.includes(browserContext.name) === true,
    options.connectTimeoutMs,
    100,
  );
  const labels = await page.getByTestId('session-labels').textContent();
  const integration = await page.getByTestId('session-integration-context').textContent();
  const network = await page.getByTestId('session-network-identity').textContent();
  const egress = await page.getByTestId('session-effective-egress').textContent();
  if (
    ownerMode !== 'collaborative'
    || idleTimeout !== '1200'
    || !detailProject?.includes(project.name)
    || !detailAdmission?.includes('project_quota_available')
    || !detailTemplate?.includes(template.name)
    || !detailBrowserContext?.includes(browserContext.name)
    || !network?.includes('de-DE')
    || !network.includes('Europe/Berlin')
    || !egress?.includes(egressProfile.name)
    || !labels?.includes('purpose=import-repro')
    || !labels.includes('team=support')
    || !integration?.includes('source=admin-smoke-template')
  ) {
    throw new Error(`Unexpected configured session detail facts: ${ownerMode} / ${idleTimeout} / ${detailProject} / ${detailAdmission} / ${detailTemplate} / ${detailBrowserContext} / ${network} / ${egress} / ${labels} / ${integration}`);
  }
  await page.getByTestId('session-file-bindings').waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
}

async function verifyInspectorTemplateFilter(page, options, sessionId, template, project, browserContext, egressProfile) {
  await page.goto(adminRouteUrl(options, 'sessions'), { waitUntil: 'domcontentloaded' });
  await page.getByTestId('session-inspector-list').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await selectOptionWhenAvailable(page, page.getByTestId('session-inspector-template-filter'), template.id, options);
  const row = page.locator(`[data-testid="session-inspector-row"][data-session-id="${sessionId}"]`);
  await row.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  const rowTemplate = await row.getByTestId('session-inspector-row-template').textContent();
  const rowProject = await row.getByTestId('session-inspector-row-project').textContent();
  const rowAdmission = await row.getByTestId('session-inspector-row-admission').textContent();
  const rowBrowserContext = await poll(
    'session inspector row browser context name',
    async () => await row.getByTestId('session-inspector-row-browser-context').textContent(),
    (value) => value?.includes(browserContext.name) === true,
    options.connectTimeoutMs,
    100,
  );
  const rowEgress = await row.getByTestId('session-inspector-row-egress').textContent();
  if (!rowTemplate?.includes(template.name)) {
    throw new Error(`Expected inspector row template ${template.name}, got ${rowTemplate}`);
  }
  if (!rowProject?.includes(project.name)) {
    throw new Error(`Expected inspector row project ${project.name}, got ${rowProject}`);
  }
  if (!rowAdmission?.includes('project_quota_available')) {
    throw new Error(`Expected inspector row project admission, got ${rowAdmission}`);
  }
  if (!rowBrowserContext?.includes(browserContext.name)) {
    throw new Error(`Expected inspector row browser context ${browserContext.name}, got ${rowBrowserContext}`);
  }
  if (!rowEgress?.includes(egressProfile.name)) {
    throw new Error(`Expected inspector row egress profile ${egressProfile.name}, got ${rowEgress}`);
  }
  const count = await page.getByTestId('session-inspector-count').textContent();
  if (!count?.includes('visible sessions')) {
    throw new Error(`Expected inspector filter count to be visible, got ${count}`);
  }

  await page.goto(options.pageUrl, { waitUntil: 'domcontentloaded' });
  await openAdminTab(page, 'sessions');
  const liveRow = page.locator(`[data-testid="session-row"][data-session-id="${sessionId}"]`);
  await liveRow.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  await liveRow.click();
  const selectedProject = await page.getByTestId('session-selected-project').textContent();
  const selectedAdmission = await page.getByTestId('session-selected-admission').textContent();
  const selectedTemplate = await page.getByTestId('session-selected-template').textContent();
  const selectedBrowserContext = await poll(
    'live selected session browser context name',
    async () => await page.getByTestId('session-selected-browser-context').textContent(),
    (value) => value?.includes(browserContext.name) === true,
    options.connectTimeoutMs,
    100,
  );
  const selectedEgress = await page.getByTestId('session-selected-egress').textContent();
  if (!selectedTemplate?.includes(template.name)) {
    throw new Error(`Expected live selected session template ${template.name}, got ${selectedTemplate}`);
  }
  if (!selectedProject?.includes(project.name)) {
    throw new Error(`Expected live selected session project ${project.name}, got ${selectedProject}`);
  }
  if (!selectedAdmission?.includes('project_quota_available')) {
    throw new Error(`Expected live selected session admission, got ${selectedAdmission}`);
  }
  if (!selectedBrowserContext?.includes(browserContext.name)) {
    throw new Error(`Expected live selected session browser context ${browserContext.name}, got ${selectedBrowserContext}`);
  }
  if (!selectedEgress?.includes(egressProfile.name)) {
    throw new Error(`Expected live selected session egress ${egressProfile.name}, got ${selectedEgress}`);
  }
}

function verifyCreatedSession(session, sessionId, template, project, browserContext, egressProfile) {
  if (session.id !== sessionId) {
    throw new Error(`Expected API session ${sessionId}, got ${session.id}`);
  }
  if (session.template_id !== template.id) {
    throw new Error(`Expected template_id ${template.id}, got ${session.template_id}`);
  }
  if (
    session.project_id !== project.id
    || session.project?.id !== project.id
    || session.admission?.state !== 'allowed'
    || session.admission?.reason_code !== 'project_quota_available'
  ) {
    throw new Error(`Expected project admission for ${project.id}, got ${JSON.stringify({
      project_id: session.project_id,
      project: session.project,
      admission: session.admission,
    })}`);
  }
  if (session.owner_mode !== 'collaborative') {
    throw new Error(`Expected collaborative owner mode, got ${session.owner_mode}`);
  }
  if (session.browser_context?.mode !== 'reusable' || session.browser_context?.context_id !== browserContext.id) {
    throw new Error(`Expected reusable browser context ${browserContext.id}, got ${JSON.stringify(session.browser_context)}`);
  }
  if (
    session.network_identity?.locale !== 'de-DE'
    || session.network_identity?.timezone !== 'Europe/Berlin'
    || session.network_identity?.egress_profile_id !== egressProfile.id
    || session.effective_egress?.profile_id !== egressProfile.id
  ) {
    throw new Error(`Expected configured network identity and egress, got ${JSON.stringify(session.network_identity)} / ${JSON.stringify(session.effective_egress)}`);
  }
  if (session.idle_timeout_sec !== 1200) {
    throw new Error(`Expected template idle timeout 1200, got ${session.idle_timeout_sec}`);
  }
  if (session.labels?.case !== '1234' || session.labels?.purpose !== 'import-repro' || session.labels?.team !== 'support') {
    throw new Error(`Expected configured labels, got ${JSON.stringify(session.labels)}`);
  }
  if (session.integration_context?.source !== 'admin-smoke-template') {
    throw new Error(`Expected template integration context, got ${JSON.stringify(session.integration_context)}`);
  }
}

async function createProject(page, options) {
  const accessToken = await getAdminAccessToken(page);
  const name = `Admin smoke project ${Date.now()}`;
  return await fetchJson(`${apiOrigin(options)}/api/v1/projects`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'content-type': 'application/json',
    },
    body: JSON.stringify({
      name,
      description: 'Admin configurator smoke project',
      labels: { suite: 'admin-session-configurator-smoke' },
      quotas: {
        max_active_sessions: 1,
        max_active_workflow_runs: 2,
        max_retained_storage_bytes: 1048576,
      },
    }),
  });
}

async function createEgressProfile(page, options) {
  const accessToken = await getAdminAccessToken(page);
  const name = `Admin smoke egress ${Date.now()}`;
  return await fetchJson(`${apiOrigin(options)}/api/v1/egress-profiles`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'content-type': 'application/json',
    },
    body: JSON.stringify({
      name,
      description: 'Admin configurator smoke egress profile',
      labels: { suite: 'admin-session-configurator-smoke' },
      proxy: { url: 'https://proxy.example:8443' },
      bypass_rules: ['localhost', '*.internal.example'],
      custom_ca: {
        certificate_ref: 'vault://pki/browserpane/eu-support',
        display_name: 'EU support CA',
      },
    }),
  });
}

async function createSessionTemplate(page, options, egressProfile) {
  const accessToken = await getAdminAccessToken(page);
  const name = `Admin smoke template ${Date.now()}`;
  return await fetchJson(`${apiOrigin(options)}/api/v1/session-templates`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'content-type': 'application/json',
    },
    body: JSON.stringify({
      name,
      description: 'Admin configurator smoke template',
      labels: { suite: 'admin-session-configurator-smoke' },
      defaults: {
        owner_mode: 'collaborative',
        idle_timeout_sec: 1200,
        labels: { team: 'support' },
        integration_context: { source: 'admin-smoke-template' },
        network_identity: {
          locale: 'de-DE',
          languages: ['de-DE', 'en-US'],
          timezone: 'Europe/Berlin',
          egress_profile_id: egressProfile.id,
        },
      },
    }),
  });
}

async function selectProject(page, project, options) {
  await selectOptionWhenAvailable(page, page.getByTestId('session-create-project'), project.id, options);
}

async function selectSessionTemplate(page, template, options) {
  const selector = page.getByTestId('session-create-template');
  await selectOptionWhenAvailable(page, selector, template.id, options);
}

async function selectBrowserContext(page, browserContext, options) {
  await page.getByTestId('session-create-browser-context-mode').selectOption('reusable');
  await selectOptionWhenAvailable(
    page,
    page.getByTestId('session-create-browser-context-id'),
    browserContext.id,
    options,
  );
}

async function createBrowserContextThroughUi(page, options) {
  const name = `Admin smoke context ${Date.now()}`;
  await page.getByTestId('session-create-context-name').fill(name);
  await page.getByTestId('session-create-context-labels').fill('suite=admin-session-configurator-smoke');
  await page.getByTestId('session-create-context-retention-days').fill('7');
  await page.getByTestId('session-create-context-max-profile-mb').fill('128');
  await page.getByTestId('session-create-context-create').click();
  await poll(
    'browser context quick-create selection',
    async () => ({
      mode: await page.getByTestId('session-create-browser-context-mode').inputValue(),
      contextId: await page.getByTestId('session-create-browser-context-id').inputValue().catch(() => ''),
    }),
    (value) => value.mode === 'reusable' && Boolean(value.contextId),
    options.connectTimeoutMs,
    100,
  );
  const created = await fetchBrowserContextByName(page, options, name);
  if (!created) {
    throw new Error(`Browser context quick-create did not create ${name}.`);
  }
  if (created.retention_sec !== 604800 || created.max_profile_storage_bytes !== 134217728) {
    throw new Error(`Browser context quick-create did not persist retention/storage limit: ${JSON.stringify(created)}`);
  }
  return created;
}

async function fetchBrowserContextByName(page, options, name) {
  const accessToken = await getAdminAccessToken(page);
  const response = await fetchJson(`${apiOrigin(options)}/api/v1/browser-contexts`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
  return response.contexts?.find((context) => context.name === name) ?? null;
}

async function selectOptionWhenAvailable(page, selector, value, options) {
  await selector.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  await poll(
    `select option ${value}`,
    async () => await selector.locator(`option[value="${value}"]`).count(),
    (count) => count > 0,
    options.connectTimeoutMs,
    100,
  );
  await selector.selectOption(value);
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

async function cleanupCreatedBrowserContext(page, options, contextId, log) {
  if (!contextId) {
    return;
  }
  const accessToken = await getAdminAccessToken(page).catch(() => '');
  if (!accessToken) {
    log(`Skipped cleanup for browser context ${contextId}; no admin access token is available.`);
    return;
  }
  await fetchJson(`${apiOrigin(options)}/api/v1/browser-contexts/${contextId}`, {
    method: 'DELETE',
    headers: { Authorization: `Bearer ${accessToken}` },
  }).catch((error) => {
    log(`Browser context cleanup for ${contextId} failed: ${error.message}`);
  });
}

async function cleanupCreatedProject(page, options, project, log) {
  if (!project?.id) {
    return;
  }
  const accessToken = await getAdminAccessToken(page).catch(() => '');
  if (!accessToken) {
    log(`Skipped cleanup for project ${project.id}; no admin access token is available.`);
    return;
  }
  await fetchJson(`${apiOrigin(options)}/api/v1/projects/${project.id}`, {
    method: 'PUT',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'content-type': 'application/json',
    },
    body: JSON.stringify({
      name: project.name,
      description: project.description,
      labels: project.labels ?? {},
      quotas: project.quotas ?? {},
      state: 'archived',
    }),
  }).catch((error) => {
    log(`Project cleanup for ${project.id} failed: ${error.message}`);
  });
}

async function waitForSessionDetailUrl(page, options) {
  await page.waitForURL(/\/sessions\/[^/]+$/, { timeout: options.connectTimeoutMs });
  const sessionId = decodeURIComponent(new URL(page.url()).pathname.split('/').filter(Boolean).at(-1) ?? '');
  if (!sessionId) {
    throw new Error(`Could not resolve session id from ${page.url()}`);
  }
  return sessionId;
}

async function emitSummary(page, options, session, template, project, log) {
  const summary = {
    pageUrl: options.pageUrl,
    sessionId: session.id,
    projectId: project.id,
    projectName: project.name,
    templateId: template.id,
    templateName: template.name,
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
