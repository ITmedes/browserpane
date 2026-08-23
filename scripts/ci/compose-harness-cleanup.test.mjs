import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import { ComposeCleanupVerifier } from '../compose-harness/cleanup-verifier.mjs';
import { ComposeStageHarness } from '../compose-harness/compose-stage-harness.mjs';
import { ComposeResourceRegistry, namespaceApiPayload } from '../compose-harness/resource-registry.mjs';

test('resource payloads carry the stage namespace through nested session defaults', () => {
  const payload = namespaceApiPayload(
    'http://localhost:8932/api/v1/workflows',
    'POST',
    { name: 'workflow', labels: { suite: 'smoke' }, default_session: { labels: {} } },
    'bpane-123-1-workflow',
  );

  assert.equal(payload.labels.bpane_ci_namespace, 'bpane-123-1-workflow');
  assert.equal(payload.default_session.labels.bpane_ci_namespace, 'bpane-123-1-workflow');
});

test('actions and unlabeled contracts are not broadened by namespace injection', () => {
  const action = { state: 'cancelled' };
  const subscription = { name: 'receiver', target_url: 'http://receiver.test' };

  assert.deepEqual(namespaceApiPayload(
    'http://localhost:8932/api/v1/workflow-runs/run-1/state',
    'POST', action, 'bpane-run-stage',
  ), action);
  assert.deepEqual(namespaceApiPayload(
    'http://localhost:8932/api/v1/workflow-event-subscriptions',
    'POST', subscription, 'bpane-run-stage',
  ), subscription);
});

test('registries reject another namespace instead of deleting foreign resources', (t) => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'bpane-registry-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  const filePath = path.join(directory, 'resources.jsonl');
  new ComposeResourceRegistry(filePath, 'bpane-run-a').record('session', 'session-a');

  assert.throws(
    () => new ComposeResourceRegistry(filePath, 'bpane-run-b').load(),
    /foreign record/,
  );
});

test('cleanup waits through asynchronous lifecycle drain and accounts for every invariant', async () => {
  let clock = 0;
  const snapshots = [
    inventory(),
    inventory({ active_sessions: 1, owned_containers: 1 }),
    inventory({ owned_containers: 1 }),
    inventory(),
  ];
  const events = [];
  const verifier = new ComposeCleanupVerifier({
    inspector: { snapshot: async () => snapshots.shift() ?? inventory() },
    clock: () => clock,
    wait: async () => { clock += 1; },
    record: (event) => events.push(event),
  });
  const baseline = await verifier.baseline();

  const result = await verifier.verify({
    namespace: 'bpane-run-stage', records: [], baseline, timeoutMs: 5, cleanPasses: 1,
  });

  assert.equal(result.outcome, 'success');
  assert.equal(result.attempts, 3);
  assert.equal(events[0].inventory.status, 'clean');
});

test('cleanup requires a stable clean window and tolerates hosted runtime drain', async () => {
  let clock = 0;
  const snapshots = [
    inventory(),
    inventory(),
    inventory({ owned_containers: 1 }),
    inventory(),
    inventory(),
    inventory(),
  ];
  const verifier = new ComposeCleanupVerifier({
    inspector: { snapshot: async () => snapshots.shift() ?? inventory() },
    clock: () => clock,
    wait: async () => { clock += 10_000; },
  });
  const baseline = await verifier.baseline();

  const result = await verifier.verify({
    namespace: 'bpane-run-stage', records: [], baseline, cleanPasses: 3,
  });

  assert.equal(result.outcome, 'success');
  assert.equal(result.attempts, 5);
  assert.equal(result.elapsed_ms, 40_000);
});

test('cleanup permits stopped-session storage while rejecting temporary volume leaks', async () => {
  const sessionVolume = 'deploy_bpane-session-data-019db438c74a7ef2810c792e298faf11';
  const verifier = new ComposeCleanupVerifier({
    inspector: {
      snapshot: async (records) => records.length === 0
        ? inventory()
        : inventory({
          retained_volumes: 1,
          retained_volume_names: [sessionVolume],
          dynamic_volumes: [sessionVolume],
        }),
    },
  });
  const baseline = await verifier.baseline();

  const result = await verifier.verify({
    namespace: 'bpane-run-stage',
    records: [{ kind: 'session', id: '019db438-c74a-7ef2-810c-792e298faf11' }],
    baseline,
  });

  assert.equal(result.outcome, 'success');
  assert.equal(result.inventory.retained_volumes, 1);
});

test('cleanup failure preserves bounded accounting for active sessions and storage leaks', async () => {
  let clock = 0;
  const verifier = new ComposeCleanupVerifier({
    inspector: { snapshot: async () => inventory({ active_sessions: 1, owned_volumes: 1 }) },
    clock: () => clock,
    wait: async () => { clock += 1; },
  });
  const baseline = await verifier.baseline();

  await assert.rejects(
    verifier.verify({
      namespace: 'bpane-run-stage', records: [], baseline, timeoutMs: 2, cleanPasses: 1,
    }),
    (error) => error.code === 'cleanup_failure'
      && error.details.inventory.active_sessions === 1
      && error.details.inventory.owned_volumes === 1,
  );
});

test('a later stage refuses a registered leak without touching a foreign namespace', async (t) => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'bpane-prior-leak-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  const registryDirectory = path.join(directory, 'harness');
  fs.mkdirSync(registryDirectory);
  new ComposeResourceRegistry(
    path.join(registryDirectory, 'previous-resources.jsonl'),
    'bpane-run-previous',
  ).record('session', 'session-previous');
  let executed = false;
  const harness = new ComposeStageHarness(directory, directory, {
    namespaceFactory: { stage: () => 'bpane-run-current' },
    verifierFactory: () => ({
      baseline: async (records = []) => inventory({ owned_containers: records.length }),
      verify: async () => {},
    }),
  });

  const result = await harness.run({
    lane: 'browser-integrations',
    stage: { id: 'browser-validation', isolation: 'resources' },
    command: ['fixture'],
    execute: async () => {
      executed = true;
      return { exitCode: 0, signal: null, spawnError: false, requestedSignal: null };
    },
    runNamespace: 'bpane-run',
    environment: {},
  });

  assert.equal(executed, false);
  assert.equal(result.execution.exitCode, 1);
  assert.equal(result.harness.cleanup.outcome, 'failure');
});

function inventory(overrides = {}) {
  return {
    active_sessions: 0,
    owned_containers: 0,
    owned_volumes: 0,
    retained_volumes: 0,
    retained_volume_names: [],
    dynamic_containers: [],
    dynamic_volumes: [],
    ...overrides,
  };
}
