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

test('identity collection scopes images to tested services and fixture projects', () => {
  const configCalls = [];
  const execute = (command, args) => {
    if (command === 'git') return `${'a'.repeat(40)}\n`;
    if (command === 'docker' && args.includes('config')) {
      configCalls.push(args);
      if (args.includes('deploy/compose.yml')) return 'deploy-gateway\n';
      if (args.some((arg) => arg.endsWith('/compose.tls.yml'))) {
        return 'mitmproxy/mitmproxy:11.0.2\n';
      }
      const projectIndex = args.indexOf('--project-name');
      const project = projectIndex >= 0 ? args[projectIndex + 1] : 'egress-observer';
      return `${project}-egress-proxy\n${project}-egress-auth-proxy\n`;
    }
    if (command === 'docker' && args.includes('inspect')) {
      if (args.at(-1).startsWith('egress-observer-')) throw new Error('wrong project');
      return `[] sha256:${'b'.repeat(64)}`;
    }
    throw new Error('unexpected fixture command');
  };

  const result = new ComposeIdentityCollector(root, execute).collect('admin-compatibility');
  const baseCall = configCalls.find((args) => args.includes('deploy/compose.yml'));
  const observerCall = configCalls.find((args) =>
    args.includes('deploy/examples/egress-observer/compose.yml'));
  const tlsCall = configCalls.find((args) =>
    args.some((arg) => arg.endsWith('/compose.tls.yml')));

  assert.equal(result.errors.length, 0);
  assert.ok(baseCall.includes('gateway'));
  assert.equal(baseCall.includes('runtime-broker'), false);
  assert.deepEqual(observerCall.slice(1, 3), ['--project-name', 'bpane-ci-egress']);
  assert.deepEqual(tlsCall.slice(1, 3), ['--project-name', 'bpane-ci-egress-tls']);
});
