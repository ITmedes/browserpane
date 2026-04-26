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
} from './workflow-smoke-lib.mjs';

const log = createLogger('workflow-reconnect-smoke');

function workflowReconnectEntrypoint() {
  return `export default async function run({ page, input, sessionId, workflowRunId, automationTaskId }) {
  const targetUrl =
    input && typeof input.target_url === 'string' && input.target_url.trim()
      ? input.target_url.trim()
      : 'http://web:8080';
  const runWaitMs =
    input && Number.isFinite(input.run_wait_ms)
      ? Number(input.run_wait_ms)
      : 6000;
  console.log(\`workflow existing-session start \${sessionId}\`);
  await page.goto(targetUrl, { waitUntil: 'networkidle' });
  await page.waitForTimeout(runWaitMs);
  const title = await page.title();
  console.error(\`workflow existing-session finish \${title}\`);
  return {
    title,
    final_url: page.url(),
    session_id: sessionId,
    workflow_run_id: workflowRunId,
    automation_task_id: automationTaskId,
  };
}
`;
}

async function createSession(accessToken, options) {
  return await fetchJson(`${options.pageUrl}/api/v1/sessions`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      labels: {
        suite: 'workflow-reconnect-smoke',
      },
      recording: {
        mode: 'manual',
        format: 'webm',
      },
    }),
  });
}

