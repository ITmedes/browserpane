import assert from 'node:assert/strict';
import test from 'node:test';

import { GrafanaValidationStack } from './grafana-validation-stack.mjs';

test('Grafana validation stack uses a private fixture and cleans every resource', () => {
  const runner = new FakeDockerRunner();
  const stack = new GrafanaValidationStack(process.cwd(), runner, () => 'unit-test');

  const containers = stack.start();
  stack.stop();

  assert.deepEqual(containers, { grafana: 'grafana-id', prometheus: 'prometheus-id' });
  assert(runner.has(['network', 'create', '--internal', 'bpane-observe-unit-test']));
  assert(runner.includes('run', '--network-alias', 'gateway'));
  assert(runner.includes('compose', 'up', '-d', '--wait'));
  assert(runner.includes('compose', 'down', '--remove-orphans', '--volumes'));
  assert(runner.has(['rm', '--force', 'bpane-observe-unit-test-gateway']));
  assert(runner.has(['network', 'rm', 'bpane-observe-unit-test']));
});

test('Grafana validation stack cleans resources after startup failure', () => {
  const runner = new FakeDockerRunner({ failComposeUp: true });
  const stack = new GrafanaValidationStack(process.cwd(), runner, () => 'failure-test');

  assert.throws(() => stack.start(), /simulated Compose failure/);

  assert(runner.includes('compose', 'down', '--remove-orphans', '--volumes'));
  assert(runner.has(['rm', '--force', 'bpane-observe-failure-test-gateway']));
  assert(runner.has(['network', 'rm', 'bpane-observe-failure-test']));
});

class FakeDockerRunner {
  commands = [];
  #options;

  constructor(options = {}) {
    this.#options = options;
  }

  run(args) {
    this.commands.push(args);
    if (this.#options.failComposeUp && args[0] === 'compose' && args.includes('up')) {
      throw new Error('simulated Compose failure');
    }
    if (args[0] === 'compose' && args.includes('ps')) {
      return args.at(-1) === 'grafana' ? 'grafana-id' : 'prometheus-id';
    }
    return '';
  }

  tryRun(args) {
    try {
      return this.run(args);
    } catch {
      return '';
    }
  }

  has(expected) {
    return this.commands.some((command) => JSON.stringify(command) === JSON.stringify(expected));
  }

  includes(first, ...values) {
    return this.commands.some((command) => command[0] === first
      && values.every((value) => command.includes(value)));
  }
}
