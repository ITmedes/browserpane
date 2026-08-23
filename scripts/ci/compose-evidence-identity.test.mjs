import assert from 'node:assert/strict';
import path from 'node:path';
import test from 'node:test';

import { ComposeIdentityCollector } from '../compose-evidence/compose-identity-collector.mjs';

const root = path.resolve(import.meta.dirname, '../..');

test('identity collector records exact git, plan, and image identities without image names', () => {
  const execute = (command, args) => {
    if (command === 'git' && args.at(-1) === 'HEAD') return `${'a'.repeat(40)}\n`;
    if (command === 'git' && args.at(-1) === 'HEAD^{tree}') return `${'b'.repeat(40)}\n`;
    if (command === 'docker' && args.includes('config')) {
      return `local-image\npinned@sha256:${'c'.repeat(64)}\n`;
    }
    if (command === 'docker' && args.includes('inspect')) {
      return `["registry/image@sha256:${'d'.repeat(64)}"] sha256:${'e'.repeat(64)}`;
    }
    throw new Error('unexpected fixture command');
  };

  const result = new ComposeIdentityCollector(root, execute).collect('gateway-default');

  assert.deepEqual(result.errors, []);
  assert.equal(result.identity.tested_commit, 'a'.repeat(40));
  assert.equal(result.identity.tested_tree, 'b'.repeat(40));
  assert.match(result.identity.workflow_sha256, /^[0-9a-f]{64}$/);
  assert.match(result.identity.test_plan_sha256, /^[0-9a-f]{64}$/);
  assert.deepEqual(result.identity.image_digests, [
    `sha256:${'c'.repeat(64)}`,
    `sha256:${'d'.repeat(64)}`,
    `sha256:${'e'.repeat(64)}`,
  ]);
  assert.doesNotMatch(JSON.stringify(result.identity), /registry|local-image/);
});

test('identity collection exposes bounded stable errors instead of raw command failures', () => {
  const execute = (command, args) => {
    if (command === 'git') return `${'a'.repeat(40)}\n`;
    if (command === 'docker' && args.includes('config')) return 'private-image-name\n';
    throw new Error('credential-bearing raw failure');
  };

  const result = new ComposeIdentityCollector(root, execute).collect('gateway-default');

  assert.deepEqual(result.errors, ['identity-image-1-unavailable']);
  assert.doesNotMatch(JSON.stringify(result), /credential-bearing|private-image-name/);
});
