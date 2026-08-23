import assert from 'node:assert/strict';
import test from 'node:test';

import { workflowControlPlaneTimeoutMs } from '../../code/web/bpane-client/scripts/workflow-smoke-lib.mjs';
import { ReadinessWaiter, boundaryReady } from '../compose-harness/readiness-waiter.mjs';

test('typed boundaries distinguish control, runtime, transport, workers, and artifacts', () => {
  const ready = {
    status: 'ready',
    lifecycle: 'running',
    checks: [
      { name: 'session_store', status: 'ready' },
      { name: 'runtime_manager', status: 'ready' },
    ],
  };
  assert.equal(boundaryReady('oidc', { issuer: 'issuer', token_endpoint: 'token' }), true);
  assert.equal(boundaryReady('control', ready), true);
  assert.equal(boundaryReady('runtime', ready), true);
  assert.equal(boundaryReady('transport', { connected: true, application_ready: false }), false);
  assert.equal(boundaryReady('transport', { connected: true, application_ready: true }), true);
  assert.equal(boundaryReady('workflow_worker', { state: 'running' }), true);
  assert.equal(boundaryReady('recording_worker', { state: 'finalizing' }), true);
  assert.equal(boundaryReady('artifact', { available: true }), true);
});

test('delayed readiness records attempts, elapsed time, and last safe state', async () => {
  let clock = 0;
  const states = [{ status: 'starting' }, { status: 'starting' }, { status: 'ready' }];
  const events = [];
  const waiter = new ReadinessWaiter({
    observe: async () => states.shift(),
    clock: () => clock,
    wait: async (duration) => { clock += duration; },
    record: (event) => events.push(event),
  });

  const result = await waiter.waitFor('control', {
    timeoutMs: 10,
    intervalMs: 2,
    isReady: (_boundary, state) => state.status === 'ready',
  });

  assert.equal(result.attempts, 3);
  assert.equal(result.elapsed_ms, 4);
  assert.equal(result.last_state.status, 'ready');
  assert.equal(events[0].outcome, 'ready');
});

test('timeout, stale resource, dependency loss, and cancellation stay distinct', async () => {
  await assert.rejects(
    deterministicWaiter(async () => ({ status: 'starting' })).waitFor('runtime', {
      timeoutMs: 2, intervalMs: 1,
    }),
    (error) => error.code === 'readiness_timeout' && error.details.attempts === 3,
  );
  await assert.rejects(
    deterministicWaiter(async () => ({ resource_id: 'old', status: 'ready' }))
      .waitFor('workflow_worker', {
        timeoutMs: 2, intervalMs: 1, expectedResourceId: 'new',
      }),
    (error) => error.code === 'stale_resource',
  );
  await assert.rejects(
    deterministicWaiter(async () => { throw new Error('offline'); }).waitFor('control', {
      timeoutMs: 2, intervalMs: 1,
    }),
    (error) => error.code === 'dependency_loss',
  );
  const controller = new AbortController();
  controller.abort();
  await assert.rejects(
    deterministicWaiter(async () => ({})).waitFor('artifact', {
      timeoutMs: 2, intervalMs: 1, signal: controller.signal,
    }),
    (error) => error.code === 'cancelled',
  );
});

test('a recovered dependency can still reach readiness before its deadline', async () => {
  let attempts = 0;
  const result = await deterministicWaiter(async () => {
    attempts += 1;
    if (attempts === 1) throw new Error('temporarily unavailable');
    return { available: true };
  }).waitFor('artifact', { timeoutMs: 2, intervalMs: 1 });

  assert.equal(result.attempts, 2);
  assert.equal(result.last_state.available, true);
});

test('workflow gateway recovery uses the shared bounded readiness deadline', () => {
  assert.equal(workflowControlPlaneTimeoutMs({ connectTimeoutMs: 30_000 }), 120_000);
  assert.equal(workflowControlPlaneTimeoutMs({ connectTimeoutMs: 180_000 }), 180_000);
});

function deterministicWaiter(observe) {
  let clock = 0;
  return new ReadinessWaiter({
    observe,
    clock: () => clock,
    wait: async (duration) => { clock += duration; },
  });
}
