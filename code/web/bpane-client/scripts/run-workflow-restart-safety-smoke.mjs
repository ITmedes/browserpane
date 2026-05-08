import fs from 'node:fs/promises';
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
  restartComposeService,
  waitForWorkflowControlPlane,
} from './workflow-smoke-lib.mjs';

const log = createLogger('workflow-restart-safety-smoke');

function queueWorkflowEntrypoint() {
  return `export default async function run({ page, input, sessionId }) {
  const targetUrl =
    input && typeof input.target_url === 'string' && input.target_url.trim()
      ? input.target_url.trim()
      : 'http://web:8080/test-embed.html';
  const holdMs =
    input && Number.isFinite(input.hold_ms)
      ? Number(input.hold_ms)
      : 0;
  console.log(\`workflow restart queue start \${sessionId}\`);
  await page.goto(targetUrl, { waitUntil: 'networkidle' });
  if (holdMs > 0) {
    await page.waitForTimeout(holdMs);
  }
  return {
    title: await page.title(),
    final_url: page.url(),
    hold_ms: holdMs,
    session_id: sessionId,
  };
}
`;
}

async function createWorkflow(accessToken, options, body) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflows`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(body),
  });
}

async function createWorkflowVersion(accessToken, options, workflowId, body) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflows/${workflowId}/versions`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(body),
  });
}

async function createSession(accessToken, options, body) {
  return await fetchJson(`${options.pageUrl}/api/v1/sessions`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(body),
  });
}

