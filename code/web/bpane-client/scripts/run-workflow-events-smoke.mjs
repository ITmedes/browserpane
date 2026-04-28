import { createHmac } from 'node:crypto';
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
  startWorkflowWebhookReceiver,
} from './workflow-smoke-lib.mjs';

const log = createLogger('workflow-events-smoke');

function signPayload(secret, timestamp, body) {
  return `v1=${createHmac('sha256', secret).update(`${timestamp}.`).update(body).digest('hex')}`;
}

async function createWorkflow(accessToken, options) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflows`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      name: 'workflow-events-smoke',
      description: 'Validate outbound workflow event delivery',
      labels: {
        suite: 'workflow-events-smoke',
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
      entrypoint: 'workflows/events/run.mjs',
    }),
  });
}

async function createSubscription(accessToken, options, targetUrl) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow-event-subscriptions`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      name: 'workflow-events-smoke',
      target_url: targetUrl,
      event_types: ['workflow_run.created', 'workflow_run.running', 'workflow_run.succeeded'],
      signing_secret: 'workflow-events-secret',
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
      source_system: 'workflow-engine',
      source_reference: 'job-42',
      labels: {
        suite: 'workflow-events-smoke',
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

async function fetchDeliveries(accessToken, options, subscriptionId) {
  return await fetchJson(
    `${options.pageUrl}/api/v1/workflow-event-subscriptions/${subscriptionId}/deliveries`,
    {
      headers: { Authorization: `Bearer ${accessToken}` },
    },
  );
}

async function fetchWorkflowOperations(accessToken, options) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow/operations`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function main() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-workflow-events-smoke.mjs');
  const browser = await launchChrome(chromium, options);

  let context = null;
  let page = null;
  let accessToken = '';
  let webhookReceiver = null;

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

    log('Starting webhook receiver container');
    webhookReceiver = await startWorkflowWebhookReceiver({
      statuses: [200, 200, 200],
    });

    const workflow = await createWorkflow(accessToken, options);
    const version = await createWorkflowVersion(accessToken, options, workflow.id);
    if (version.executor !== 'manual') {
      throw new Error(`Expected manual executor, got ${version.executor}`);
    }

    const subscription = await createSubscription(
      accessToken,
      options,
      webhookReceiver.targetUrl,
    );
    if (!subscription.id) {
      throw new Error('Workflow event subscription creation did not return an id.');
    }

    const createdRun = await createWorkflowRun(accessToken, options, workflow.id);
    const runId = createdRun.id ?? '';
    const sessionId = createdRun.session_id ?? '';
    if (!runId || !sessionId) {
      throw new Error('Workflow run creation did not return run and session ids.');
    }

    const automationAccess = await issueAutomationAccess(accessToken, options, sessionId);
    const automationToken = automationAccess.token ?? '';
    if (!automationToken) {
      throw new Error('Automation access issuance did not return a token.');
    }

    await transitionRun(automationToken, options, runId, {
      state: 'running',
      message: 'manual executor started',
    });
    await transitionRun(automationToken, options, runId, {
      state: 'succeeded',
      message: 'manual executor finished',
      output: {
        ok: true,
      },
    });

    const capturedRequests = await poll(
      'captured workflow webhook deliveries',
      async () => await webhookReceiver.readRequests(),
      (requests) => requests.length >= 3,
      options.connectTimeoutMs,
      250,
    );

    const eventTypes = capturedRequests.map((request) => request.body?.event_type ?? '');
    const expectedEventTypes = [
      'workflow_run.created',
      'workflow_run.running',
      'workflow_run.succeeded',
    ];
    if (JSON.stringify(eventTypes) !== JSON.stringify(expectedEventTypes)) {
      throw new Error(
        `Unexpected workflow event delivery order: ${JSON.stringify(eventTypes)}`,
      );
    }

    for (const request of capturedRequests) {
      const timestamp = request.headers['x-bpane-signature-timestamp'] ?? '';
      const signature = request.headers['x-bpane-signature-v1'] ?? '';
      const expected = signPayload(
        'workflow-events-secret',
        timestamp,
        Buffer.from(JSON.stringify(request.body), 'utf8'),
      );
      if (signature !== expected) {
        throw new Error(`Workflow event delivery signature mismatch for ${request.body.event_type}`);
      }
    }

    const deliveries = await fetchDeliveries(accessToken, options, subscription.id);
    if (!Array.isArray(deliveries.deliveries) || deliveries.deliveries.length !== 3) {
      throw new Error('Workflow event delivery diagnostics did not return all delivered events.');
    }
    const deliveryStates = deliveries.deliveries.map((delivery) => delivery.state);
    if (!deliveryStates.every((state) => state === 'delivered')) {
      throw new Error(`Unexpected workflow event delivery states: ${deliveryStates.join(', ')}`);
    }

    const operations = await fetchWorkflowOperations(accessToken, options);
    if (operations.event_delivery_attempts_total < 3) {
      throw new Error('Workflow operations did not record delivery attempts.');
    }
    if (operations.event_delivery_successes_total < 3) {
      throw new Error('Workflow operations did not record delivery successes.');
    }

    const summary = {
      workflowId: workflow.id,
      workflowVersionId: version.id,
      runId,
      sessionId,
      subscriptionId: subscription.id,
      eventTypes,
      deliveryCount: deliveries.deliveries.length,
      attemptsTotal: operations.event_delivery_attempts_total,
      successesTotal: operations.event_delivery_successes_total,
    };

    if (options.outputPath) {
      await fs.writeFile(options.outputPath, `${JSON.stringify(summary, null, 2)}\n`, 'utf8');
    }
    log(`Smoke complete: ${JSON.stringify(summary)}`);
  } finally {
    await page?.close().catch(() => {});
    await context?.close().catch(() => {});
    await browser.close().catch(() => {});
    await webhookReceiver?.cleanup?.().catch(() => {});
  }
}

main().catch((error) => {
  console.error(`[workflow-events-smoke] ${error.message}`);
  process.exitCode = 1;
});
