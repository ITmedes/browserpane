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

const log = createLogger('workflow-runtime-hold-smoke');

async function createWorkflow(accessToken, options) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflows`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      name: 'workflow-runtime-hold-smoke',
      description: 'Validate paused workflow runtime hold and release semantics',
      labels: {
        suite: 'workflow-runtime-hold-smoke',
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
      executor: 'manual_test',
      entrypoint: 'workflows/runtime-hold/run.mjs',
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
        suite: 'workflow-runtime-hold-smoke',
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

async function fetchWorkflowRun(accessToken, options, runId) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow-runs/${runId}`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function fetchSession(accessToken, options, sessionId) {
  return await fetchJson(`${options.pageUrl}/api/v1/sessions/${sessionId}`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function main() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-workflow-runtime-hold-smoke.mjs');
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
    if (version.executor !== 'manual_test') {
      throw new Error('Workflow version did not persist the expected executor.');
    }

    log('Creating workflow run with a live runtime hold');
    const heldRun = await createWorkflowRun(accessToken, options, workflow.id);
    const heldRunId = heldRun.id ?? '';
    const heldSessionId = heldRun.session_id ?? '';
    if (!heldRunId || !heldSessionId) {
      throw new Error('Held workflow run creation did not return run and session ids.');
    }

    const heldAutomationAccess = await issueAutomationAccess(accessToken, options, heldSessionId);
    const heldAutomationToken = heldAutomationAccess.token ?? '';
    if (!heldAutomationToken) {
      throw new Error('Held workflow run automation access did not return a token.');
    }

    await transitionRun(heldAutomationToken, options, heldRunId, {
      state: 'running',
      message: 'executor attached',
    });
    await transitionRun(heldAutomationToken, options, heldRunId, {
      state: 'awaiting_input',
      message: 'approval required',
      data: {
        intervention_request: {
          kind: 'approval',
          prompt: 'Approve the paused run',
        },
        runtime_hold: {
          mode: 'live',
          timeout_sec: 1,
        },
      },
    });

    const liveRuntime = await poll(
      'live runtime hold visibility',
      () => fetchWorkflowRun(accessToken, options, heldRunId),
      (run) => run.runtime?.resume_mode === 'live_runtime',
      options.connectTimeoutMs,
      100,
    );
    if (liveRuntime.runtime?.exact_runtime_available !== true) {
      throw new Error('Held workflow run did not report an exact live runtime.');
    }
    if (!liveRuntime.runtime?.hold_until) {
      throw new Error('Held workflow run did not expose hold_until.');
    }

    const heldSession = await fetchSession(accessToken, options, heldSessionId);
    if (heldSession.state === 'stopped') {
      throw new Error('Held workflow session was released before the hold timeout expired.');
    }

    const releasedRuntime = await poll(
      'held runtime release',
      () => fetchWorkflowRun(accessToken, options, heldRunId),
      (run) => run.runtime?.released_at && run.runtime?.release_reason === 'hold_expired',
      options.connectTimeoutMs,
      100,
    );
    if (releasedRuntime.runtime?.resume_mode !== 'profile_restart') {
      throw new Error('Held workflow run did not switch to profile_restart after hold expiry.');
    }

    const releasedHeldSession = await fetchSession(accessToken, options, heldSessionId);
    if (releasedHeldSession.state !== 'stopped') {
      throw new Error(
        `Expected held workflow session to stop after release, got ${releasedHeldSession.state}`,
      );
    }

    log('Creating workflow run without a live runtime hold');
    const immediateRun = await createWorkflowRun(accessToken, options, workflow.id);
    const immediateRunId = immediateRun.id ?? '';
    const immediateSessionId = immediateRun.session_id ?? '';
    if (!immediateRunId || !immediateSessionId) {
      throw new Error('Immediate-release workflow run creation did not return ids.');
    }

    const immediateAutomationAccess = await issueAutomationAccess(
      accessToken,
      options,
      immediateSessionId,
    );
    const immediateAutomationToken = immediateAutomationAccess.token ?? '';
    if (!immediateAutomationToken) {
      throw new Error('Immediate-release workflow automation access did not return a token.');
    }

    await transitionRun(immediateAutomationToken, options, immediateRunId, {
      state: 'running',
      message: 'executor attached',
    });
    await transitionRun(immediateAutomationToken, options, immediateRunId, {
      state: 'awaiting_input',
      message: 'approval required',
      data: {
        intervention_request: {
          kind: 'approval',
          prompt: 'Approve the paused run',
        },
      },
    });

    const immediateRelease = await poll(
      'immediate runtime release',
      () => fetchWorkflowRun(accessToken, options, immediateRunId),
      (run) =>
        run.runtime?.released_at &&
        run.runtime?.release_reason === 'awaiting_input_no_live_hold',
      options.connectTimeoutMs,
      100,
    );
    if (immediateRelease.runtime?.resume_mode !== 'profile_restart') {
      throw new Error('Immediate awaiting-input workflow run did not expose profile_restart.');
    }

    const releasedImmediateSession = await fetchSession(accessToken, options, immediateSessionId);
    if (releasedImmediateSession.state !== 'stopped') {
      throw new Error(
        `Expected immediate-release workflow session to stop, got ${releasedImmediateSession.state}`,
      );
    }

    const summary = {
      workflowId: workflow.id,
      heldRunId,
      heldSessionId,
      heldReleaseReason: releasedRuntime.runtime.release_reason,
      immediateRunId,
      immediateSessionId,
      immediateReleaseReason: immediateRelease.runtime.release_reason,
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
  console.error(`[workflow-runtime-hold-smoke] ${error?.stack ?? error}`);
  process.exitCode = 1;
});
