import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import { ComposeStageHarness } from '../compose-harness/compose-stage-harness.mjs';

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
