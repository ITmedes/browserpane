import fs from 'node:fs/promises';
import process from 'node:process';
import { chromium } from 'playwright-core';
import {
  cleanupAdminBeforeRun,
  ensureAdminLoggedIn,
  getAdminAccessToken,
} from './admin-smoke-lib.mjs';
import {
  appendRunLog,
  createWorkflow,
  createWorkflowVersion,
  issueAutomationAccess,
  transitionRun,
} from './admin-workflow-smoke-lib.mjs';
import { DEFAULTS, createLogger, deleteSession, fetchJson, launchChrome, parseSmokeArgs, poll } from './workflow-smoke-lib.mjs';

async function run() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-admin-workflow-run-detail-smoke.mjs');
  if (options.pageUrl === DEFAULTS.pageUrl) {
    options.pageUrl = `${DEFAULTS.pageUrl}/admin/`;
  }
  const rootUrl = new URL('/', options.pageUrl).origin;
  const log = createLogger('admin-workflow-run-detail-smoke');
  const browser = await launchChrome(chromium, options);
  const context = await browser.newContext({ viewport: { width: 1440, height: 980 } });
  const page = await context.newPage();
  let summary = null;
  let createdSessionId = '';

  try {
    log(`Opening ${options.pageUrl}`);
    await ensureAdminLoggedIn(page, options);
    await cleanupAdminBeforeRun(page, options, log);
    const accessToken = await getAdminAccessToken(page);
    const workflow = await createWorkflow(accessToken, rootUrl);
    await createWorkflowVersion(accessToken, rootUrl, workflow.id);
    const session = await createSession(accessToken, rootUrl);
    createdSessionId = session.id;
    const runResource = await createRun(accessToken, rootUrl, workflow.id, session.id);
    const automation = await issueAutomationAccess(accessToken, rootUrl, session.id);
    await transitionRun(automation.token, rootUrl, runResource.id, {
      state: 'running',
      message: 'route-level manual executor attached',
    });
    await appendRunLog(automation.token, rootUrl, runResource.id, 'route-level workflow run detail smoke log');
    await transitionRun(automation.token, rootUrl, runResource.id, {
      state: 'awaiting_input',
      message: 'route-level approval required',
      data: {
        intervention_request: {
          request_id: crypto.randomUUID(),
          kind: 'approval',
          prompt: 'Approve the route-level workflow run detail smoke',
        },
        runtime_hold: { mode: 'live', timeout_sec: 30 },
      },
    });

    log('Verifying workflow run list and detail routes.');
    await verifyRunList(page, options, runResource.id);
    await verifyRunDetail(page, options, runResource.id, session.id, workflow.id);
    await page.getByTestId('workflow-run-detail-operator-input').fill('{\n  "approved": true\n}');
    await page.getByTestId('workflow-run-detail-submit-input').click();
    await waitForDetailState(page, options, 'running');
    await transitionRun(automation.token, rootUrl, runResource.id, {
      state: 'succeeded',
      message: 'route-level workflow run detail smoke succeeded',
    });
    await page.getByTestId('workflow-run-inspector-detail-refresh').click();
    await waitForDetailState(page, options, 'succeeded');
    if (await page.getByTestId('workflow-run-detail-cancel').isEnabled()) {
      throw new Error('Cancel should be disabled for a succeeded workflow run.');
    }

    summary = {
      pageUrl: options.pageUrl,
      workflowId: workflow.id,
      runId: runResource.id,
      sessionId: session.id,
      finalState: await page.getByTestId('workflow-run-detail-state').textContent(),
    };
    await emitSummary(options, summary, log);
  } finally {
    const cleanupToken = await getAdminAccessToken(page).catch(() => '');
    if (cleanupToken && createdSessionId) {
      await deleteSession(cleanupToken, { ...options, pageUrl: rootUrl }, createdSessionId).catch((error) => {
        log(`cleanup warning: failed to delete session ${createdSessionId}: ${error instanceof Error ? error.message : String(error)}`);
      });
    }
    await context.close();
    await browser.close();
  }
}

