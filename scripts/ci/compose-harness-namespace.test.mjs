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
import {
  ComposeResourceRegistry,
  namespaceApiPayload,
} from '../compose-harness/resource-registry.mjs';

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

test('workflow-run session creation receives the stage namespace without broadening bindings', () => {
  const namespace = 'bpane-run-stage';
  const created = namespaceApiPayload(
    'http://localhost:8932/api/v1/workflow-runs',
    'POST',
    {
      labels: { suite: 'workflow' },
      session: { create_session: { project_id: 'project-id' } },
    },
    namespace,
  );
  const bound = namespaceApiPayload(
    'http://localhost:8932/api/v1/workflow-runs',
    'POST',
    { session: { session_id: 'session-id', create_session: null } },
    namespace,
  );

  assert.equal(created.session.create_session.labels.bpane_ci_namespace, namespace);
  assert.equal(bound.session.create_session, null);
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
  const responseBody = Buffer.from(JSON.stringify({
    id: '019db438-c74a-7ef2-810c-792e298faf11',
  }));
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
        body: async () => responseBody,
      };
    },
    fulfill: async ({ body }) => { fulfilled = body === responseBody; },
  });

  assert.equal(rewrittenBody.labels.bpane_ci_namespace, 'bpane-run-stage');
  assert.equal(fulfilled, true);
  assert.deepEqual(new ComposeResourceRegistry(registryPath, 'bpane-run-stage').load(), [{
    namespace: 'bpane-run-stage',
    kind: 'session',
    id: '019db438-c74a-7ef2-810c-792e298faf11',
  }]);
});

test('Playwright forwards action responses and registers their created resource', async (t) => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'bpane-transport-action-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  const registryPath = path.join(directory, 'resources.jsonl');
  let routeHandler = null;
  await installNamespacedPlaywrightTarget({
    route: async (_pattern, handler) => { routeHandler = handler; },
  }, {
    BPANE_CI_STAGE_NAMESPACE: 'bpane-run-stage',
    BPANE_CI_RESOURCE_REGISTRY: registryPath,
  });
  const responseBody = Buffer.from(JSON.stringify({
    id: '019db438-c74a-7ef2-810c-792e298faf12',
  }));
  let forwardedBody = null;

  await routeHandler(routeFixture({
    url: 'http://localhost:8932/api/v1/browser-contexts/source-id/clone',
    responseBody,
    fulfill: ({ body }) => { forwardedBody = body; },
  }));

  assert.equal(forwardedBody, responseBody);
  assert.deepEqual(new ComposeResourceRegistry(registryPath, 'bpane-run-stage').load(), [{
    namespace: 'bpane-run-stage',
    kind: 'browser_context',
    id: '019db438-c74a-7ef2-810c-792e298faf12',
  }]);
});

test('Playwright preserves import bytes while namespacing import metadata', async (t) => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'bpane-transport-import-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  let routeHandler = null;
  await installNamespacedPlaywrightTarget({
    route: async (_pattern, handler) => { routeHandler = handler; },
  }, {
    BPANE_CI_STAGE_NAMESPACE: 'bpane-run-stage',
    BPANE_CI_RESOURCE_REGISTRY: path.join(directory, 'resources.jsonl'),
  });
  let fetchOptions = null;

  await routeHandler(routeFixture({
    url: 'http://localhost:8932/api/v1/browser-contexts/import',
    postData: 'binary archive bytes',
    headers: {
      'content-type': 'application/zip',
      'x-bpane-browser-context-labels': JSON.stringify({ suite: 'smoke' }),
    },
    fetch: async (options) => {
      fetchOptions = options;
      return { ok: () => true, body: async () => Buffer.from('{}') };
    },
  }));

  assert.equal(fetchOptions.postData, undefined);
  assert.deepEqual(JSON.parse(fetchOptions.headers['x-bpane-browser-context-labels']), {
    suite: 'smoke',
    bpane_ci_namespace: 'bpane-run-stage',
  });
});

test('Playwright ignores only target closure during route teardown', async (t) => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'bpane-transport-close-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  let routeHandler = null;
  await installNamespacedPlaywrightTarget({
    route: async (_pattern, handler) => { routeHandler = handler; },
  }, {
    BPANE_CI_STAGE_NAMESPACE: 'bpane-run-stage',
    BPANE_CI_RESOURCE_REGISTRY: path.join(directory, 'resources.jsonl'),
  });
  const closed = new Error('Target page, context or browser has been closed');
  closed.name = 'TargetClosedError';

  await assert.doesNotReject(routeHandler(routeFixture({ fetchError: closed })));
  await assert.rejects(routeHandler(routeFixture({ fetchError: new Error('network failed') })),
    /network failed/);
});

function routeFixture({
  url = 'http://localhost:8932/api/v1/sessions',
  responseBody = Buffer.from('{}'),
  fulfill = () => {},
  fetchError = null,
  fetch = null,
  postData = JSON.stringify({ labels: { suite: 'smoke' } }),
  headers = { 'content-type': 'application/json' },
} = {}) {
  return {
    request: () => ({
      method: () => 'POST',
      url: () => url,
      postData: () => postData,
      headers: () => headers,
    }),
    fetch: fetch ?? (async () => {
      if (fetchError) throw fetchError;
      return { ok: () => true, body: async () => responseBody };
    }),
    fulfill,
  };
}
