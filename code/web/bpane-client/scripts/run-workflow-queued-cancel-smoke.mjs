import process from 'node:process';

import { chromium } from 'playwright-core';

import {
  buildWorkflowWorkerImage,
  cleanupWorkflowSmokeSessions,
  configurePage,
  createLocalWorkflowRepo,
  createLogger,
  ensureLoggedIn,
  fetchJson,
  getAccessToken,
  launchChrome,
  parseSmokeArgs,
  poll,
  recreateComposeServices,
  waitForWorkflowControlPlane,
} from './workflow-smoke-lib.mjs';

const log = createLogger('workflow-queued-cancel-smoke');

function workflowQueuedCancelEntrypoint() {
  return `export default async function run({ page, input, sessionId }) {
  const holdMs =
    input && Number.isFinite(input.hold_ms)
      ? Number(input.hold_ms)
      : 0;
  await page.goto('http://web:8080/test-embed.html', { waitUntil: 'networkidle' });
  if (holdMs > 0) {
    await page.waitForTimeout(holdMs);
  }
  return {
    title: await page.title(),
    hold_ms: holdMs,
    session_id: sessionId,
  };
}
`;
}

async function configureGatewayForQueuedCancellation(accessToken, options) {
  log('Recreating gateway in docker_pool mode with workflow worker backpressure enabled');
  recreateComposeServices(['gateway'], {
    envOverrides: {
      BPANE_GATEWAY_RUNTIME_BACKEND: 'docker_pool',
      BPANE_GATEWAY_MAX_ACTIVE_RUNTIMES: '4',
      BPANE_WORKFLOW_WORKER_MAX_ACTIVE: '1',
    },
  });
  await waitForWorkflowControlPlane(accessToken, options);
}

async function createWorkflow(accessToken, options) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflows`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      name: 'workflow-queued-cancel-smoke',
      description: 'Validate queued workflow cancellation before dispatch',
      labels: {
        suite: 'workflow-queued-cancel-smoke',
      },
    }),
  });
}

async function createWorkflowVersion(accessToken, options, workflowId, source) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflows/${workflowId}/versions`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      version: 'v1',
      executor: 'playwright',
      entrypoint: 'workflows/cancel/run.mjs',
      source: {
        kind: 'git',
        repository_url: source.repositoryUrl,
        ref: 'refs/heads/main',
        root_path: 'workflows',
      },
      default_session: {
        labels: {
          origin: 'workflow-queued-cancel-smoke',
        },
      },
    }),
  });
}

async function createWorkflowRun(accessToken, options, workflowId, holdMs, clientRequestId) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow-runs`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      workflow_id: workflowId,
      version: 'v1',
      session: {
        create_session: {},
      },
      client_request_id: clientRequestId,
      input: {
        hold_ms: holdMs,
      },
      labels: {
        suite: 'workflow-queued-cancel-smoke',
      },
    }),
  });
}

async function fetchWorkflowRun(accessToken, options, runId) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow-runs/${runId}`, {
    headers: {
      Authorization: `Bearer ${accessToken}`,
    },
  });
}

async function fetchWorkflowRunEvents(accessToken, options, runId) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow-runs/${runId}/events`, {
    headers: {
      Authorization: `Bearer ${accessToken}`,
    },
  });
}

async function fetchAutomationTask(accessToken, options, taskId) {
  return await fetchJson(`${options.pageUrl}/api/v1/automation-tasks/${taskId}`, {
    headers: {
      Authorization: `Bearer ${accessToken}`,
    },
  });
}

async function cancelWorkflowRun(accessToken, options, runId) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow-runs/${runId}/cancel`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
    },
  });
}

