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
  appendRunLog,
  createWorkflow,
  createWorkflowVersion,
  issueAutomationAccess,
  transitionRun,
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

const ARTIFACT_NAME = 'workflow-detail-evidence.txt';
const ARTIFACT_BYTES = Buffer.from('BrowserPane workflow run detail evidence\n', 'utf8');

async function run() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-admin-unified-workflow-runs-smoke.mjs');
  if (options.pageUrl === DEFAULTS.pageUrl) {
    options.pageUrl = `${DEFAULTS.pageUrl}/admin-new/workflow-runs`;
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
    const workspace = await createFileWorkspace(accessToken, rootUrl);
    const workflow = await createWorkflow(accessToken, rootUrl);
    await createWorkflowVersion(accessToken, rootUrl, workflow.id, {
      allowedFileWorkspaceIds: [workspace.id],
    });
    const session = await createSession(accessToken, rootUrl);
    createdSessionId = session.id;
    const automation = await issueAutomationAccess(accessToken, rootUrl, session.id);

    const awaitingRun = await createRun(
      accessToken,
      rootUrl,
      workflow.id,
      session.id,
      'awaiting-input',
    );
    await transitionRun(automation.token, rootUrl, awaitingRun.id, {
      state: 'running',
      message: 'Admin-new detail smoke executor attached',
    });
    await appendRunLog(
      automation.token,
      rootUrl,
      awaitingRun.id,
      'Waiting for an operator approval in admin-new.',
    );
    await transitionRun(automation.token, rootUrl, awaitingRun.id, {
      state: 'awaiting_input',
      message: 'Admin-new detail smoke requires approval',
      data: {
        intervention_request: {
          request_id: crypto.randomUUID(),
          kind: 'approval',
          prompt: 'Approve the admin-new workflow run detail smoke',
        },
        runtime_hold: { mode: 'live', timeout_sec: 60 },
      },
    });

    const succeededRun = await createRun(
      accessToken,
      rootUrl,
      workflow.id,
      session.id,
      'succeeded',
    );
    await transitionRun(automation.token, rootUrl, succeededRun.id, {
      state: 'running',
      message: 'Successful detail smoke started',
    });
    await appendRunLog(
      automation.token,
      rootUrl,
      succeededRun.id,
      'Writing the retained workflow evidence artifact.',
    );
    const producedFile = await uploadProducedFile(
      automation.token,
      rootUrl,
      succeededRun.id,
      workspace.id,
    );
    await transitionRun(automation.token, rootUrl, succeededRun.id, {
      state: 'succeeded',
      output: { artifact_file_id: producedFile.file_id, verified: true },
      message: 'Successful detail smoke completed',
    });

    const failedRun = await createRun(
      accessToken,
      rootUrl,
      workflow.id,
      session.id,
      'failed',
    );
    await transitionRun(automation.token, rootUrl, failedRun.id, {
      state: 'running',
      message: 'Failed detail smoke started',
    });
    await appendRunLog(
      automation.token,
      rootUrl,
      failedRun.id,
      'A deterministic validation failure is being recorded.',
    );
    await transitionRun(automation.token, rootUrl, failedRun.id, {
      state: 'failed',
      error: 'Smoke validation rejected the deterministic fixture.',
      message: 'Failed detail smoke completed',
    });

    log('Verifying catalog navigation and awaiting-input operations.');
    await verifyCatalog(page, options, workflow.id, session.id, awaitingRun.id);
    await verifyAwaitingDetail(page, options, workflow.id, session.id, awaitingRun.id);

    await page.getByTestId('workflow-run-detail-operator-input').fill('{invalid');
    await page.getByTestId('workflow-run-detail-operator-input-help').filter({ hasText: 'Enter valid JSON' }).waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });
    if (await page.getByTestId('workflow-run-detail-submit-input').isEnabled()) {
      throw new Error('Submit input must be disabled for invalid JSON.');
    }
    await page.getByTestId('workflow-run-detail-operator-input').fill('{"approved":true}');
    await page.getByTestId('workflow-run-detail-submit-input').click();
    await waitForDetailState(page, options, 'running');
    await page.getByTestId('workflow-run-detail-action-success').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });
    await transitionRun(automation.token, rootUrl, awaitingRun.id, {
      state: 'succeeded',
      output: { approved: true },
      message: 'Awaiting-input detail smoke completed',
    });
    await page.getByTestId('workflow-run-detail-refresh').click();
    await waitForDetailState(page, options, 'succeeded');

    log('Verifying terminal evidence, exact artifact download, and alias routing.');
    await verifySucceededDetail(page, options, succeededRun.id);
    await verifyDownload(page, options);
    await verifyFailedDetail(page, options, failedRun.id);
    await verifyAliasRoute(page, options, succeededRun.id);

    await page.setViewportSize({ width: 390, height: 844 });
    await page.goto(adminRouteUrl(options, `workflow-runs/${failedRun.id}`), {
      waitUntil: 'domcontentloaded',
    });
    await waitForDetailState(page, options, 'failed');
    await assertNoBodyHorizontalOverflow(page, 'narrow workflow run detail');
    await assertNoHorizontalOverflow(
      page,
      'workflow-run-detail-inspector',
      'narrow workflow run inspector',
    );

    summary = {
      pageUrl: options.pageUrl,
      workflowId: workflow.id,
      awaitingRunId: awaitingRun.id,
      succeededRunId: succeededRun.id,
      failedRunId: failedRun.id,
      producedFileId: producedFile.file_id,
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

async function createFileWorkspace(accessToken, rootUrl) {
  return await fetchJson(`${rootUrl}/api/v1/file-workspaces`, {
    method: 'POST',
    headers: jsonAuthHeaders(accessToken),
    body: JSON.stringify({
      name: `admin-unified-run-evidence-${Date.now()}`,
      description: 'Produced-file destination for the admin-new workflow run detail smoke.',
      labels: { suite: 'admin-unified-workflow-runs-smoke' },
    }),
  });
}

async function createSession(accessToken, rootUrl) {
  return await fetchJson(`${rootUrl}/api/v1/sessions`, {
    method: 'POST',
    headers: jsonAuthHeaders(accessToken),
    body: JSON.stringify({ labels: { suite: 'admin-unified-workflow-runs-smoke' } }),
  });
}

async function createRun(accessToken, rootUrl, workflowId, sessionId, scenario) {
  return await fetchJson(`${rootUrl}/api/v1/workflow-runs`, {
    method: 'POST',
    headers: jsonAuthHeaders(accessToken),
    body: JSON.stringify({
      workflow_id: workflowId,
      version: 'v1',
      session: { existing_session_id: sessionId },
      input: { task: 'unified workflow run detail smoke', scenario },
      client_request_id: `admin-unified-workflow-run-${scenario}-${Date.now()}`,
      labels: { source: 'admin-unified-workflow-runs-smoke', scenario },
    }),
  });
}

async function uploadProducedFile(automationToken, rootUrl, runId, workspaceId) {
  return await fetchJson(`${rootUrl}/api/v1/workflow-runs/${runId}/produced-files`, {
    method: 'POST',
    headers: {
      'x-bpane-automation-access-token': automationToken,
      'x-bpane-workflow-workspace-id': workspaceId,
      'x-bpane-file-name': ARTIFACT_NAME,
      'x-bpane-file-provenance': JSON.stringify({ suite: 'admin-unified-workflow-runs-smoke' }),
      'content-type': 'text/plain',
    },
    body: ARTIFACT_BYTES,
  });
}

async function verifyCatalog(page, options, workflowId, sessionId, runId) {
  await page.goto(adminRouteUrl(options, 'workflow-runs'), { waitUntil: 'domcontentloaded' });
  await page.getByTestId('workflow-runs-overview').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await assertNoBodyHorizontalOverflow(page, 'unified workflow runs');
  await assertNoHorizontalOverflow(page, 'workflow-runs-overview', 'unified workflow runs overview');
  await page.getByTestId('workflow-runs-search').fill(runId);
  const row = await waitForRunRow(page, options, runId);
  await waitForContains(row, options, 'workflow-runs-workflow-link', workflowId);
  await waitForContains(row, options, 'workflow-runs-session-link', sessionId.slice(0, 12));

  assertHrefIncludes(
    await row.getByTestId('workflow-runs-open-detail').getAttribute('href'),
    `/admin-new/workflow-runs/${runId}`,
    'workflow run detail',
  );
  assertHrefIncludes(
    await row.getByTestId('workflow-runs-open-session').getAttribute('href'),
    `/admin-new/sessions/${sessionId}`,
    'session',
  );
  assertHrefIncludes(
    await row.getByTestId('workflow-runs-open-workflow').getAttribute('href'),
    `/admin-new/workflows/${workflowId}`,
    'workflow',
  );
  await row.getByTestId('workflow-runs-open-detail').click();
  await page.waitForURL(new RegExp(`/admin-new/workflow-runs/${runId}$`), {
    timeout: options.connectTimeoutMs,
  });
}

async function verifyAwaitingDetail(page, options, workflowId, sessionId, runId) {
  await waitForDetailState(page, options, 'awaiting_input');
  await waitForContains(
    page,
    options,
    'workflow-run-detail-pending-prompt',
    'Approve the admin-new workflow run detail smoke',
  );
  await waitForMinimumCount(page, options, 'workflow-run-detail-event-count', 1);
  await waitForMinimumCount(page, options, 'workflow-run-detail-log-count', 1);
  assertHrefIncludes(
    await page.getByTestId('workflow-run-detail-workflow-link').getAttribute('href'),
    `/admin-new/workflows/${workflowId}`,
    'detail workflow',
  );
  assertHrefIncludes(
    await page.getByTestId('workflow-run-detail-session-link').getAttribute('href'),
    `/admin-new/sessions/${sessionId}`,
    'detail session',
  );
  assertHrefIncludes(
    await page.getByTestId('workflow-run-detail-preview-link').getAttribute('href'),
    `/admin-new/sessions/${sessionId}/preview`,
    'detail session preview',
  );
  await page.reload({ waitUntil: 'domcontentloaded' });
  await waitForDetailState(page, options, 'awaiting_input');
  if (new URL(page.url()).pathname !== `/admin-new/workflow-runs/${runId}`) {
    throw new Error(`Canonical deep link changed after refresh: ${page.url()}`);
  }
}

async function verifySucceededDetail(page, options, runId) {
  await page.goto(adminRouteUrl(options, `workflow-runs/${runId}`), {
    waitUntil: 'domcontentloaded',
  });
  await waitForDetailState(page, options, 'succeeded');
  await waitForMinimumCount(page, options, 'workflow-run-detail-event-count', 1);
  await waitForMinimumCount(page, options, 'workflow-run-detail-log-count', 1);
  await waitForMinimumCount(page, options, 'workflow-run-detail-produced-file-count', 1);
  await waitForContains(page, options, 'workflow-run-detail-output', 'artifact_file_id');
  if (await page.getByTestId('workflow-run-detail-cancel').isEnabled()) {
    throw new Error('Cancel must be disabled for a succeeded workflow run.');
  }
}

async function verifyDownload(page, options) {
  const downloadPromise = page.waitForEvent('download', { timeout: options.connectTimeoutMs });
  await page.getByTestId('workflow-run-detail-download-produced-file').click();
  const download = await downloadPromise;
  if (download.suggestedFilename() !== ARTIFACT_NAME) {
    throw new Error(`Expected ${ARTIFACT_NAME}, got ${download.suggestedFilename()}`);
  }
  const downloadPath = await download.path();
  if (!downloadPath) {
    throw new Error('Playwright did not expose the produced-file download path.');
  }
  const downloadedBytes = await fs.readFile(downloadPath);
  if (!downloadedBytes.equals(ARTIFACT_BYTES)) {
    throw new Error('Produced-file download bytes differ from the uploaded fixture.');
  }
  await page.getByTestId('workflow-run-detail-download-success').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
}

async function verifyFailedDetail(page, options, runId) {
  await page.goto(adminRouteUrl(options, `workflow-runs/${runId}`), {
    waitUntil: 'domcontentloaded',
  });
  await waitForDetailState(page, options, 'failed');
  await waitForContains(
    page,
    options,
    'workflow-run-detail-terminal-error',
    'Smoke validation rejected',
  );
  if (await page.getByTestId('workflow-run-detail-cancel').isEnabled()) {
    throw new Error('Cancel must be disabled for a failed workflow run.');
  }
  if (await page.getByTestId('workflow-run-detail-submit-input').isEnabled()) {
    throw new Error('Submit input must be disabled for a failed workflow run.');
  }
}

async function verifyAliasRoute(page, options, runId) {
  await page.goto(adminRouteUrl(options, `runs/${runId}`), { waitUntil: 'domcontentloaded' });
  await waitForDetailState(page, options, 'succeeded');
  if (new URL(page.url()).pathname !== `/admin-new/runs/${runId}`) {
    throw new Error(`Compatibility alias changed unexpectedly: ${page.url()}`);
  }
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

async function waitForDetailState(page, options, expected) {
  await poll(
    `workflow run detail state ${expected}`,
    async () => await page.getByTestId('workflow-run-detail-state').textContent().catch(() => ''),
    (value) => value === expected,
    options.connectTimeoutMs,
  );
}

async function waitForMinimumCount(page, options, testId, minimum) {
  await poll(
    `${testId} >= ${minimum}`,
    async () => Number.parseInt(
      await page.getByTestId(testId).textContent().catch(() => '0'),
      10,
    ),
    (value) => Number.isFinite(value) && value >= minimum,
    options.connectTimeoutMs,
  );
}

function assertHrefIncludes(actual, expected, label) {
  if (!actual?.includes(expected)) {
    throw new Error(`Expected ${label} link containing ${expected}, got ${actual}`);
  }
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
