import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import {
  ComposeNamespaceFactory,
  validateComposeNamespace,
} from '../compose-harness/compose-namespace.mjs';
import { installNamespacedPlaywrightTarget } from '../compose-harness/namespace-transports.mjs';
import { ComposeResourceRegistry } from '../compose-harness/resource-registry.mjs';

test('GitHub run and stage namespaces are deterministic and bounded', () => {
  const factory = new ComposeNamespaceFactory(() => 'unused');
  const environment = { GITHUB_RUN_ID: '32627746784', GITHUB_RUN_ATTEMPT: '2' };

  const run = factory.run('admin-compatibility', environment);
  const stage = factory.stage(run, 'compose-admin-compat-workflow-run-detail');

  assert.equal(run, 'bpane-32627746784-2-admin-compatibility');
  assert.equal(factory.run('admin-compatibility', environment), run);
  assert.equal(factory.stage(run, 'compose-admin-compat-workflow-run-detail'), stage);
  assert.ok(stage.length <= 63);
  assert.match(stage, /^[a-z0-9-]+$/u);
});

test('local attempts receive different run namespaces', () => {
  const values = ['11111111-1111-1111-1111-111111111111', '22222222-2222-2222-2222-222222222222'];
  const factory = new ComposeNamespaceFactory(() => values.shift());

  assert.notEqual(factory.run('gateway-default', {}), factory.run('gateway-default', {}));
});

test('configured namespaces fail closed when malformed', () => {
  assert.throws(() => validateComposeNamespace('../foreign'), /Compose namespace/);
  assert.throws(
    () => new ComposeNamespaceFactory().run('lane', { BPANE_CI_RUN_NAMESPACE: 'UPPER' }),
    /Compose namespace/,
  );
});

test('Playwright requests carry ownership and register created resources', async (t) => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'bpane-transport-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  const registryPath = path.join(directory, 'resources.jsonl');
  let routeHandler = null;
  const target = {
    route: async (_pattern, handler) => { routeHandler = handler; },
  };
  await installNamespacedPlaywrightTarget(target, {
    BPANE_CI_STAGE_NAMESPACE: 'bpane-run-stage',
    BPANE_CI_RESOURCE_REGISTRY: registryPath,
  });

  let rewrittenBody = null;
  let fulfilled = false;
  await routeHandler({
    request: () => ({
      method: () => 'POST',
      url: () => 'http://localhost:8932/api/v1/sessions',
      postData: () => JSON.stringify({ labels: { suite: 'smoke' } }),
    }),
    fetch: async ({ postData }) => {
      rewrittenBody = JSON.parse(postData);
      return {
        ok: () => true,
        json: async () => ({ id: '019db438-c74a-7ef2-810c-792e298faf11' }),
      };
    },
    fulfill: async () => { fulfilled = true; },
  });

  assert.equal(rewrittenBody.labels.bpane_ci_namespace, 'bpane-run-stage');
  assert.equal(fulfilled, true);
  assert.deepEqual(new ComposeResourceRegistry(registryPath, 'bpane-run-stage').load(), [{
    namespace: 'bpane-run-stage',
    kind: 'session',
    id: '019db438-c74a-7ef2-810c-792e298faf11',
  }]);
});
