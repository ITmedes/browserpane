import fs from 'node:fs/promises';
import process from 'node:process';
import { chromium } from 'playwright-core';
import {
  ensureAdminLoggedIn,
  getAdminAccessToken,
} from './admin-smoke-lib.mjs';
import {
  adminRouteUrl,
  assertNoBodyHorizontalOverflow,
  assertNoHorizontalOverflow,
  authJsonHeaders,
  waitForContains,
} from './admin-unified-smoke-lib.mjs';
import { createWorkflow, createWorkflowVersion } from './admin-workflow-smoke-lib.mjs';
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
  const options = parseSmokeArgs(process.argv.slice(2), 'run-admin-unified-workflows-smoke.mjs');
  if (options.pageUrl === DEFAULTS.pageUrl) {
    options.pageUrl = `${DEFAULTS.pageUrl}/admin-new/workflows`;
  }
  const log = createLogger('admin-unified-workflows-smoke');
  const browser = await launchChrome(chromium, options);
  const context = await browser.newContext({ viewport: { width: 1440, height: 980 } });
  const page = await context.newPage();
  let summary = null;
  let accessToken = '';
  let createdRunId = '';
  let createdSessionId = '';
  let createdProject = null;

  try {
    log(`Opening ${options.pageUrl}`);
    await ensureAdminLoggedIn(page, options);
    accessToken = await getAdminAccessToken(page);
    if (!accessToken) {
      throw new Error('No admin access token available after login.');
    }

    const hiddenWorkflow = await createWorkflow(accessToken, apiOrigin(options));
    await createWorkflowVersion(accessToken, apiOrigin(options), hiddenWorkflow.id);
    createdProject = await createWorkflowSmokeProject(accessToken, options);

    await page.goto(adminRouteUrl(options, 'workflows'), { waitUntil: 'domcontentloaded' });
    await page.getByTestId('workflows-overview').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });
    await assertNoBodyHorizontalOverflow(page, 'unified workflow catalog');
    await assertNoHorizontalOverflow(page, 'workflows-overview', 'unified workflow overview');

    const hiddenCount = await page.getByText(hiddenWorkflow.name).count();
    if (hiddenCount !== 0) {
      throw new Error(`Hidden smoke workflow ${hiddenWorkflow.name} appeared in the unified catalog.`);
    }
    await waitForContains(page, options, 'workflows-hidden-note', 'internal workflow');
    await page.getByTestId('workflows-search').fill('BrowserPane Tour');
    const tourRow = await waitForCatalogRow(page, options, 'BrowserPane Tour');
    const href = await tourRow.getByTestId('workflows-detail-link').getAttribute('href');
    if (!href?.includes('/admin-new/workflows/')) {
      throw new Error(`Expected BrowserPane Tour detail link, got ${href}`);
    }
    await tourRow.getByTestId('workflows-detail-link').click();
    await page.waitForURL(/\/admin-new\/workflows\/[^/]+$/, { timeout: options.connectTimeoutMs });
    await page.getByTestId('workflow-definition-detail-route').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });
    await waitForContains(page, options, 'workflow-definition-detail-title', 'BrowserPane Tour');
    await waitForContains(page, options, 'workflow-definition-detail-kind', 'Example template');
    await waitForContains(page, options, 'workflow-definition-detail-latest-version', 'v1');
    await waitForContains(page, options, 'workflow-definition-version-entrypoint', 'browserpane-tour');
    await waitForContains(page, options, 'workflow-definition-source', '/workspace');
    await waitForContains(page, options, 'workflow-definition-policy', 'File workspaces');
    await waitForSourceTreePath(page, options, 'dev/workflows/browserpane-tour/run.mjs');
    await waitForSourceTreePath(page, options, 'dev/web-fixtures');
    await waitForContains(page, options, 'workflow-code-preview-code', 'export default async function run');
    await waitForContains(page, options, 'workflow-code-preview-language', 'TypeScript');
    const highlightedKeywordCount = await page
      .getByTestId('workflow-code-preview-code')
      .locator('.hljs-keyword')
      .count();
    if (highlightedKeywordCount === 0) {
      throw new Error('Workflow code preview did not render TypeScript syntax highlighting.');
    }
    await page.locator(sourceTreeSelector('dev/web-fixtures')).click();
    await waitForSourceTreePath(page, options, 'dev/web-fixtures/test-embed.html');
    await page.locator(sourceTreeSelector('dev/web-fixtures/test-embed.html')).click();
    await waitForContains(page, options, 'workflow-code-preview-entrypoint', 'dev/web-fixtures/test-embed.html');
    await waitForContains(page, options, 'workflow-code-preview-code', 'BrowserPane Test Embed');
    const launch = await verifyWorkflowRunLauncher(page, options, createdProject);
    createdRunId = launch.runId;
    createdSessionId = launch.sessionId;
    await assertNoBodyHorizontalOverflow(page, 'unified workflow detail');
    await assertNoHorizontalOverflow(page, 'workflow-definition-detail-route', 'unified workflow detail route');

    await verifyResponsiveDetailLayout(page, options);
    await verifySourceEditor(page, options, log, hiddenWorkflow.id);
    await verifyCreatedRunCatalog(page, options, createdRunId, createdProject.id);

    summary = {
      pageUrl: options.pageUrl,
      hiddenWorkflowId: hiddenWorkflow.id,
      catalogTemplate: 'BrowserPane Tour',
      createdRunId,
      createdSessionId,
      projectId: createdProject.id,
      detailVisible: true,
    };
    await emitSummary(options, summary, log);
  } finally {
    if (accessToken && createdSessionId) {
      await deleteSession(accessToken, options, createdSessionId).catch((error) => {
        log(`cleanup warning: failed to delete session ${createdSessionId}: ${error instanceof Error ? error.message : String(error)}`);
      });
    }
    if (accessToken && createdProject) {
      await archiveProject(accessToken, options, createdProject, log);
    }
    await context.close();
    await browser.close();
  }
}