async function createWorkflowRun(accessToken, options, body) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow-runs`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(body),
  });
}

async function fetchWorkflowRun(accessToken, options, runId) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow-runs/${runId}`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function fetchWorkflowRunEvents(accessToken, options, runId) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow-runs/${runId}/events`, {
    headers: { Authorization: `Bearer ${accessToken}` },
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

async function restartGateway(accessToken, options) {
  log('Restarting gateway service');
  restartComposeService('gateway');
  await waitForWorkflowControlPlane(accessToken, options);
}

async function configureGatewayForQueuedRestartSafety(accessToken, options) {
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

async function main() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-workflow-restart-safety-smoke.mjs');
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
      throw new Error('Workflow restart safety smoke failed to acquire an access token.');
    }

    await configureGatewayForQueuedRestartSafety(accessToken, options);
    await cleanupWorkflowSmokeSessions(accessToken, options, log);

    log('Preparing local workflow source for queued-run restart validation');
    localWorkflowSource = await createLocalWorkflowRepo('.workflow-restart-safety-smoke-repo-', {
      'workflows/restart/run.mjs': queueWorkflowEntrypoint(),
    });

    log('Building workflow-worker image');
    buildWorkflowWorkerImage();

    const queueWorkflow = await createWorkflow(accessToken, options, {
      name: 'workflow-restart-safety-queued',
      description: 'Validate queued workflow restart safety and queued cancellation',
      labels: {
        suite: 'workflow-restart-safety-smoke',
        scenario: 'queued',
      },
    });
    const queueVersion = await createWorkflowVersion(accessToken, options, queueWorkflow.id, {
      version: 'v1',
      executor: 'playwright',
      entrypoint: 'workflows/restart/run.mjs',
      source: {
        kind: 'git',
        repository_url: localWorkflowSource.repositoryUrl,
        ref: 'refs/heads/main',
        root_path: 'workflows',
      },
      default_session: {
        labels: {
          origin: 'workflow-restart-safety-smoke',
        },
      },
    });
    if (queueVersion.source?.resolved_commit !== localWorkflowSource.commit) {
      throw new Error('Queued workflow version did not pin the expected local git commit.');
    }

    const requestPrefix = `workflow-restart-safety-${Date.now().toString(36)}`;
    log('Creating active run and queued follower');
    const activeRun = await createWorkflowRun(accessToken, options, {
      workflow_id: queueWorkflow.id,
      version: 'v1',
      session: {
        create_session: {},
      },
      client_request_id: `${requestPrefix}-active`,
      input: {
        target_url: 'http://web:8080/test-embed.html',
        hold_ms: 8000,
      },
      labels: {
        suite: 'workflow-restart-safety-smoke',
        scenario: 'queued',
      },
    });
    const runningActiveRun = await poll(
      'active run enters running state',
      async () => await fetchWorkflowRun(accessToken, options, activeRun.id),
      (run) => run?.state === 'running',
      options.connectTimeoutMs,
      250,
    );
    if (!runningActiveRun.session_id) {
      throw new Error('Active queued-safety run did not expose its session id.');
    }

    const queuedRun = await createWorkflowRun(accessToken, options, {
      workflow_id: queueWorkflow.id,
      version: 'v1',
      session: {
        create_session: {},
      },
      client_request_id: `${requestPrefix}-queued`,
      input: {
        target_url: 'http://web:8080/test-embed.html',
        hold_ms: 0,
      },
      labels: {
        suite: 'workflow-restart-safety-smoke',
        scenario: 'queued',
      },
    });
    const durableQueuedRun = await poll(
      'queued follower admission',
      async () => await fetchWorkflowRun(accessToken, options, queuedRun.id),
      (run) => run?.state === 'queued',
      options.connectTimeoutMs,
      250,
    );

    await restartGateway(accessToken, options);

    const activeRunAfterRestart = await poll(
      'active run terminal state after gateway restart',
      async () => await fetchWorkflowRun(accessToken, options, activeRun.id),
      (run) => ['failed', 'cancelled', 'succeeded', 'timed_out'].includes(String(run?.state ?? '')),
      options.connectTimeoutMs,
      500,
    );

    const resumedQueuedRun = await poll(
      'queued follower survives restart and completes',
      async () => await fetchWorkflowRun(accessToken, options, queuedRun.id),
      (run) => ['succeeded', 'failed', 'cancelled', 'timed_out'].includes(String(run?.state ?? '')),
      30000,
      500,
    );
    if (resumedQueuedRun.state !== 'succeeded') {
      throw new Error(`Expected queued follower run to succeed after restart, got ${resumedQueuedRun.state}.`);
    }
    const queuedEvents = await fetchWorkflowRunEvents(accessToken, options, queuedRun.id);
    const queuedEventTypes = (Array.isArray(queuedEvents.events) ? queuedEvents.events : []).map((event) =>
      String(event?.event_type ?? ''),
    );
    for (const expected of ['workflow_run.queued', 'workflow_run.running', 'workflow_run.succeeded']) {
      if (!queuedEventTypes.includes(expected)) {
        throw new Error(`Queued run is missing expected event ${expected}.`);
      }
    }
    if (
      queuedEventTypes.filter((eventType) => eventType === 'workflow_run.running').length !== 1
    ) {
      throw new Error(`Queued run emitted workflow_run.running multiple times: ${queuedEventTypes.join(', ')}`);
    }
    if (
      queuedEventTypes.filter((eventType) => eventType === 'automation_task.running').length !== 1
    ) {
      throw new Error(`Queued run emitted automation_task.running multiple times: ${queuedEventTypes.join(', ')}`);
    }

    const manualWorkflow = await createWorkflow(accessToken, options, {
      name: 'workflow-restart-safety-awaiting-input',
      description: 'Validate awaiting-input runs survive gateway restarts and remain resolvable',
      labels: {
        suite: 'workflow-restart-safety-smoke',
        scenario: 'awaiting-input',
      },
    });
    const manualVersion = await createWorkflowVersion(accessToken, options, manualWorkflow.id, {
      version: 'v1',
      executor: 'manual',
      entrypoint: 'workflows/manual/run.mjs',
    });
    if (manualVersion.executor !== 'manual') {
      throw new Error('Manual workflow version did not persist the expected executor.');
    }

    const manualRun = await createWorkflowRun(accessToken, options, {
      workflow_id: manualWorkflow.id,
      version: 'v1',
      session: {
        create_session: {},
      },
      labels: {
        suite: 'workflow-restart-safety-smoke',
        scenario: 'awaiting-input',
      },
    });
    if (!manualRun.id || !manualRun.session_id) {
      throw new Error('Manual workflow run did not return run and session ids.');
    }

    const automationAccess = await issueAutomationAccess(accessToken, options, manualRun.session_id);
    const automationToken = automationAccess.token ?? '';
    if (!automationToken) {
      throw new Error('Manual workflow automation access did not return a token.');
    }

    await transitionRun(automationToken, options, manualRun.id, {
      state: 'running',
      message: 'manual executor attached',
    });

    const requestId = crypto.randomUUID();
    await transitionRun(automationToken, options, manualRun.id, {
      state: 'awaiting_input',
      message: 'approval required',
      data: {
        intervention_request: {
          request_id: requestId,
          kind: 'approval',
          prompt: 'Approve the restarted run',
        },
        runtime_hold: {
          mode: 'live',
          timeout_sec: 30,
        },
      },
    });

    await restartGateway(accessToken, options);

    const resumedCandidate = await poll(
      'awaiting-input run survives gateway restart',
      async () => await fetchWorkflowRun(accessToken, options, manualRun.id),
      (run) =>
        run?.state === 'awaiting_input'
        && run?.intervention?.pending_request?.request_id === requestId,
      options.connectTimeoutMs,
      500,
    );
    if (!resumedCandidate.runtime) {
      throw new Error('Awaiting-input run did not expose runtime semantics after gateway restart.');
    }

    const resumedByOwner = await fetchJson(
      `${options.pageUrl}/api/v1/workflow-runs/${manualRun.id}/resume`,
      {
        method: 'POST',
        headers: {
          Authorization: `Bearer ${accessToken}`,
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          comment: 'resume after gateway restart',
        }),
      },
    );
    if (resumedByOwner.state !== 'running') {
      throw new Error(`Expected resumed run to re-enter running, got ${resumedByOwner.state}.`);
    }
    if (resumedByOwner.intervention?.last_resolution?.action !== 'resume') {
      throw new Error('Resumed run did not persist the operator resolution after restart.');
    }

    const replacementAutomationAccess = await issueAutomationAccess(
      accessToken,
      options,
      manualRun.session_id,
    );
    const replacementAutomationToken = replacementAutomationAccess.token ?? '';
    if (!replacementAutomationToken) {
      throw new Error('Replacement automation access was not issued after gateway restart.');
    }

    await transitionRun(replacementAutomationToken, options, manualRun.id, {
      state: 'succeeded',
      message: 'manual executor finished after restart',
      output: {
        restarted: true,
        session_id: manualRun.session_id,
        run_id: manualRun.id,
      },
    });

    const completedManualRun = await poll(
      'awaiting-input run completes after restart',
      async () => await fetchWorkflowRun(accessToken, options, manualRun.id),
      (run) => run?.state === 'succeeded',
      options.connectTimeoutMs,
      500,
    );
    if (completedManualRun.output?.restarted !== true) {
      throw new Error('Manual run did not preserve the expected post-restart output payload.');
    }

    const manualEvents = await fetchWorkflowRunEvents(accessToken, options, manualRun.id);
    const manualEventTypes = (Array.isArray(manualEvents.events) ? manualEvents.events : []).map((event) =>
      String(event?.event_type ?? ''),
    );
    for (const expected of ['workflow_run.awaiting_input', 'workflow_run.resumed', 'workflow_run.succeeded']) {
      if (!manualEventTypes.includes(expected)) {
        throw new Error(`Awaiting-input restart run is missing expected event ${expected}.`);
      }
    }

    const summary = {
      activeRunId: activeRun.id,
      activeRunState: activeRunAfterRestart.state,
      queuedRunId: durableQueuedRun.id,
      queuedRunState: resumedQueuedRun.state,
      queuedRunEvents: queuedEventTypes,
      manualRunId: manualRun.id,
      manualRunState: completedManualRun.state,
      manualRunEvents: manualEventTypes,
    };
    console.log(JSON.stringify(summary, null, 2));
    if (options.outputPath) {
      await fs.writeFile(options.outputPath, `${JSON.stringify(summary, null, 2)}\n`, 'utf8');
    }
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
    `[workflow-restart-safety-smoke] ${error instanceof Error ? error.stack ?? error.message : String(error)}`,
  );
  process.exitCode = 1;
});
