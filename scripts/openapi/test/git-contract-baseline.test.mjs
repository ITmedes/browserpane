import assert from 'node:assert/strict';
import test from 'node:test';

import { GitContractBaseline } from '../src/git-contract-baseline.mjs';

test('verifies a commit before loading the contract without shell interpolation', () => {
  const calls = [];
  const baseline = new GitContractBaseline('/repo', (command, args, options) => {
    calls.push({ command, args, options });
    return { status: 0, stdout: calls.length === 1 ? 'sha\n' : 'openapi: 3.0.3\n', stderr: '' };
  });

  assert.equal(baseline.load('feature/ref', 'openapi/api.yaml'), 'openapi: 3.0.3\n');
  assert.deepEqual(calls.map((call) => call.args), [
    ['rev-parse', '--verify', 'feature/ref^{commit}'],
    ['show', 'feature/ref:openapi/api.yaml']
  ]);
});

test('rejects missing revisions without attempting git show', () => {
  let calls = 0;
  const baseline = new GitContractBaseline('/repo', () => {
    calls += 1;
    return { status: 128, stdout: '', stderr: 'unknown revision' };
  });

  assert.throws(() => baseline.load('missing', 'openapi/api.yaml'), /not a commit/);
  assert.equal(calls, 1);
});