async function waitForCatalogRow(page, options, text) {
  return await poll(
    `unified workflow catalog row ${text}`,
    async () => {
      const row = page.getByTestId('workflows-list-row').filter({ hasText: text }).first();
      return await row.isVisible().catch(() => false) ? row : null;
    },
    Boolean,
    options.connectTimeoutMs,
  );
}

async function waitForSourceTreePath(page, options, path) {
  await poll(
    `source tree path ${path}`,
    async () => await page.locator(sourceTreeSelector(path)).count(),
    (count) => count > 0,
    options.connectTimeoutMs,
  );
}

function sourceTreeSelector(path) {
  return `[data-source-path="${path}"]`;
}

async function verifyResponsiveDetailLayout(page, options) {
  await page.setViewportSize({ width: 390, height: 900 });
  await page.getByTestId('workflow-definition-detail-route').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await page.getByTestId('workflow-definition-inspector').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await assertNoBodyHorizontalOverflow(page, 'mobile unified workflow detail');
  await assertNoHorizontalOverflow(page, 'workflow-definition-detail-route', 'mobile unified workflow detail route');
  await assertNoHorizontalOverflow(page, 'workflow-definition-inspector', 'mobile unified workflow inspector');
  await page.setViewportSize({ width: 1440, height: 980 });
}

async function verifySourceEditor(page, options, log, workflowId) {
  log(`Opening hidden workflow ${workflowId} for source editor smoke`);
  await page.goto(adminRouteUrl(options, `workflows/${workflowId}`), { waitUntil: 'domcontentloaded' });
  await page.getByTestId('workflow-definition-detail-route').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await page.getByTestId('workflow-source-entrypoint').fill('dev/workflows/browserpane-tour/run.mjs');
  await page.getByTestId('workflow-source-validate').click();
  await waitForContains(page, options, 'workflow-source-validation-ready', 'files available');
  await page.getByTestId('workflow-source-create-version').click();
  await waitForContains(page, options, 'workflow-definition-action-success', 'Workflow version');
  await waitForContains(page, options, 'workflow-definition-selected-version', 'v2');
  await assertNoBodyHorizontalOverflow(page, 'unified workflow source editor');
  await assertNoHorizontalOverflow(page, 'workflow-definition-detail-route', 'unified workflow source editor route');
}