async function createSession(accessToken, rootUrl) {
  return await fetchJson(`${rootUrl}/api/v1/sessions`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${accessToken}`, 'Content-Type': 'application/json' },
    body: JSON.stringify({ labels: { suite: 'admin-workflow-run-detail-smoke' } }),
  });
}

async function createRun(accessToken, rootUrl, workflowId, sessionId) {
  return await fetchJson(`${rootUrl}/api/v1/workflow-runs`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${accessToken}`, 'Content-Type': 'application/json' },
    body: JSON.stringify({
      workflow_id: workflowId,
      version: 'v1',
      session: { existing_session_id: sessionId },
      input: { task: 'route workflow run detail smoke' },
      client_request_id: `admin-workflow-run-detail-${Date.now()}`,
      labels: { source: 'admin-workflow-run-detail-smoke' },
    }),
  });
}

async function verifyRunList(page, options, runId) {
  await page.goto(adminRouteUrl(options, 'workflow-runs'), { waitUntil: 'domcontentloaded' });
  await page.getByTestId('workflow-run-inspector-list').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  const row = page.locator(`[data-testid="workflow-run-inspector-row"][data-run-id="${runId}"]`);
  await row.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  await row.click();
  await page.waitForURL(/\/workflow-runs\/[^/]+$/, { timeout: options.connectTimeoutMs });
}

async function verifyRunDetail(page, options, runId, sessionId, workflowId) {
  await waitForDetailUrl(page, options, runId);
  await page.getByTestId('workflow-run-inspector-detail').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await waitForDetailState(page, options, 'awaiting_input');
  await waitForText(page, options, 'workflow-run-detail-session-id', sessionId);
  await waitForMinimumCount(page, options, 'workflow-run-detail-event-count', 1);
  await waitForMinimumCount(page, options, 'workflow-run-detail-log-list-count', 1);
  await page.getByText('Approve the route-level workflow run detail smoke').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  const sessionHref = await page.getByTestId('workflow-run-session-link').getAttribute('href');
  if (!sessionHref?.includes(`/sessions/${sessionId}`)) {
    throw new Error(`Expected workflow run session link for ${sessionId}, got ${sessionHref}`);
  }
  const workflowHref = await page.getByTestId('workflow-run-definition-link').getAttribute('href');
  if (!workflowHref?.includes(`/workflows/${workflowId}`)) {
    throw new Error(`Expected workflow run definition link for ${workflowId}, got ${workflowHref}`);
  }
}

async function waitForDetailUrl(page, options, runId) {
  await page.waitForURL(new RegExp(`/workflow-runs/${runId}$`), { timeout: options.connectTimeoutMs });
}

async function waitForDetailState(page, options, expected) {
  await poll(
    `workflow run detail state ${expected}`,
    async () => await page.getByTestId('workflow-run-detail-state').textContent(),
    (value) => value === expected,
    options.connectTimeoutMs,
  );
}

async function waitForText(page, options, testId, expected) {
  await poll(
    testId,
    async () => await page.getByTestId(testId).textContent(),
    (value) => value === expected,
    options.connectTimeoutMs,
  );
}

async function waitForMinimumCount(page, options, testId, minimum) {
  await poll(
    testId,
    async () => Number(await page.getByTestId(testId).textContent()),
    (value) => Number.isFinite(value) && value >= minimum,
    options.connectTimeoutMs,
  );
}

function adminRouteUrl(options, routePath) {
  const baseUrl = new URL(options.pageUrl);
  if (!baseUrl.pathname.endsWith('/')) {
    baseUrl.pathname = `${baseUrl.pathname}/`;
  }
  return new URL(routePath, baseUrl).toString();
}

async function emitSummary(options, summary, log) {
  console.log(JSON.stringify(summary, null, 2));
  if (options.outputPath) {
    await fs.writeFile(options.outputPath, `${JSON.stringify(summary, null, 2)}\n`, 'utf8');
    log(`Wrote summary to ${options.outputPath}`);
  }
}

run().catch((error) => {
  console.error(`[admin-workflow-run-detail-smoke] ${error.stack || error.message}`);
  process.exitCode = 1;
});
