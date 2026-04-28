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

const log = createLogger('workflow-admission-smoke');

function workflowAdmissionEntrypoint() {
  return `export default async function run({ page, input, sessionId }) {
  const holdMs = Number(input && typeof input.hold_ms === 'number' ? input.hold_ms : 0);
  await page.goto('http://web:8080', { waitUntil: 'networkidle' });
  if (holdMs > 0) {
    await new Promise((resolve) => setTimeout(resolve, holdMs));
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

async function createWorkflow(accessToken, options) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflows`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      name: 'workflow-admission-smoke',
      description: 'Validate queued workflow admission when worker capacity is exhausted',
      labels: {
        suite: 'workflow-admission-smoke',
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
      entrypoint: 'workflows/admission/run.mjs',
      source: {
        kind: 'git',
        repository_url: source.repositoryUrl,
        ref: 'refs/heads/main',
        root_path: 'workflows',
      },
      default_session: {
        labels: {
          origin: 'workflow-admission-smoke',
        },
      },
    }),
  });
}

async function createWorkflowRun(accessToken, options, workflowId, input, clientRequestId) {
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
      input,
      labels: {
        suite: 'workflow-admission-smoke',
      },
    }),
  });
}

async function getWorkflowRun(accessToken, options, runId) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow-runs/${runId}`, {
    headers: {
      Authorization: `Bearer ${accessToken}`,
    },
  });
}

async function getWorkflowRunEvents(accessToken, options, runId) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow-runs/${runId}/events`, {
    headers: {
      Authorization: `Bearer ${accessToken}`,
    },
  });
}

async function main() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-workflow-admission-smoke.mjs');
  const browser = await launchChrome(chromium, options);

  let context = null;
  let page = null;
  let accessToken = '';
  let localWorkflowSource = null;
  let firstRun = null;
  let secondRun = null;
  const requestIdPrefix = `workflow-admission-smoke-${Date.now().toString(36)}`;

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
      throw new Error('Workflow admission smoke failed to acquire an access token.');
    }

    await cleanupWorkflowSmokeSessions(accessToken, options, log);

    log('Preparing local workflow source');
    localWorkflowSource = await createLocalWorkflowRepo('.workflow-admission-smoke-repo-', {
      'workflows/admission/run.mjs': workflowAdmissionEntrypoint(),
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
      throw new Error('Workflow admission smoke did not pin the expected git commit.');
    }

    log('Creating first workflow run that will hold the only worker slot');
    firstRun = await createWorkflowRun(
      accessToken,
      options,
      workflow.id,
      { hold_ms: 1500 },
      `${requestIdPrefix}-run-1`,
    );

    log('Creating second workflow run that should be queued');
    secondRun = await createWorkflowRun(
      accessToken,
      options,
      workflow.id,
      { hold_ms: 0 },
      `${requestIdPrefix}-run-2`,
    );
    if (secondRun.state !== 'queued') {
      throw new Error(`Expected queued second run, got ${secondRun.state ?? 'unknown'}.`);
    }
    if (secondRun.admission?.reason !== 'workflow_worker_capacity') {
      throw new Error('Queued workflow run did not expose the expected admission reason.');
    }

    const completedFirstRun = await poll(
      'first workflow run completion',
      async () => await getWorkflowRun(accessToken, options, firstRun.id),
      (run) => ['succeeded', 'failed', 'cancelled', 'timed_out'].includes(String(run?.state ?? '')),
      30000,
      500,
    );
    if (completedFirstRun.state !== 'succeeded') {
      throw new Error(`Expected first run to succeed, got ${completedFirstRun.state}.`);
    }

    const completedSecondRun = await poll(
      'queued workflow run completion',
      async () => await getWorkflowRun(accessToken, options, secondRun.id),
      (run) => ['succeeded', 'failed', 'cancelled', 'timed_out'].includes(String(run?.state ?? '')),
      30000,
      500,
    );
    if (completedSecondRun.state !== 'succeeded') {
      throw new Error(`Expected queued run to succeed, got ${completedSecondRun.state}.`);
    }
    if (completedSecondRun.admission !== null) {
      throw new Error('Expected queued run admission block to clear after execution.');
    }

    const events = await getWorkflowRunEvents(accessToken, options, secondRun.id);
    const eventTypes = (Array.isArray(events.events) ? events.events : []).map((event) =>
      String(event?.event_type ?? ''),
    );
    for (const expected of ['workflow_run.queued', 'workflow_run.succeeded']) {
      if (!eventTypes.includes(expected)) {
        throw new Error(`Queued run is missing expected event ${expected}.`);
      }
    }

    const summary = {
      workflowId: workflow.id,
      firstRunId: firstRun.id,
      secondRunId: secondRun.id,
      secondRunAdmissionReason: secondRun.admission?.reason ?? null,
      secondRunQueuedAt: secondRun.admission?.queued_at ?? null,
      firstRunState: completedFirstRun.state,
      secondRunState: completedSecondRun.state,
      eventTypes,
    };
    console.log(JSON.stringify(summary, null, 2));
  } finally {
    if (context) {
      await context.close().catch(() => {});
    }
    await browser.close().catch(() => {});
    if (localWorkflowSource) {
      await localWorkflowSource.cleanup().catch(() => {});
    }
  }
}

main().catch((error) => {
  console.error(error instanceof Error ? error.stack ?? error.message : String(error));
  process.exitCode = 1;
});