async function verifyWorkflowRunLauncher(page, options, project) {
  await page.getByTestId('workflow-run-launcher').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await page.getByTestId('workflow-run-project-id').selectOption(project.id);
  await waitForContains(page, options, 'workflow-run-project-governance', project.name);
  await waitForContains(page, options, 'workflow-run-project-governance', 'Active runs: 0 / 1');
  await page.getByTestId('workflow-run-input-scroll_delay_ms').fill('30');
  await page.getByTestId('workflow-run-input-scroll_step_px').fill('480');
  await page.getByTestId('workflow-run-input-max_scroll_steps').fill('2');
  await page.getByTestId('workflow-run-start').click();
  await waitForWorkflowLaunch(page, options);
  const runHref = await page.getByTestId('workflow-run-open-run').getAttribute('href');
  const sessionHref = await page.getByTestId('workflow-run-open-session').getAttribute('href');
  const runUrl = new URL(runHref ?? '', apiOrigin(options));
  const runId = runUrl.pathname.match(/^\/admin-new\/workflow-runs\/([^/]+)$/)?.[1] ?? '';
  const sessionId = sessionHref?.split('/').filter(Boolean).at(-1) ?? '';
  if (!runId) {
    throw new Error(`Expected a canonical workflow run detail link, got ${runHref}`);
  }
  if (!sessionId) {
    throw new Error(`Expected workflow run session link, got ${sessionHref}`);
  }
  await assertNoHorizontalOverflow(page, 'workflow-run-launcher', 'workflow run launcher');
  return { runId, sessionId };
}

async function waitForWorkflowLaunch(page, options) {
  const success = page.getByTestId('workflow-run-launch-success');
  const failure = page.getByTestId('workflow-run-launch-error');
  await Promise.race([
    success.waitFor({ state: 'visible', timeout: options.connectTimeoutMs }),
    failure.waitFor({ state: 'visible', timeout: options.connectTimeoutMs }).then(async () => {
      throw new Error(`Workflow launch failed: ${(await failure.textContent())?.trim()}`);
    }),
  ]);
}

async function verifyCreatedRunCatalog(page, options, runId, projectId) {
  await page.goto(adminRouteUrl(options, 'workflow-runs'), { waitUntil: 'domcontentloaded' });
  await page.getByTestId('workflow-runs-overview').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await waitForContains(page, options, 'workflow-runs-integration-panel', 'Start workflow runs from outside');
  await page.getByTestId('workflow-runs-search').fill(runId);
  const row = await poll(
    `created workflow run row ${runId}`,
    async () => {
      const row = page.getByTestId('workflow-runs-list-row').filter({ hasText: runId.slice(0, 12) }).first();
      return await row.isVisible().catch(() => false) ? row : null;
    },
    (candidate) => candidate !== null,
    options.connectTimeoutMs,
  );
  await assertNoBodyHorizontalOverflow(page, 'unified workflow run catalog after launch');
  await assertNoHorizontalOverflow(page, 'workflow-runs-overview', 'unified workflow run catalog after launch');
  await row.getByTestId('workflow-runs-detail-link').click();
  await page.getByTestId('workflow-run-detail-route').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await page.getByTestId('workflow-run-admission-evidence').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  const admissionMessage = page.getByTestId('workflow-run-admission-message');
  if (await admissionMessage.count() === 0) {
    throw new Error(`Expected project admission evidence, got ${(await page.getByTestId('workflow-run-admission-evidence').textContent())?.trim()}`);
  }
  if (!(await admissionMessage.textContent())?.trim()) {
    throw new Error('Expected the workflow run project admission message to be non-empty.');
  }
  await waitForContains(page, options, 'workflow-run-admission-active-workflows', ' / 1');
  const projectHref = await page.getByTestId('workflow-run-detail-project-link').getAttribute('href');
  if (projectHref !== `/admin-new/projects/${projectId}`) {
    throw new Error(`Expected workflow run project link for ${projectId}, got ${projectHref}`);
  }
  await page.setViewportSize({ width: 390, height: 900 });
  await assertNoHorizontalOverflow(page, 'workflow-run-detail-route', 'mobile workflow run detail route');
  await page.setViewportSize({ width: 1440, height: 980 });
}

async function createWorkflowSmokeProject(accessToken, options) {
  const runLabel = `admin-unified-workflows-smoke-${Date.now()}`;
  return await fetchJson(`${apiOrigin(options)}/api/v1/projects`, {
    method: 'POST',
    headers: authJsonHeaders(accessToken),
    body: JSON.stringify({
      name: `Workflow smoke project ${runLabel}`,
      description: 'Bounded project for unified workflow launch and admission evidence.',
      labels: { suite: 'admin-unified-workflows-smoke', run: runLabel },
      state: 'active',
      quotas: { max_active_workflow_runs: 1 },
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
    }),
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

async function emitSummary(options, summary, log) {
  console.log(JSON.stringify(summary, null, 2));
  if (options.outputPath) {
    await fs.writeFile(options.outputPath, `${JSON.stringify(summary, null, 2)}\n`, 'utf8');
    log(`Wrote summary to ${options.outputPath}`);
  }
}

run().catch((error) => {
  console.error(`[admin-unified-workflows-smoke] ${error.stack || error.message}`);
  process.exitCode = 1;
});
