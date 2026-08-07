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
  authJsonHeaders as jsonAuthHeaders,
  waitForContains,
} from './admin-unified-smoke-lib.mjs';
import {
  createWorkflow,
  createWorkflowVersion,
} from './admin-workflow-smoke-lib.mjs';
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
  const options = parseSmokeArgs(process.argv.slice(2), 'run-admin-unified-workflow-runs-smoke.mjs');
  if (options.pageUrl === DEFAULTS.pageUrl) {
    options.pageUrl = `${DEFAULTS.pageUrl}/admin-new/runs`;
  }
  const log = createLogger('admin-unified-workflow-runs-smoke');
  const browser = await launchChrome(chromium, options);
  const context = await browser.newContext({ viewport: { width: 1440, height: 980 } });
  const page = await context.newPage();
  let createdSessionId = '';
  let summary = null;

  try {
    log(`Opening ${options.pageUrl}`);
    await ensureAdminLoggedIn(page, options);
    const accessToken = await getAdminAccessToken(page);
    if (!accessToken) {
      throw new Error('No admin access token available after login.');
    }

    const rootUrl = apiOrigin(options);
    const workflow = await createWorkflow(accessToken, rootUrl);
    await createWorkflowVersion(accessToken, rootUrl, workflow.id);
    const session = await createSession(accessToken, rootUrl);
    createdSessionId = session.id;
    const runResource = await createRun(accessToken, rootUrl, workflow.id, session.id);

    await page.goto(adminRouteUrl(options, 'runs'), { waitUntil: 'domcontentloaded' });
    await page.getByTestId('workflow-runs-overview').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });
    await waitForContains(page, options, 'workflow-runs-metric-total', '');
    await assertNoBodyHorizontalOverflow(page, 'unified workflow runs');
    await assertNoHorizontalOverflow(page, 'workflow-runs-overview', 'unified workflow runs overview');

    await page.getByTestId('workflow-runs-search').fill(runResource.id);
    const row = await waitForRunRow(page, options, runResource.id);
    await waitForContains(row, options, 'workflow-runs-workflow-link', workflow.id);
    await waitForContains(row, options, 'workflow-runs-session-link', session.id.slice(0, 12));

    const sessionHref = await row.getByTestId('workflow-runs-open-session').getAttribute('href');
    if (!sessionHref?.includes(`/admin-new/sessions/${session.id}`)) {
      throw new Error(`Expected session link for ${session.id}, got ${sessionHref}`);
    }
    const workflowHref = await row.getByTestId('workflow-runs-open-workflow').getAttribute('href');
    if (!workflowHref?.includes(`/admin-new/workflows/${workflow.id}`)) {
      throw new Error(`Expected workflow link for ${workflow.id}, got ${workflowHref}`);
    }

    await page.getByTestId('workflow-runs-search').fill('text that should not match');
    await page.getByTestId('workflow-runs-filter-empty').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });

    summary = {
      pageUrl: options.pageUrl,
      workflowId: workflow.id,
      runId: runResource.id,
      sessionId: session.id,
    };
    await emitSummary(options, summary, log);
  } finally {
    const cleanupToken = await getAdminAccessToken(page).catch(() => '');
    if (cleanupToken && createdSessionId) {
      await deleteSession(cleanupToken, options, createdSessionId).catch((error) => {
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
    headers: jsonAuthHeaders(accessToken),
    body: JSON.stringify({
      labels: { suite: 'admin-unified-workflow-runs-smoke' },
    }),
  });
}

async function createRun(accessToken, rootUrl, workflowId, sessionId) {
  return await fetchJson(`${rootUrl}/api/v1/workflow-runs`, {
    method: 'POST',
    headers: jsonAuthHeaders(accessToken),
    body: JSON.stringify({
      workflow_id: workflowId,
      version: 'v1',
      session: { existing_session_id: sessionId },
      input: { task: 'unified workflow run catalog smoke' },
      client_request_id: `admin-unified-workflow-run-${Date.now()}`,
      labels: { source: 'admin-unified-workflow-runs-smoke' },
    }),
  });
}

async function waitForRunRow(page, options, runId) {
  return await poll(
    `unified workflow run row ${runId}`,
    async () => {
      const row = page.getByTestId('workflow-runs-list-row').filter({ hasText: runId.slice(0, 12) }).first();
      return await row.isVisible().catch(() => false) ? row : null;
    },
    Boolean,
    options.connectTimeoutMs,
  );
}

async function emitSummary(options, summary, log) {
  console.log(JSON.stringify(summary, null, 2));
  if (options.outputPath) {
    await fs.writeFile(options.outputPath, `${JSON.stringify(summary, null, 2)}\n`, 'utf8');
    log(`Wrote summary to ${options.outputPath}`);
  }
}

run().catch((error) => {
  console.error(`[admin-unified-workflow-runs-smoke] ${error.stack || error.message}`);
  process.exitCode = 1;
});
