import fs from 'node:fs/promises';
import process from 'node:process';
import { chromium } from 'playwright-core';
import { cleanupAdminBeforeRun, cleanupAdminSmoke, ensureAdminLoggedIn, getAdminAccessToken, openAdminTab, waitForBrowserConnected } from './admin-smoke-lib.mjs';
import { appendRunLog, createWorkflow, createWorkflowVersion, issueAutomationAccess, transitionRun } from './admin-workflow-smoke-lib.mjs';
import { DEFAULTS, createLogger, launchChrome, parseSmokeArgs, poll } from './workflow-smoke-lib.mjs';

async function run() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-admin-workflow-smoke.mjs');
  if (options.pageUrl === DEFAULTS.pageUrl) {
    options.pageUrl = `${DEFAULTS.pageUrl}/admin/`;
  }
  const rootUrl = new URL('/', options.pageUrl).origin;
  const log = createLogger('admin-workflow-smoke');
  const browser = await launchChrome(chromium, options);
  const context = await browser.newContext({ viewport: { width: 1440, height: 980 } });
  const page = await context.newPage();
  let sessionId = '';

  try {
    log(`Opening ${options.pageUrl}`);
    await ensureAdminLoggedIn(page, options);
    await cleanupAdminBeforeRun(page, options, log);
    const accessToken = await getAdminAccessToken(page);
    const workflow = await createWorkflow(accessToken, rootUrl);
    const version = await createWorkflowVersion(accessToken, rootUrl, workflow.id);

    await openAdminTab(page, 'sessions');
    await page.getByTestId('session-new').click();
    sessionId = await resolveSelectedSessionId(page, options);
    await waitForBrowserConnected(page, options);

    await openAdminTab(page, 'workflows');
    await page.getByTestId('workflow-definition-select').selectOption(workflow.id);
    await waitForEnabled(page.getByTestId('workflow-invoke'), options, 'admin workflow invoke');
    await page.getByTestId('workflow-input').fill('{\n  "task": "admin workflow smoke"\n}');
    await page.getByTestId('workflow-invoke').click();
    const runId = await waitForRunId(page, options);

    const automation = await issueAutomationAccess(accessToken, rootUrl, sessionId);
    await transitionRun(automation.token, rootUrl, runId, { state: 'running', message: 'manual executor attached' });
    await appendRunLog(automation.token, rootUrl, runId, 'admin workflow smoke log');
    await transitionRun(automation.token, rootUrl, runId, {
      state: 'awaiting_input',
      message: 'approval required',
      data: {
        intervention_request: {
          request_id: crypto.randomUUID(),
          kind: 'approval',
          prompt: 'Approve the admin workflow smoke',
        },
        runtime_hold: { mode: 'live', timeout_sec: 5 },
      },
    });

    await refreshAndWaitForState(page, options, 'awaiting_input');
    await waitForEnabled(page.getByTestId('workflow-submit-input'), options, 'admin workflow submit input');
    await page.getByText('Approve the admin workflow smoke').waitFor({ state: 'visible' });
    await page.getByTestId('workflow-intervention-input').fill('{\n  "approved": true\n}');
    await page.getByTestId('workflow-submit-input').click();
    await waitForState(page, options, 'running');

    await transitionRun(automation.token, rootUrl, runId, {
      state: 'awaiting_input',
      message: 'resume required',
      data: {
        intervention_request: {
          request_id: crypto.randomUUID(),
          kind: 'confirmation',
          prompt: 'Release the admin runtime hold',
        },
        runtime_hold: { mode: 'live', timeout_sec: 5 },
      },
    });
    await refreshAndWaitForState(page, options, 'awaiting_input');
    await waitForEnabled(page.getByTestId('workflow-release-hold'), options, 'admin workflow release hold');
    await page.getByTestId('workflow-release-hold').click();
    await waitForState(page, options, 'running');

    await transitionRun(automation.token, rootUrl, runId, { state: 'succeeded', message: 'admin workflow smoke succeeded' });
    await refreshAndWaitForState(page, options, 'succeeded');
    await waitForCount(page, options, 'workflow-log-count', 1);
    await waitForCount(page, options, 'workflow-event-count', 1);
    await openAdminTab(page, 'logs');
    await waitForWorkflowGatewayLog(page, options);
    await emitSummary(options, {
      workflowId: workflow.id,
      version: version.version,
      runId,
      sessionId,
      workflowGatewayLogs: true,
    }, log);
  } finally {
    await cleanupAdminSmoke(page, options, log);
    await context.close();
    await browser.close();
  }
}

async function resolveSelectedSessionId(page, options) {
  const row = page.getByTestId('session-row').first();
  await row.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  const sessionId = await row.getAttribute('data-session-id') ?? '';
  if (!sessionId) {
    throw new Error('Admin session row did not expose a session id.');
  }
  return sessionId;
}

async function waitForRunId(page, options) {
  const text = await poll('admin workflow run id', async () => {
    return await page.getByTestId('workflow-run-id').textContent();
  }, (value) => Boolean(value && !value.includes('--')), options.connectTimeoutMs);
  return text.replace(/^.*Run id:\s*/, '').trim();
}

async function refreshAndWaitForState(page, options, state) {
  await page.getByTestId('workflow-run-refresh').click();
  await waitForState(page, options, state);
}

async function waitForState(page, options, state) {
  await poll(`admin workflow state ${state}`, async () => {
    return await page.getByTestId('workflow-run-state').textContent();
  }, (value) => value === state, options.connectTimeoutMs);
}

async function waitForEnabled(locator, options, description) {
  await poll(description, async () => await locator.isEnabled(), Boolean, options.connectTimeoutMs);
}

async function waitForCount(page, options, testId, minimum) {
  const refresh = page.getByTestId('workflow-run-refresh');
  await poll(`admin workflow ${testId}`, async () => {
    const count = parseCount(await page.getByTestId(testId).textContent());
    if (count < minimum && await refresh.isEnabled().catch(() => false)) {
      await refresh.click();
    }
    return count;
  }, (value) => Number.isFinite(value) && value >= minimum, options.connectTimeoutMs);
}

async function waitForWorkflowGatewayLog(page, options) {
  const workflowLogs = page
    .locator('[data-testid="admin-log-entry"][data-log-source="gateway"]')
    .filter({ hasText: 'workflow snapshot' });
  await poll(
    'admin workflow gateway log entry',
    async () => await workflowLogs.count(),
    (count) => count > 0,
    options.connectTimeoutMs,
  );
}

function parseCount(text) { return Number(text?.match(/\d+/)?.[0] ?? Number.NaN); }
async function emitSummary(options, summary, log) {
  console.log(JSON.stringify(summary, null, 2));
  if (options.outputPath) {
    await fs.writeFile(options.outputPath, `${JSON.stringify(summary, null, 2)}\n`, 'utf8');
    log(`Wrote summary to ${options.outputPath}`);
  }
}

run().catch((error) => {
  console.error(`[admin-workflow-smoke] ${error.stack || error.message}`);
  process.exitCode = 1;
});
