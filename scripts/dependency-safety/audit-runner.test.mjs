import assert from 'node:assert/strict';
import test from 'node:test';

import { DependencyAuditRunner } from './audit-runner.mjs';

class CommandFixture {
  #responses;
  calls = [];

  constructor(responses) {
    this.#responses = [...responses];
  }

  execute = (command, args, options) => {
    this.calls.push({ command, args, cwd: options.cwd });
    const response = this.#responses.shift();
    if (!response) throw new Error(`unexpected command: ${command} ${args.join(' ')}`);
    return { status: 0, stdout: '', stderr: '', ...response };
  };
}

test('runner scans cargo and every committed npm lockfile', () => {
  const fixture = new CommandFixture([
    { stdout: 'cargo-audit 0.22.2\n' },
    { stdout: 'a/package-lock.json\nb/package-lock.json\n' },
    { stdout: JSON.stringify({ vulnerabilities: { list: [] } }) },
    { stdout: JSON.stringify({ vulnerabilities: {} }) },
    { stdout: JSON.stringify({ vulnerabilities: {} }) }
  ]);
  const runner = new DependencyAuditRunner('/repo', fixture.execute);

  const result = runner.run();

  assert.deepEqual(result.npmLockfiles, ['a/package-lock.json', 'b/package-lock.json']);
  assert.deepEqual(result.findings, []);
  assert.deepEqual(
    fixture.calls.slice(3).map((call) => call.cwd),
    ['/repo/a', '/repo/b']
  );
});

test('runner reports the cargo-audit installation prerequisite', () => {
  const fixture = new CommandFixture([{ status: 1, stderr: 'no such command' }]);
  const runner = new DependencyAuditRunner('/repo', fixture.execute);

  assert.throws(
    () => runner.run(),
    /cargo install cargo-audit --locked/
  );
});

test('runner rejects invalid audit output rather than reporting a clean scan', () => {
  const fixture = new CommandFixture([
    { stdout: 'cargo-audit 0.22.2\n' },
    { stdout: 'a/package-lock.json\n' },
    { status: 1, stdout: 'not-json', stderr: 'registry failure' }
  ]);
  const runner = new DependencyAuditRunner('/repo', fixture.execute);

  assert.throws(
    () => runner.run(),
    /cargo audit did not return JSON: registry failure/
  );
});
