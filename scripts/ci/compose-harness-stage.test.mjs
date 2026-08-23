import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import { ComposeStageHarness } from '../compose-harness/compose-stage-harness.mjs';
import { ComposeResourceRegistry } from '../compose-harness/resource-registry.mjs';

test('a primary command failure remains available when bounded teardown also fails', async (t) => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'bpane-stage-harness-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  let teardownCalls = 0;
  const harness = new ComposeStageHarness(directory, directory, {
    namespaceFactory: { stage: () => 'bpane-run-stage' },
    verifierFactory: () => ({
      baseline: async () => inventory(),
      verify: async () => {
        teardownCalls += 1;
        throw new Error('fixture cleanup failure');
      },
    }),
  });

  const result = await harness.run({
    lane: 'gateway-default',
    stage: { id: 'gateway-suite', isolation: 'resources' },
    command: ['fixture'],
    execute: async () => ({
      exitCode: 17, signal: null, spawnError: false, requestedSignal: null,
    }),
    runNamespace: 'bpane-run',
    environment: {},
  });

  assert.equal(result.execution.exitCode, 17);
  assert.ok(result.cleanupError);
  assert.equal(teardownCalls, 1);
});

test('a cancelled primary command still executes bounded teardown', async (t) => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'bpane-cancelled-stage-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  let teardownCalls = 0;
  const harness = new ComposeStageHarness(directory, directory, {
    namespaceFactory: { stage: () => 'bpane-run-cancelled' },
    verifierFactory: () => ({
      baseline: async () => inventory(),
      verify: async () => { teardownCalls += 1; },
    }),
  });

  const result = await harness.run({
    lane: 'admin-unified',
    stage: { id: 'admin-validation', isolation: 'resources' },
    command: ['fixture'],
    execute: async () => ({
      exitCode: 143, signal: 'SIGTERM', spawnError: false, requestedSignal: 'SIGTERM',
    }),
    runNamespace: 'bpane-run',
    environment: {},
  });

  assert.equal(result.execution.exitCode, 143);
  assert.equal(result.execution.requestedSignal, 'SIGTERM');
  assert.equal(teardownCalls, 1);
});

test('nested stages propagate owned resources to the enclosing lane registry', async (t) => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'bpane-nested-stage-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  const parentPath = path.join(directory, 'parent-resources.jsonl');
  const sessionId = '019db438-c74a-7ef2-810c-792e298faf11';
  const harness = new ComposeStageHarness(directory, directory, {
    namespaceFactory: { stage: () => 'bpane-run-child' },
    verifierFactory: () => ({
      baseline: async () => inventory(),
      verify: async () => {},
    }),
  });

  await harness.run({
    lane: 'browser-integrations',
    stage: { id: 'browser-validation', isolation: 'resources' },
    command: ['fixture'],
    execute: async (_command, environment) => {
      new ComposeResourceRegistry(
        environment.BPANE_CI_RESOURCE_REGISTRY,
        environment.BPANE_CI_STAGE_NAMESPACE,
      ).record('session', sessionId);
      return { exitCode: 0, signal: null, spawnError: false, requestedSignal: null };
    },
    runNamespace: 'bpane-run',
    environment: {
      BPANE_CI_RESOURCE_REGISTRY: parentPath,
      BPANE_CI_STAGE_NAMESPACE: 'bpane-run-parent',
    },
  });

  assert.deepEqual(
    new ComposeResourceRegistry(parentPath, 'bpane-run-parent').load(),
    [{ namespace: 'bpane-run-parent', kind: 'session', id: sessionId }],
  );
});

function inventory() {
  return {
    active_sessions: 0,
    owned_containers: 0,
    owned_volumes: 0,
    retained_volumes: 0,
    retained_volume_names: [],
    dynamic_containers: [],
    dynamic_volumes: [],
  };
}
