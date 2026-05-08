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

const log = createLogger('workflow-cancel-smoke');

function workflowCancelEntrypoint() {
  return `export default async function run({ page, sessionId }) {
  console.log(\`workflow cancel start \${sessionId}\`);
  await page.goto('http://web:8080/test-embed.html', { waitUntil: 'networkidle' });
  console.error('workflow awaiting cancellation');
  await page.waitForTimeout(60000);
  return {
    title: await page.title(),
    final_url: page.url(),
    session_id: sessionId,
  };
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
      name: 'workflow-cancel-smoke',
      description: 'Validate workflow run cancellation semantics',
      labels: {
        suite: 'workflow-cancel-smoke',
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
          origin: 'workflow-cancel-smoke',
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
      labels: {
        suite: 'workflow-cancel-smoke',
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

async function cancelWorkflowRun(accessToken, options, runId) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow-runs/${runId}/cancel`, {
    method: 'POST',
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
  const options = parseSmokeArgs(process.argv.slice(2), 'run-workflow-cancel-smoke.mjs');
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
    localWorkflowSource = await createLocalWorkflowRepo('.workflow-cancel-smoke-repo-', {
      'workflows/cancel/run.mjs': workflowCancelEntrypoint(),
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

    log('Creating long-running workflow run');
    const createdRun = await createWorkflowRun(accessToken, options, workflow.id);
    const runId = createdRun.id ?? '';
    createdSessionId = createdRun.session_id ?? '';
    if (!runId || !createdSessionId) {
      throw new Error('Workflow run creation did not return run and session ids.');
    }

    await poll(
      'workflow run running state',
      () => fetchWorkflowRun(accessToken, options, runId),
      (candidate) => candidate?.state === 'running',
      options.connectTimeoutMs,
    );

    log(`Cancelling workflow run ${runId}`);
    await cancelWorkflowRun(accessToken, options, runId);

    const cancelledRun = await poll(
      'workflow run cancelled state',
      () => fetchWorkflowRun(accessToken, options, runId),
      (candidate) => candidate?.state === 'cancelled',
      options.connectTimeoutMs,
    );
    if (cancelledRun.error !== null) {
      throw new Error('Cancelled workflow run should not expose an error payload.');
    }
    if (!cancelledRun.completed_at) {
      throw new Error('Cancelled workflow run did not record a completion timestamp.');
    }

    const events = await fetchWorkflowRunEvents(accessToken, options, runId);
    const eventTypes = events.events.map((event) => event.event_type);
    for (const expected of [
      'workflow_run.cancel_requested',
      'workflow_run.cancelled',
      'automation_task.cancelled',
    ]) {
      if (!eventTypes.includes(expected)) {
        throw new Error(`Workflow cancel smoke is missing ${expected}.`);
      }
    }

    const logs = await fetchWorkflowRunLogs(accessToken, options, runId);
    if (
      !logs.logs.some(
        (entry) =>
          entry.source === 'run' &&
          entry.message.includes('workflow run cancelled'),
      )
    ) {
      throw new Error('Workflow cancel smoke is missing the run cancellation log.');
    }
    if (
      !logs.logs.some(
        (entry) =>
          entry.message.includes('cancelled'),
      )
    ) {
      throw new Error('Workflow cancel smoke is missing cancellation evidence in logs.');
    }

    const summary = {
      workflowId: workflow.id,
      workflowVersion: version.version,
      workflowSourceCommit: version.source?.resolved_commit ?? null,
      runId,
      state: cancelledRun.state,
      sessionId: createdSessionId,
      automationTaskId: cancelledRun.automation_task_id,
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
    `[workflow-cancel-smoke] ${error instanceof Error ? error.stack ?? error.message : String(error)}`,
  );
  process.exitCode = 1;
});
