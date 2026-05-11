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

const log = createLogger('workflow-failure-smoke');

function workflowFailureEntrypoint() {
  return `export default async function run({ page, input, sessionId }) {
  const targetUrl =
    input && typeof input.target_url === 'string' && input.target_url.trim()
      ? input.target_url.trim()
      : 'http://web:8080/test-embed.html';
  console.log(\`workflow failure start \${sessionId}\`);
  await page.goto(targetUrl, { waitUntil: 'networkidle' });
  await page.waitForTimeout(2000);
  console.error('workflow failure about to throw');
  throw new Error('workflow failure smoke expected error');
}
`;
}

async function createWorkflow(accessToken, options) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflows`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      name: 'workflow-failure-smoke',
      description: 'Validate failed workflow runs retain logs and linked recordings',
      labels: {
        suite: 'workflow-failure-smoke',
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
      entrypoint: 'workflows/failure/run.mjs',
      source: {
        kind: 'git',
        repository_url: source.repositoryUrl,
        ref: 'refs/heads/main',
        root_path: 'workflows',
      },
      input_schema: {
        type: 'object',
        required: ['target_url'],
        properties: {
          target_url: { type: 'string' },
        },
      },
      default_session: {
        labels: {
          origin: 'workflow-failure-smoke',
        },
        recording: {
          mode: 'manual',
          format: 'webm',
        },
      },
    }),
  });
}

async function createWorkflowRun(accessToken, options, workflowId) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow-runs`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      workflow_id: workflowId,
      version: 'v1',
      input: {
        target_url: 'http://web:8080/test-embed.html',
      },
      labels: {
        suite: 'workflow-failure-smoke',
      },
    }),
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

async function fetchWorkflowRunLogs(accessToken, options, runId) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow-runs/${runId}/logs`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function createSessionRecording(accessToken, options, sessionId) {
  return await fetchJson(`${options.pageUrl}/api/v1/sessions/${sessionId}/recordings`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function stopSessionRecording(accessToken, options, sessionId, recordingId) {
  return await fetchJson(
    `${options.pageUrl}/api/v1/sessions/${sessionId}/recordings/${recordingId}/stop`,
    {
      method: 'POST',
      headers: { Authorization: `Bearer ${accessToken}` },
    },
  );
}

async function fetchSessionRecording(accessToken, options, sessionId, recordingId) {
  return await fetchJson(
    `${options.pageUrl}/api/v1/sessions/${sessionId}/recordings/${recordingId}`,
    {
      headers: { Authorization: `Bearer ${accessToken}` },
    },
  );
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
  const options = parseSmokeArgs(process.argv.slice(2), 'run-workflow-failure-smoke.mjs');
  const browser = await launchChrome(chromium, options);

  let context = null;
  let page = null;
  let accessToken = '';
  let createdSessionId = '';
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
      throw new Error('Failed to acquire an access token from the test page.');
    }
    await cleanupWorkflowSmokeSessions(accessToken, options, log);

    log('Preparing local git-backed workflow source');
    localWorkflowSource = await createLocalWorkflowRepo('.workflow-failure-smoke-repo-', {
      'workflows/failure/run.mjs': workflowFailureEntrypoint(),
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
      throw new Error('Workflow version did not pin the expected local git commit.');
    }

    log('Creating workflow run that is expected to fail');
    const createdRun = await createWorkflowRun(accessToken, options, workflow.id);
    const runId = createdRun.id ?? '';
    createdSessionId = createdRun.session_id ?? '';
    if (!runId || !createdSessionId) {
      throw new Error('Workflow run creation did not return run and session ids.');
    }

    log(`Starting manual session recording for failure session ${createdSessionId}`);
    const recording = await createSessionRecording(accessToken, options, createdSessionId);
    if (!recording.id) {
      throw new Error('Workflow failure smoke recording creation did not return an id.');
    }

    const failedRun = await poll(
      'workflow run failed state',
      () => fetchWorkflowRun(accessToken, options, runId),
      (candidate) => candidate?.state === 'failed',
      options.connectTimeoutMs,
    );
    if (!String(failedRun.error ?? '').includes('workflow failure smoke expected error')) {
      throw new Error('Failed workflow run did not persist the expected error message.');
    }

    log(`Stopping workflow failure recording ${recording.id}`);
    const stoppedRecording = await stopSessionRecording(
      accessToken,
      options,
      createdSessionId,
      recording.id,
    );
    if (!['finalizing', 'ready', 'failed'].includes(String(stoppedRecording.state ?? ''))) {
      throw new Error('Workflow failure recording did not enter a terminal control-plane state.');
    }

    const events = await fetchWorkflowRunEvents(accessToken, options, runId);
    const eventTypes = events.events.map((event) => event.event_type);
    for (const expected of ['workflow_run.failed', 'automation_task.failed']) {
      if (!eventTypes.includes(expected)) {
        throw new Error(`Workflow failure smoke is missing ${expected}.`);
      }
    }

    const logs = await fetchWorkflowRunLogs(accessToken, options, runId);
    if (
      !logs.logs.some(
        (entry) =>
          entry.source === 'automation_task' &&
          entry.message.includes(`workflow failure start ${createdSessionId}`),
      )
    ) {
      throw new Error('Workflow failure smoke is missing the start log.');
    }
    if (
      !logs.logs.some(
        (entry) =>
          entry.source === 'automation_task' &&
          entry.message.includes('workflow failure about to throw'),
      )
    ) {
      throw new Error('Workflow failure smoke is missing the stderr log.');
    }
    if (
      !logs.logs.some(
        (entry) =>
          entry.source === 'run' &&
          entry.message.includes('workflow worker failed:') &&
          entry.message.includes('workflow failure smoke expected error'),
      )
    ) {
      throw new Error('Workflow failure smoke is missing the run-level failure log.');
    }

    const completedRun = await fetchWorkflowRun(accessToken, options, runId);
    if (!completedRun.retention?.logs_expire_at || !completedRun.retention?.output_expire_at) {
      throw new Error('Failed workflow run did not expose retention metadata.');
    }
    if (!Array.isArray(completedRun.recordings) || !completedRun.recordings.length) {
      throw new Error('Failed workflow run did not expose linked recordings.');
    }
    if (!completedRun.recordings.some((entry) => entry.id === recording.id)) {
      throw new Error('Failed workflow run did not retain the expected linked recording.');
    }
    const linkedRecording = await fetchSessionRecording(
      accessToken,
      options,
      createdSessionId,
      recording.id,
    );
    if (!completedRun.recordings.some((entry) => entry.state === linkedRecording.state)) {
      throw new Error('Failed workflow run did not reflect the linked recording state.');
    }

    const summary = {
      workflowId: workflow.id,
      workflowVersion: version.version,
      workflowSourceCommit: version.source?.resolved_commit ?? null,
      runId,
      state: completedRun.state,
      error: completedRun.error,
      sessionId: createdSessionId,
      automationTaskId: completedRun.automation_task_id,
      recordingId: recording.id,
      recordingState: linkedRecording.state,
      events: events.events.length,
      logs: logs.logs.length,
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
    if (context) {
      await context.close().catch(() => {});
    }
    await browser.close().catch(() => {});
  }
}

main().catch((error) => {
  console.error(
    `[workflow-failure-smoke] ${error instanceof Error ? error.stack ?? error.message : String(error)}`,
  );
  process.exitCode = 1;
});
