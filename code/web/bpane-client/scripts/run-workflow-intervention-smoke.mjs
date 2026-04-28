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
} from './workflow-smoke-lib.mjs';

const log = createLogger('workflow-intervention-smoke');

async function createWorkflow(accessToken, options) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflows`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      name: 'workflow-intervention-smoke',
      description: 'Validate durable owner intervention on workflow runs',
      labels: {
        suite: 'workflow-intervention-smoke',
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
      session: {
        create_session: {},
      },
      labels: {
        suite: 'workflow-intervention-smoke',
      },
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

async function submitInput(accessToken, options, runId, body) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow-runs/${runId}/submit-input`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(body),
  });
}

async function resumeRun(accessToken, options, runId, body) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow-runs/${runId}/resume`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(body),
  });
}

async function rejectRun(accessToken, options, runId, body) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow-runs/${runId}/reject`, {
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

async function main() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-workflow-intervention-smoke.mjs');
  const browser = await launchChrome(chromium, options);

  let context = null;
  let page = null;
  let accessToken = '';
  let createdSessionId = '';

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
      throw new Error('Workflow intervention smoke version did not persist the expected executor.');
    }

    log('Creating workflow run for intervention validation');
    const createdRun = await createWorkflowRun(accessToken, options, workflow.id);
    const runId = createdRun.id ?? '';
    createdSessionId = createdRun.session_id ?? '';
    if (!runId || !createdSessionId) {
      throw new Error('Workflow run creation did not return run and session ids.');
    }

    const automationAccess = await issueAutomationAccess(accessToken, options, createdSessionId);
    const automationToken = automationAccess.token ?? '';
    if (!automationToken) {
      throw new Error('Automation access issuance did not return a token.');
    }

    await transitionRun(automationToken, options, runId, {
      state: 'running',
      message: 'executor attached',
    });

    const firstRequestId = crypto.randomUUID();
    log('Submitting first awaiting_input request');
    const awaitingInput = await transitionRun(automationToken, options, runId, {
      state: 'awaiting_input',
      message: 'approval required',
      data: {
        intervention_request: {
          request_id: firstRequestId,
          kind: 'approval',
          prompt: 'Approve payout export',
          details: {
            phase: 'review',
          },
        },
      },
    });
    if (awaitingInput.state !== 'awaiting_input') {
      throw new Error(`Expected awaiting_input, got ${awaitingInput.state}`);
    }
    if (awaitingInput.intervention?.pending_request?.request_id !== firstRequestId) {
      throw new Error('Pending intervention request was not exposed on the workflow run.');
    }

    const submitted = await submitInput(accessToken, options, runId, {
      input: {
        approved: true,
      },
      comment: 'operator approved',
    });
    if (submitted.state !== 'running') {
      throw new Error(`Expected running after submit-input, got ${submitted.state}`);
    }
    if (submitted.intervention?.last_resolution?.action !== 'submit_input') {
      throw new Error('Submit-input resolution was not persisted on the workflow run.');
    }

    const secondRequestId = crypto.randomUUID();
    log('Submitting second awaiting_input request');
    await transitionRun(automationToken, options, runId, {
      state: 'awaiting_input',
      message: 'resume required',
      data: {
        intervention_request: {
          request_id: secondRequestId,
          kind: 'confirmation',
          prompt: 'Resume the run',
        },
      },
    });

    const resumed = await resumeRun(accessToken, options, runId, {
      comment: 'operator resumed',
    });
    if (resumed.state !== 'running') {
      throw new Error(`Expected running after resume, got ${resumed.state}`);
    }
    if (resumed.intervention?.last_resolution?.action !== 'resume') {
      throw new Error('Resume resolution was not persisted on the workflow run.');
    }

    const thirdRequestId = crypto.randomUUID();
    log('Submitting third awaiting_input request');
    await transitionRun(automationToken, options, runId, {
      state: 'awaiting_input',
      message: 'approval required again',
      data: {
        intervention_request: {
          request_id: thirdRequestId,
          kind: 'approval',
          prompt: 'Reject this run',
        },
      },
    });

    const rejected = await rejectRun(accessToken, options, runId, {
      reason: 'operator denied approval',
    });
    if (rejected.state !== 'failed') {
      throw new Error(`Expected failed after reject, got ${rejected.state}`);
    }
    if (rejected.error !== 'operator denied approval') {
      throw new Error('Rejected workflow run did not persist the rejection reason.');
    }
    if (rejected.intervention?.last_resolution?.action !== 'reject') {
      throw new Error('Reject resolution was not persisted on the workflow run.');
    }

    const finalRun = await fetchWorkflowRun(accessToken, options, runId);
    const events = await fetchWorkflowRunEvents(accessToken, options, runId);
    const eventTypes = Array.isArray(events.events)
      ? events.events.map((event) => event.event_type)
      : [];
    for (const expected of [
      'workflow_run.input_submitted',
      'workflow_run.resumed',
      'workflow_run.rejected',
    ]) {
      if (!eventTypes.includes(expected)) {
        throw new Error(`Workflow intervention smoke is missing ${expected}.`);
      }
    }

    const summary = {
      workflowId: workflow.id,
      workflowVersionId: version.id,
      runId,
      sessionId: createdSessionId,
      finalState: finalRun.state,
      finalError: finalRun.error,
      lastResolutionAction: finalRun.intervention?.last_resolution?.action ?? null,
      events: eventTypes.length,
    };
    log(`Workflow intervention smoke passed for run ${runId}`);

    if (options.outputPath) {
      await fs.writeFile(options.outputPath, JSON.stringify(summary, null, 2));
    }
  } finally {
    if (createdSessionId && accessToken) {
      try {
        const response = await fetch(`${options.pageUrl}/api/v1/sessions/${createdSessionId}`, {
          method: 'DELETE',
          headers: { Authorization: `Bearer ${accessToken}` },
        });
        if (!response.ok && response.status !== 404) {
          const detail = await response.text().catch(() => '');
          log(
            `Failed to delete workflow intervention smoke session ${createdSessionId}: HTTP ${response.status}${detail ? ` ${detail}` : ''}`,
          );
        }
      } catch (error) {
        log(
          `Failed to delete workflow intervention smoke session ${createdSessionId}: ${error instanceof Error ? error.message : String(error)}`,
        );
      }
    }
    await context?.close().catch(() => {});
    await browser.close().catch(() => {});
  }
}

main().catch((error) => {
  console.error(
    `[workflow-intervention-smoke] ${error instanceof Error ? error.stack ?? error.message : String(error)}`,
  );
  process.exitCode = 1;
});