async function createWorkflow(accessToken, options) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflows`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      name: 'workflow-reconnect-smoke',
      description: 'Validate existing-session workflow runs survive browser client reconnects',
      labels: {
        suite: 'workflow-reconnect-smoke',
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
      entrypoint: 'workflows/reconnect/run.mjs',
      source: {
        kind: 'git',
        repository_url: source.repositoryUrl,
        ref: 'refs/heads/main',
        root_path: 'workflows',
      },
      input_schema: {
        type: 'object',
        required: ['target_url', 'run_wait_ms'],
        properties: {
          target_url: { type: 'string' },
          run_wait_ms: { type: 'number' },
        },
      },
      output_schema: {
        type: 'object',
        required: ['title', 'final_url', 'session_id', 'workflow_run_id', 'automation_task_id'],
      },
    }),
  });
}

async function createWorkflowRun(accessToken, options, workflowId, sessionId) {
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
        existing_session_id: sessionId,
      },
      input: {
        target_url: 'http://web:8080',
        run_wait_ms: 6000,
      },
      labels: {
        suite: 'workflow-reconnect-smoke',
      },
    }),
  });
}

async function fetchWorkflowRun(accessToken, options, runId) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow-runs/${runId}`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function fetchWorkflowRunLogs(accessToken, options, runId) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow-runs/${runId}/logs`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function deleteSession(accessToken, options, sessionId) {
  const response = await fetch(`${options.pageUrl}/api/v1/sessions/${sessionId}`, {
    method: 'DELETE',
    headers: { Authorization: `Bearer ${accessToken}` },
  });
  if (!response.ok && response.status !== 404) {
    const detail = await response.text().catch(() => '');
    throw new Error(`HTTP ${response.status}${detail ? ` ${detail}` : ''}`);
  }
}

async function main() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-workflow-reconnect-smoke.mjs');
  const browser = await launchChrome(chromium, options);

  let ownerContext = null;
  let ownerPage = null;
  let reconnectContext = null;
  let reconnectPage = null;
  let accessToken = '';
  let createdSessionId = '';
  let localWorkflowSource = null;

  try {
    ownerContext = await browser.newContext({
      viewport: { width: 1440, height: 960 },
      deviceScaleFactor: 1,
    });
    ownerPage = await ownerContext.newPage();
    await configurePage(ownerPage, options);
    await ensureLoggedIn(ownerPage, options);
    accessToken = (await getAccessToken(ownerPage)) ?? '';
    if (!accessToken) {
      throw new Error('Failed to acquire an access token from the test page.');
    }
    await cleanupWorkflowSmokeSessions(accessToken, options, log);

    log('Preparing local git-backed workflow source');
    localWorkflowSource = await createLocalWorkflowRepo('.workflow-reconnect-smoke-repo-', {
      'workflows/reconnect/run.mjs': workflowReconnectEntrypoint(),
    });

    log('Building workflow-worker image');
    buildWorkflowWorkerImage();

    const session = await createSession(accessToken, options);
    createdSessionId = session.id ?? '';
    if (!createdSessionId) {
      throw new Error('Existing session creation did not return an id.');
    }

    const workflow = await createWorkflow(accessToken, options);
    const version = await createWorkflowVersion(
      accessToken,
      options,
      workflow.id,
      localWorkflowSource,
    );
    if (version.source?.resolved_commit !== localWorkflowSource.commit) {
      throw new Error('Workflow version did not pin the expected local git commit.');
    }

    log('Creating workflow run bound to an existing session');
    const createdRun = await createWorkflowRun(
      accessToken,
      options,
      workflow.id,
      createdSessionId,
    );
    const runId = createdRun.id ?? '';
    if (!runId) {
      throw new Error('Workflow run creation did not return a run id.');
    }
    if (createdRun.session_id !== createdSessionId) {
      throw new Error('Workflow run did not bind to the requested existing session.');
    }

    const runningRun = await poll(
      'workflow run running state',
      () => fetchWorkflowRun(accessToken, options, runId),
      (candidate) => candidate?.state === 'running',
      options.connectTimeoutMs,
    );
    if (runningRun.session_id !== createdSessionId) {
      throw new Error('Running workflow run is attached to the wrong session.');
    }

    log('Disconnecting the current client while the workflow run stays active');
    await ownerContext.close();
    ownerContext = null;
    ownerPage = null;

    const stillRunning = await fetchWorkflowRun(accessToken, options, runId);
    if (stillRunning.state !== 'running') {
      throw new Error('Workflow run stopped unexpectedly after browser client disconnect.');
    }

    reconnectContext = await browser.newContext({
      viewport: { width: 1440, height: 960 },
      deviceScaleFactor: 1,
    });
    reconnectPage = await reconnectContext.newPage();
    await configurePage(reconnectPage, options);
    await ensureLoggedIn(reconnectPage, options);
    const reconnectAccessToken = (await getAccessToken(reconnectPage)) ?? '';
    if (!reconnectAccessToken) {
      throw new Error('Reconnect page failed to acquire an access token.');
    }

    log('Reconnecting with a fresh client to continue observing the live workflow run');

    const completedRun = await poll(
      'workflow run success after reconnect',
      () => fetchWorkflowRun(reconnectAccessToken, options, runId),
      (candidate) => candidate?.state === 'succeeded',
      options.connectTimeoutMs,
    );
    if (completedRun.output?.session_id !== createdSessionId) {
      throw new Error('Completed workflow run output carried the wrong session id.');
    }
    if (completedRun.output?.workflow_run_id !== runId) {
      throw new Error('Completed workflow run output carried the wrong run id.');
    }

    const logs = await fetchWorkflowRunLogs(reconnectAccessToken, options, runId);
    if (
      !logs.logs.some(
        (entry) =>
          entry.source === 'automation_task' &&
          entry.message.includes(`workflow existing-session start ${createdSessionId}`),
      )
    ) {
      throw new Error('Workflow reconnect smoke is missing the existing-session start log.');
    }
    if (
      !logs.logs.some(
        (entry) =>
          entry.source === 'automation_task' &&
          entry.message.includes('workflow existing-session finish BrowserPane Test Embed'),
      )
    ) {
      throw new Error('Workflow reconnect smoke is missing the completion log.');
    }

    const summary = {
      workflowId: workflow.id,
      workflowVersion: version.version,
      workflowSourceCommit: version.source?.resolved_commit ?? null,
      runId,
      state: completedRun.state,
      sessionId: createdSessionId,
      automationTaskId: completedRun.automation_task_id,
      reconnected: true,
      logs: logs.logs.length,
      outputTitle: completedRun.output?.title ?? null,
      outputFinalUrl: completedRun.output?.final_url ?? null,
    };

    if (options.outputPath) {
      await fs.writeFile(options.outputPath, `${JSON.stringify(summary, null, 2)}\n`, 'utf8');
      log(`Wrote summary to ${options.outputPath}`);
    }

    console.log(JSON.stringify(summary, null, 2));
  } finally {
    if (createdSessionId && accessToken) {
      try {
        await deleteSession(accessToken, options, createdSessionId);
      } catch (error) {
        log(
          `cleanup warning: failed to delete session ${createdSessionId}: ${error instanceof Error ? error.message : String(error)}`,
        );
      }
    }
    if (localWorkflowSource?.repoDir) {
      await fs.rm(localWorkflowSource.repoDir, { recursive: true, force: true }).catch(() => {});
    }
    if (ownerContext) {
      await ownerContext.close().catch(() => {});
    }
    if (reconnectContext) {
      await reconnectContext.close().catch(() => {});
    }
    await browser.close().catch(() => {});
  }
}

main().catch((error) => {
  console.error(
    `[workflow-reconnect-smoke] ${error instanceof Error ? error.stack ?? error.message : String(error)}`,
  );
  process.exitCode = 1;
});
