import fs from 'node:fs/promises';
import process from 'node:process';

import { chromium } from 'playwright-core';

import {
  cleanupWorkflowSmokeSessions,
  configurePage,
  createLogger,
  ensureLoggedIn,
  fetchJson,
  getAccessToken,
  launchChrome,
  parseSmokeArgs,
  poll,
} from './workflow-smoke-lib.mjs';

const log = createLogger('workflow-embed-operations-smoke');

async function createWorkflow(accessToken, options) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflows`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      name: 'workflow-embed-operations-smoke',
      description: 'Validate test-embed operator controls for workflow runs',
      labels: {
        suite: 'workflow-embed-operations-smoke',
      },
    }),
  });
}

async function createWorkflowVersion(accessToken, options, workflowId) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflows/${workflowId}/versions`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      version: 'v1',
      executor: 'manual',
      entrypoint: 'workflows/operator/run.mjs',
    }),
  });
}

async function issueAutomationAccess(accessToken, options, sessionId) {
  return await fetchJson(`${options.pageUrl}/api/v1/sessions/${sessionId}/automation-access`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function transitionRun(automationToken, options, runId, body) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow-runs/${runId}/state`, {
    method: 'POST',
    headers: {
      'x-bpane-automation-access-token': automationToken,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(body),
  });
}

async function refreshRunFromEmbed(page) {
  await page.locator('#btn-workflow-run-refresh').click();
  return await page.evaluate(() => window.__bpaneWorkflow?.getState?.() ?? null);
}

async function main() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-workflow-embed-operations-smoke.mjs');
  const browser = await launchChrome(chromium, options);

  let context = null;
  let page = null;
  let accessToken = '';

  try {
    context = await browser.newContext({
      viewport: { width: 1440, height: 960 },
      deviceScaleFactor: 1,
    });
    page = await context.newPage();
    await configurePage(page, options);
    await ensureLoggedIn(page, options);
    accessToken = (await getAccessToken(page)) ?? '';
    if (!accessToken) {
      throw new Error('Failed to acquire an access token from the test page.');
    }

    await cleanupWorkflowSmokeSessions(accessToken, options, log);

    const workflow = await createWorkflow(accessToken, options);
    const version = await createWorkflowVersion(accessToken, options, workflow.id);
    if (version.executor !== 'manual') {
      throw new Error('Workflow embed operations smoke failed to create the manual version.');
    }

    log('Creating workflow run through test-embed controls');
    const createdRun = await page.evaluate(async ({ workflowId, versionName }) => {
      await window.__bpaneWorkflow.refreshDefinitions({ preserveSelection: false, silent: true });
      await window.__bpaneWorkflow.selectWorkflow(workflowId, { loadVersion: false });
      window.__bpaneWorkflow.setVersion(versionName);
      await window.__bpaneWorkflow.loadVersion({ silent: true });
      return await window.__bpaneWorkflow.invokeSelected({
        silent: true,
        session: {
          create_session: {},
        },
        labels: {
          suite: 'workflow-embed-operations-smoke',
        },
      });
    }, {
      workflowId: workflow.id,
      versionName: version.version,
    });

    const runId = createdRun?.id ?? '';
    const sessionId = createdRun?.session_id ?? '';
    if (!runId || !sessionId) {
      throw new Error('Workflow embed operations smoke did not create a run and session.');
    }

    const automationAccess = await issueAutomationAccess(accessToken, options, sessionId);
    const automationToken = automationAccess.token ?? '';
    if (!automationToken) {
      throw new Error('Workflow embed operations smoke failed to issue automation access.');
    }

    await transitionRun(automationToken, options, runId, {
      state: 'running',
      message: 'executor attached',
    });

    const firstRequestId = crypto.randomUUID();
    await transitionRun(automationToken, options, runId, {
      state: 'awaiting_input',
      message: 'approval required',
      data: {
        intervention_request: {
          request_id: firstRequestId,
          kind: 'approval',
          prompt: 'Approve the embed run',
        },
        runtime_hold: {
          mode: 'live',
          timeout_sec: 5,
        },
      },
    });

    await refreshRunFromEmbed(page);
    const awaitingInputState = await poll(
      'embed awaiting-input visibility',
      async () => await page.evaluate(() => window.__bpaneWorkflow?.getState?.() ?? null),
      (value) =>
        value?.run?.state === 'awaiting_input'
        && value?.run?.intervention?.pending_request?.request_id === firstRequestId
        && value?.run?.runtime?.resume_mode === 'live_runtime',
      options.connectTimeoutMs,
      250,
    );
    if (awaitingInputState.run.runtime?.exact_runtime_available !== true) {
      throw new Error('Embed workflow state did not expose the live runtime hold.');
    }

    const interventionText = await page.locator('#workflow-run-intervention').textContent();
    if (!interventionText?.includes('awaiting approval')) {
      throw new Error(`Workflow panel did not render the pending intervention summary: ${interventionText ?? 'missing'}`);
    }
    const runtimeText = await page.locator('#workflow-run-runtime').textContent();
    if (!runtimeText?.includes('live_runtime')) {
      throw new Error(`Workflow panel did not render the live runtime summary: ${runtimeText ?? 'missing'}`);
    }
    const operatorNoteText = await page.locator('#workflow-operator-note').textContent();
    if (!operatorNoteText?.includes('Awaiting approval')) {
      throw new Error(`Workflow operator note did not render the pending request: ${operatorNoteText ?? 'missing'}`);
    }

    await page.locator('#workflow-operator-input-editor').fill('{\n  "approved": true\n}');
    await page.locator('#workflow-operator-comment-input').fill('approved through embed');
    await page.locator('#btn-workflow-submit-input').click();

    const submittedState = await poll(
      'embed submit-input resolution',
      async () => await page.evaluate(() => window.__bpaneWorkflow?.getState?.() ?? null),
      (value) =>
        value?.run?.state === 'running'
        && value?.run?.intervention?.last_resolution?.action === 'submit_input',
      options.connectTimeoutMs,
      250,
    );
    if (submittedState.run.intervention?.pending_request) {
      throw new Error('Embed submit-input did not clear the pending intervention request.');
    }

    const secondRequestId = crypto.randomUUID();
    await transitionRun(automationToken, options, runId, {
      state: 'awaiting_input',
      message: 'resume required',
      data: {
        intervention_request: {
          request_id: secondRequestId,
          kind: 'confirmation',
          prompt: 'Resume the embed run',
        },
      },
    });

    await refreshRunFromEmbed(page);
    await page.locator('#workflow-operator-comment-input').fill('resume through embed');
    await page.locator('#btn-workflow-resume').click();

    const resumedState = await poll(
      'embed resume resolution',
      async () => await page.evaluate(() => window.__bpaneWorkflow?.getState?.() ?? null),
      (value) =>
        value?.run?.state === 'running'
        && value?.run?.intervention?.last_resolution?.action === 'resume',
      options.connectTimeoutMs,
      250,
    );
    const resumedNoteText = await page.locator('#workflow-operator-note').textContent();
    if (!resumedNoteText?.includes('Last operator action: resume')) {
      throw new Error(`Workflow operator note did not render the resume resolution: ${resumedNoteText ?? 'missing'}`);
    }

    const thirdRequestId = crypto.randomUUID();
    await transitionRun(automationToken, options, runId, {
      state: 'awaiting_input',
      message: 'rejection required',
      data: {
        intervention_request: {
          request_id: thirdRequestId,
          kind: 'approval',
          prompt: 'Reject the embed run',
        },
      },
    });

    await refreshRunFromEmbed(page);
    await page.locator('#workflow-operator-comment-input').fill('rejected through embed');
    await page.locator('#btn-workflow-reject').click();

    const rejectedState = await poll(
      'embed reject resolution',
      async () => await page.evaluate(() => window.__bpaneWorkflow?.getState?.() ?? null),
      (value) =>
        value?.run?.state === 'failed'
        && value?.run?.intervention?.last_resolution?.action === 'reject',
      options.connectTimeoutMs,
      250,
    );
    if (rejectedState.run.intervention?.last_resolution?.reason !== 'rejected through embed') {
      throw new Error('Embed reject did not persist the operator rejection reason.');
    }

    const rejectedStatusText = await page.locator('#workflow-run-status').textContent();
    if (!rejectedStatusText?.includes('failed')) {
      throw new Error(`Workflow panel did not render the failed status: ${rejectedStatusText ?? 'missing'}`);
    }

    const summary = {
      workflowId: workflow.id,
      runId,
      sessionId,
      firstRequestId,
      secondRequestId,
      thirdRequestId,
      finalState: rejectedState.run.state,
      finalAction: rejectedState.run.intervention?.last_resolution?.action ?? null,
    };
    log(`Smoke passed: ${JSON.stringify(summary)}`);
    if (options.outputPath) {
      await fs.writeFile(options.outputPath, `${JSON.stringify(summary, null, 2)}\n`, 'utf8');
    }
  } finally {
    await page?.close().catch(() => {});
    await context?.close().catch(() => {});
    await browser.close().catch(() => {});
  }
}

main().catch((error) => {
  console.error(`[workflow-embed-operations-smoke] ${error?.stack ?? error}`);
  process.exitCode = 1;
});
