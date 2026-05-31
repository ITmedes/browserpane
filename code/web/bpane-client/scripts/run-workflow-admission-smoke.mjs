import { chromium } from 'playwright-core';

import {
  buildWorkflowWorkerImage,
  cleanupWorkflowSmokeSessions,
  configurePage,
  createLocalWorkflowRepo,
  createLogger,
  ensureLoggedIn,
  apiOrigin,
  fetchJson,
  getAccessToken,
  launchChrome,
  parseSmokeArgs,
  poll,
  recreateComposeServices,
  waitForWorkflowControlPlane,
} from './workflow-smoke-lib.mjs';

const log = createLogger('workflow-admission-smoke');

async function configureGatewayForWorkerAdmissionSmoke(accessToken, options) {
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

async function configureGatewayForProjectAdmissionSmoke(accessToken, options) {
  log('Recreating gateway in docker_pool mode with workflow worker backpressure disabled');
  recreateComposeServices(['gateway'], {
    envOverrides: {
      BPANE_GATEWAY_RUNTIME_BACKEND: 'docker_pool',
      BPANE_GATEWAY_MAX_ACTIVE_RUNTIMES: '4',
      BPANE_WORKFLOW_WORKER_MAX_ACTIVE: '0',
    },
  });
  await waitForWorkflowControlPlane(accessToken, options);
}

function workflowAdmissionEntrypoint() {
  return `export default async function run({ page, input, sessionId }) {
  const holdMs = Number(input && typeof input.hold_ms === 'number' ? input.hold_ms : 0);
  await page.goto('http://web:8080/test-embed.html', { waitUntil: 'networkidle' });
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
  return await fetchJson(`${apiOrigin(options)}/api/v1/workflows`, {
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
  return await fetchJson(`${apiOrigin(options)}/api/v1/workflows/${workflowId}/versions`, {
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

async function createProject(accessToken, options) {
  return await fetchJson(`${apiOrigin(options)}/api/v1/projects`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      name: `workflow-admission-smoke-${Date.now()}`,
      description: 'Validate project-scoped workflow admission and queueing',
      labels: {
        suite: 'workflow-admission-smoke',
      },
      quotas: {
        max_active_sessions: 4,
        max_active_workflow_runs: 1,
        max_retained_storage_bytes: 1073741824,
      },
    }),
  });
}

async function createWorkflowRun(
  accessToken,
  options,
  workflowId,
  input,
  clientRequestId,
  { sessionProjectId = null, projectId = null } = {},
) {
  const body = {
    workflow_id: workflowId,
    version: 'v1',
    session: {
      create_session: sessionProjectId ? { project_id: sessionProjectId } : {},
    },
    client_request_id: clientRequestId,
    input,
    labels: {
      suite: 'workflow-admission-smoke',
    },
  };
  if (projectId) {
    body.project_id = projectId;
  }

  return await fetchJson(`${apiOrigin(options)}/api/v1/workflow-runs`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(body),
  });
}

async function getWorkflowRun(accessToken, options, runId) {
  return await fetchJson(`${apiOrigin(options)}/api/v1/workflow-runs/${runId}`, {
    headers: {
      Authorization: `Bearer ${accessToken}`,
    },
  });
}

async function getWorkflowRunEvents(accessToken, options, runId) {
  return await fetchJson(`${apiOrigin(options)}/api/v1/workflow-runs/${runId}/events`, {
    headers: {
      Authorization: `Bearer ${accessToken}`,
    },
  });
}

async function waitForQueuedWorkflowRun(accessToken, options, runId, description) {
  const run = await poll(
    description,
    async () => await getWorkflowRun(accessToken, options, runId),
    (candidate) => {
      const state = String(candidate?.state ?? '');
      return ['queued', 'running', 'succeeded', 'failed', 'cancelled', 'timed_out'].includes(state);
    },
    30000,
    500,
  );
  if (run.state !== 'queued') {
    throw new Error(`Expected ${description} to queue, got ${run.state ?? 'unknown'}.`);
  }
  return run;
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
  let projectRun = null;
  let queuedProjectRun = null;
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
    await configureGatewayForWorkerAdmissionSmoke(accessToken, options);

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
      { hold_ms: 5000 },
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
    secondRun = await waitForQueuedWorkflowRun(
      accessToken,
      options,
      secondRun.id,
      'second workflow run',
    );
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

    await configureGatewayForProjectAdmissionSmoke(accessToken, options);
    await cleanupWorkflowSmokeSessions(accessToken, options, log);

    log('Creating project-scoped workflow runs that should queue on project quota');
    const project = await createProject(accessToken, options);
    projectRun = await createWorkflowRun(
      accessToken,
      options,
      workflow.id,
      { hold_ms: 5000 },
      `${requestIdPrefix}-project-run-1`,
      { sessionProjectId: project.id },
    );
    if (projectRun.project_id !== project.id) {
      throw new Error(
        `Expected first project run to inherit project ${project.id}, got ${projectRun.project_id ?? 'null'}.`,
      );
    }
    if (projectRun.project?.id !== project.id) {
      throw new Error('First project run did not expose the project summary.');
    }
    if (projectRun.project_admission?.state !== 'allowed') {
      throw new Error(
        `Expected first project run admission to be allowed, got ${projectRun.project_admission?.state ?? 'unknown'}.`,
      );
    }

    queuedProjectRun = await createWorkflowRun(
      accessToken,
      options,
      workflow.id,
      { hold_ms: 0 },
      `${requestIdPrefix}-project-run-2`,
      { sessionProjectId: project.id },
    );
    queuedProjectRun = await waitForQueuedWorkflowRun(
      accessToken,
      options,
      queuedProjectRun.id,
      'project-quota workflow run',
    );
    if (queuedProjectRun.project_id !== project.id) {
      throw new Error('Queued project run did not inherit the bound session project.');
    }
    if (queuedProjectRun.project_admission?.reason_code !== 'active_workflow_run_quota_exceeded') {
      throw new Error(
        `Expected project quota reason, got ${queuedProjectRun.project_admission?.reason_code ?? 'unknown'}.`,
      );
    }
    if (queuedProjectRun.admission?.reason !== 'project_active_workflow_quota_exhausted') {
      throw new Error(
        `Expected workflow admission reason project_active_workflow_quota_exhausted, got ${queuedProjectRun.admission?.reason ?? 'unknown'}.`,
      );
    }

    const completedProjectRun = await poll(
      'first project workflow run completion',
      async () => await getWorkflowRun(accessToken, options, projectRun.id),
      (run) => ['succeeded', 'failed', 'cancelled', 'timed_out'].includes(String(run?.state ?? '')),
      30000,
      500,
    );
    if (completedProjectRun.state !== 'succeeded') {
      throw new Error(`Expected first project run to succeed, got ${completedProjectRun.state}.`);
    }

    const completedQueuedProjectRun = await poll(
      'project queued workflow run completion',
      async () => await getWorkflowRun(accessToken, options, queuedProjectRun.id),
      (run) => ['succeeded', 'failed', 'cancelled', 'timed_out'].includes(String(run?.state ?? '')),
      30000,
      500,
    );
    if (completedQueuedProjectRun.state !== 'succeeded') {
      throw new Error(
        `Expected queued project run to succeed, got ${completedQueuedProjectRun.state}.`,
      );
    }

    const summary = {
      workflowId: workflow.id,
      firstRunId: firstRun.id,
      secondRunId: secondRun.id,
      projectRunId: projectRun.id,
      queuedProjectRunId: queuedProjectRun.id,
      secondRunAdmissionReason: secondRun.admission?.reason ?? null,
      secondRunQueuedAt: secondRun.admission?.queued_at ?? null,
      queuedProjectRunAdmissionReason: queuedProjectRun.admission?.reason ?? null,
      queuedProjectRunProjectAdmissionReason: queuedProjectRun.project_admission?.reason_code ?? null,
      firstRunState: completedFirstRun.state,
      secondRunState: completedSecondRun.state,
      projectRunState: completedProjectRun.state,
      queuedProjectRunState: completedQueuedProjectRun.state,
      eventTypes,
    };
    console.log(JSON.stringify(summary, null, 2));
  } finally {
    if (accessToken) {
      await cleanupWorkflowSmokeSessions(accessToken, options, log).catch((error) => {
        log(`Cleanup skipped after workflow admission smoke: ${error}`);
      });
    }
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
