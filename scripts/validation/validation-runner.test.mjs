import assert from 'node:assert/strict';
import test from 'node:test';

import { ValidationRunner } from './validation-runner.mjs';

class ExecutorFixture {
  #results;
  executed = [];
  cancelled = [];

  constructor(results) {
    this.#results = [...results];
  }

  async execute(stage) {
    this.executed.push(stage.id);
    return this.#results.shift() ?? { exitCode: 0, timedOut: false };
  }

  cancel(signal) {
    this.cancelled.push(signal);
  }
}

class LoggerFixture {
  messages = [];

  log = (message) => this.messages.push(message);
  error = (message) => this.messages.push(message);
}

function stage(id) {
  return {
    id,
    description: `${id} description`,
    cwd: '/repo',
    timeoutMs: 1000,
    commandLine: () => `run ${id}`
  };
}

test('runner stops on the first failure and preserves its exit code', async () => {
  const executor = new ExecutorFixture([
    { exitCode: 0, timedOut: false },
    { exitCode: 17, timedOut: false },
    { exitCode: 0, timedOut: false }
  ]);
  const logger = new LoggerFixture();
  const runner = new ValidationRunner(executor, logger);

  const exitCode = await runner.run([stage('first'), stage('second'), stage('third')]);

  assert.equal(exitCode, 17);
  assert.deepEqual(executor.executed, ['first', 'second']);
  assert.ok(logger.messages.some((message) => message.includes('--stage second')));
});

test('runner dry-run prints every command without executing it', async () => {
  const executor = new ExecutorFixture([]);
  const logger = new LoggerFixture();
  const runner = new ValidationRunner(executor, logger);

  const exitCode = await runner.run([stage('first'), stage('second')], { dryRun: true });

  assert.equal(exitCode, 0);
  assert.deepEqual(executor.executed, []);
  assert.ok(logger.messages.some((message) => message.includes('run second')));
});

test('runner cancellation prevents new stages and returns the signal exit code', async () => {
  const executor = new ExecutorFixture([]);
  const runner = new ValidationRunner(executor, new LoggerFixture());

  runner.cancel('SIGTERM');
  const exitCode = await runner.run([stage('first')]);

  assert.equal(exitCode, 143);
  assert.deepEqual(executor.cancelled, ['SIGTERM']);
  assert.deepEqual(executor.executed, []);
});