async function main() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-workflow-queued-cancel-smoke.mjs');
  const browser = await launchChrome(chromium, options);

  let context = null;
  let page = null;
  let accessToken = '';
  let localWorkflowSource = null;

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
      throw new Error('Workflow queued-cancel smoke failed to acquire an access token.');
    }

    await configureGatewayForQueuedCancellation(accessToken, options);
    await cleanupWorkflowSmokeSessions(accessToken, options, log);

    log('Preparing local workflow source');
    localWorkflowSource = await createLocalWorkflowRepo('.workflow-queued-cancel-smoke-repo-', {
      'workflows/cancel/run.mjs': workflowQueuedCancelEntrypoint(),
    });

    log('Building workflow-worker image');
    buildWorkflowWorkerImage();

    const workflow = await createWorkflow(accessToken, options);
    const version = await createWorkflowVersion(
      accessToken,
      options,
      workflow.id,
      localWorkflowSource,
    );
    if (version.source?.resolved_commit !== localWorkflowSource.commit) {
      throw new Error('Queued-cancel workflow version did not pin the expected git commit.');
    }

    const requestPrefix = `workflow-queued-cancel-${Date.now().toString(36)}`;
    log('Creating active run and queued follower');
    const activeRun = await createWorkflowRun(
      accessToken,
      options,
      workflow.id,
      6000,
      `${requestPrefix}-active`,
    );
    await poll(
      'active run enters running state',
      async () => await fetchWorkflowRun(accessToken, options, activeRun.id),
      (run) => run?.state === 'running',
      options.connectTimeoutMs,
      250,
    );

    const queuedRun = await createWorkflowRun(
      accessToken,
      options,
      workflow.id,
      0,
      `${requestPrefix}-queued`,
    );
    const durableQueuedRun = await poll(
      'queued follower admission',
      async () => await fetchWorkflowRun(accessToken, options, queuedRun.id),
      (run) => run?.state === 'queued',
      options.connectTimeoutMs,
      250,
    );

    let cancelledRun;
    try {
      cancelledRun = await cancelWorkflowRun(accessToken, options, durableQueuedRun.id);
    } catch (error) {
      const currentRun = await fetchWorkflowRun(accessToken, options, durableQueuedRun.id);
      const currentTask = durableQueuedRun.automation_task_id
        ? await fetchAutomationTask(accessToken, options, durableQueuedRun.automation_task_id)
        : null;
      if (currentRun.state === 'cancelled') {
        cancelledRun = currentRun;
      } else {
        throw new Error(
          `Queued run cancel failed while run state was ${currentRun.state} and task state was ${currentTask?.state ?? 'unknown'}: ${error instanceof Error ? error.message : String(error)}`,
        );
      }
    }
    if (cancelledRun.state !== 'cancelled') {
      throw new Error(`Expected queued run cancellation, got ${cancelledRun.state ?? 'unknown'}.`);
    }

    const completedActiveRun = await poll(
      'active run completion',
      async () => await fetchWorkflowRun(accessToken, options, activeRun.id),
      (run) => ['succeeded', 'failed', 'cancelled', 'timed_out'].includes(String(run?.state ?? '')),
      30000,
      500,
    );
    if (completedActiveRun.state !== 'succeeded') {
      throw new Error(`Expected active run to succeed, got ${completedActiveRun.state}.`);
    }

    const stableCancelledRun = await poll(
      'queued cancellation remains terminal',
      async () => await fetchWorkflowRun(accessToken, options, durableQueuedRun.id),
      (run) => run?.state === 'cancelled',
      options.connectTimeoutMs,
      250,
    );

    const events = await fetchWorkflowRunEvents(accessToken, options, durableQueuedRun.id);
    const eventTypes = (Array.isArray(events.events) ? events.events : []).map((event) =>
      String(event?.event_type ?? ''),
    );
    for (const expected of ['workflow_run.queued', 'workflow_run.cancel_requested', 'workflow_run.cancelled']) {
      if (!eventTypes.includes(expected)) {
        throw new Error(`Queued cancelled run is missing expected event ${expected}.`);
      }
    }
    for (const unexpected of ['workflow_run.running', 'automation_task.running', 'workflow_run.succeeded']) {
      if (eventTypes.includes(unexpected)) {
        throw new Error(`Queued cancelled run unexpectedly emitted ${unexpected}.`);
      }
    }

    console.log(
      JSON.stringify(
        {
          workflowId: workflow.id,
          activeRunId: activeRun.id,
          activeRunState: completedActiveRun.state,
          queuedRunId: durableQueuedRun.id,
          queuedRunState: stableCancelledRun.state,
          queuedRunEvents: eventTypes,
        },
        null,
        2,
      ),
    );
  } finally {
    await context?.close().catch(() => {});
    await browser.close().catch(() => {});
    if (localWorkflowSource) {
      await localWorkflowSource.cleanup().catch(() => {});
    }
  }
}

main().catch((error) => {
  console.error(
    `[workflow-queued-cancel-smoke] ${error instanceof Error ? error.stack ?? error.message : String(error)}`,
  );
  process.exitCode = 1;
});
