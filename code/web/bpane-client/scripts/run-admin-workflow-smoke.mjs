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
    await page.evaluate(() => {
      sessionStorage.setItem('bpane.admin.showHiddenWorkflowDefinitions', 'true');
    });
    await cleanupAdminBeforeRun(page, options, log);
    const accessToken = await getAdminAccessToken(page);
    const workflow = await createWorkflow(accessToken, rootUrl);
    const version = await createWorkflowVersion(accessToken, rootUrl, workflow.id);

    await openAdminTab(page, 'sessions');
    await page.getByTestId('session-refresh').click();
    await openAdminTab(page, 'workflows');
    await page.getByTestId('workflow-definition-select').selectOption(workflow.id);
    await prepareWorkflowBaseline(page, options);
    sessionId = await resolveWorkflowBaselineSessionId(page, options);
    await waitForBrowserConnected(page, options);

    await waitForEnabled(page.getByTestId('workflow-invoke'), options, 'admin workflow invoke');
    await page.getByTestId('workflow-input').fill('{\n  "task": "admin workflow smoke"\n}');
    await page.getByTestId('workflow-invoke').click();
    const runId = await waitForRunId(page, options);
    await waitForText(page, options, 'workflow-run-session-id', sessionId);
    await waitForContains(page, options, 'workflow-run-session-note', 'selected baseline session');

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

async function resolveWorkflowBaselineSessionId(page, options) {
  return await poll(
    'admin workflow baseline session id',
    async () => await page.getByTestId('workflow-session-id').textContent(),
    (value) => Boolean(value && value !== '--'),
    options.connectTimeoutMs,
  );
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

async function waitForText(page, options, testId, expected) {
  await poll(testId, async () => await page.getByTestId(testId).textContent(), (value) => value === expected, options.connectTimeoutMs);
}

async function waitForContains(page, options, testId, expected) {
  await poll(testId, async () => await page.getByTestId(testId).textContent(), (value) => value?.includes(expected), options.connectTimeoutMs);
}

async function prepareWorkflowBaseline(page, options) {
  const state = await poll('workflow baseline prerequisite action', async () => {
    const invokeEnabled = await page.getByTestId('workflow-invoke').isEnabled().catch(() => false);
    const createVisible = await page.getByTestId('workflow-create-session').isVisible().catch(() => false);
    const connectVisible = await page.getByTestId('workflow-connect-session').isVisible().catch(() => false);
    const reason = await page.getByTestId('workflow-invoke-disabled-reason').textContent().catch(() => '');
    return { invokeEnabled, createVisible, connectVisible, reason };
  }, (value) => value.invokeEnabled || value.createVisible || value.connectVisible, options.connectTimeoutMs);
  if (state.invokeEnabled) {
    return;
  }
  if (!state.reason) {
    throw new Error('Workflow invoke was blocked without a visible reason.');
  }
  if (state.createVisible) {
    await page.getByTestId('workflow-create-session').click();
    return;
  }
  if (state.connectVisible) {
    await page.getByTestId('workflow-connect-session').click();
    return;
  }
  throw new Error(`Workflow baseline is blocked without an action: ${state.reason}`);
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
