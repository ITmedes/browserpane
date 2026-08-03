import assert from 'node:assert/strict';
import test from 'node:test';

import { SubprocessStageExecutor } from './subprocess-executor.mjs';

function stage(script, timeoutMs = 2000) {
  return {
    command: process.execPath,
    args: ['-e', script],
    cwd: process.cwd(),
    timeoutMs
  };
}

test('subprocess executor preserves a child exit code', async () => {
  const executor = new SubprocessStageExecutor({ stdio: 'ignore' });

  const result = await executor.execute(stage('process.exit(23)'));

  assert.equal(result.exitCode, 23);
  assert.equal(result.timedOut, false);
});

test('subprocess executor times out and terminates a stuck process group', { timeout: 3000 }, async () => {
  const executor = new SubprocessStageExecutor({ stdio: 'ignore', killGraceMs: 25 });
  const script = "process.on('SIGTERM', () => {}); setInterval(() => {}, 1000)";

  const result = await executor.execute(stage(script, 50));

  assert.equal(result.exitCode, 124);
  assert.equal(result.timedOut, true);
});

test('subprocess executor maps cancellation to the originating signal', { timeout: 3000 }, async () => {
  const executor = new SubprocessStageExecutor({ stdio: 'ignore', killGraceMs: 25 });
  const execution = executor.execute(stage('setInterval(() => {}, 1000)'));
  setTimeout(() => executor.cancel('SIGINT'), 50);

  const result = await execution;

  assert.equal(result.exitCode, 130);
  assert.equal(result.signal, 'SIGINT');
});
