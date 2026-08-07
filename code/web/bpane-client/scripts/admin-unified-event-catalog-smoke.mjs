import assert from 'node:assert/strict';
import {
  adminRouteUrl,
  assertNoBodyHorizontalOverflow,
  authJsonHeaders,
} from './admin-unified-smoke-lib.mjs';
import { apiOrigin, deleteSession, fetchJson, poll } from './workflow-smoke-lib.mjs';

export async function exerciseEventCatalog(page, options, accessToken, runLabel, signingSecret) {
  const name = `Catalog events ${runLabel}`;
  await page.goto(adminRouteUrl(options, 'workflow-event-subscriptions/new'), {
    waitUntil: 'domcontentloaded',
  });
  await page.getByTestId('workflow-event-subscription-create-form').waitFor({ state: 'visible' });
  await page.getByTestId('workflow-event-subscription-name').fill(name);
  await page.getByTestId('workflow-event-subscription-target').fill('not-a-url');
  await page.getByTestId('workflow-event-subscription-target-error').waitFor({ state: 'visible' });
  await page
    .getByTestId('workflow-event-subscription-target')
    .fill('http://web:8080/test/workflow-events');
  await page.getByTestId('workflow-event-subscription-event-types').fill('workflow_run.*');
  await page.getByTestId('workflow-event-subscription-signing-secret').fill(signingSecret);
  await assertServerRejection(page, options);
  await page.getByTestId('workflow-event-subscription-create-submit').click();
  await page.waitForURL(/\/admin-new\/workflow-event-subscriptions\/[^/]+$/, {
    timeout: options.connectTimeoutMs,
  });
  await page.getByTestId('workflow-event-subscription-detail-name').waitFor({ state: 'visible' });
  assert.equal(
    (await page.content()).includes(signingSecret),
    false,
    'signing secret leaked into HTML',
  );

  const subscriptionId = new URL(page.url()).pathname.split('/').at(-1);
  assert.ok(subscriptionId, 'event subscription detail URL must contain an id');
  const run = await generateDeliveredEvents(accessToken, options, runLabel);
  try {
    await poll(
      'workflow event catalog deliveries',
      async () =>
        await fetchJson(
          `${apiOrigin(options)}/api/v1/workflow-event-subscriptions/${subscriptionId}/deliveries`,
          { headers: { Authorization: `Bearer ${accessToken}` } },
        ),
      (response) =>
        response.deliveries?.length >= 3 &&
        response.deliveries.every((delivery) => delivery.state === 'delivered'),
      options.connectTimeoutMs,
    );
  } finally {
    await deleteSession(accessToken, options, run.session_id);
  }
  await page.reload({ waitUntil: 'domcontentloaded' });
  await page.getByTestId('workflow-events-metric-delivered').waitFor({ state: 'visible' });
  assert.match(
    (await page.getByTestId('workflow-events-metric-delivered').textContent()) ?? '',
    /3/,
  );
  await assertNoBodyHorizontalOverflow(page, 'workflow event subscription detail');
  await page.getByTestId('workflow-event-subscription-delete').click();
  await page.getByTestId('workflow-event-subscription-delete-confirm-button').click();
  await page.waitForURL(/\/admin-new\/workflow-event-subscriptions\/?$/, {
    timeout: options.connectTimeoutMs,
  });
  return { id: subscriptionId, name, runId: run.id, deleted: true };
}

async function assertServerRejection(page, options) {
  const pattern = '**/api/v1/workflow-event-subscriptions';
  const handler = async (route) => {
    if (route.request().method() !== 'POST') return await route.fallback();
    await route.fulfill({
      status: 422,
      contentType: 'application/json',
      body: JSON.stringify({ error: 'Receiver policy rejected this subscription.' }),
    });
  };
  await page.route(pattern, handler);
  try {
    await page.getByTestId('workflow-event-subscription-create-submit').click();
    await page.getByTestId('workflow-event-subscription-create-error').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });
  } finally {
    await page.unroute(pattern, handler);
  }
}

async function generateDeliveredEvents(accessToken, options, runLabel) {
  const headers = authJsonHeaders(accessToken);
  const root = apiOrigin(options);
  const workflow = await fetchJson(`${root}/api/v1/workflows`, {
    method: 'POST',
    headers,
    body: JSON.stringify({ name: `Catalog event source ${runLabel}`, labels: { run: runLabel } }),
  });
  await fetchJson(`${root}/api/v1/workflows/${workflow.id}/versions`, {
    method: 'POST',
    headers,
    body: JSON.stringify({
      version: 'v1',
      executor: 'manual',
      entrypoint: 'workflows/catalog/run.mjs',
    }),
  });
  const run = await fetchJson(`${root}/api/v1/workflow-runs`, {
    method: 'POST',
    headers,
    body: JSON.stringify({
      workflow_id: workflow.id,
      version: 'v1',
      session: { create_session: {} },
    }),
  });
  const access = await fetchJson(`${root}/api/v1/sessions/${run.session_id}/automation-access`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${accessToken}` },
  });
  for (const state of ['running', 'succeeded']) {
    await fetchJson(`${root}/api/v1/workflow-runs/${run.id}/state`, {
      method: 'POST',
      headers: {
        'x-bpane-automation-access-token': access.token,
        'content-type': 'application/json',
      },
      body: JSON.stringify({
        state,
        message: `catalog smoke ${state}`,
        ...(state === 'succeeded' ? { output: { ok: true } } : {}),
      }),
    });
  }
  return run;
}
