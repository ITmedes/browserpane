import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import { SingleNodeConfigRenderer } from './render-config.mjs';

const VALID_ENVIRONMENT = {
  BPANE_DEPLOYMENT_NAME: 'bpane-production',
  BPANE_PUBLIC_GATEWAY_URL: 'https://browser.example:4433',
  BPANE_RECORDING_CONNECT_GATEWAY_URL: 'https://gateway:4433',
  BPANE_OIDC_TOKEN_URL: 'https://identity.example/oauth/token',
  BPANE_OIDC_WORKER_CLIENT_ID: 'bpane-worker',
  BPANE_BROWSER_START_URL: 'https://intranet.example/',
};

test('renders deterministic non-secret browser and worker policy', () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'bpane-single-node-'));

  new SingleNodeConfigRenderer().render(VALID_ENVIRONMENT, directory);

  const browserEnvironment = fs.readFileSync(path.join(directory, 'browser.env'), 'utf8');
  const workers = JSON.parse(fs.readFileSync(path.join(directory, 'workers.json'), 'utf8'));
  assert.match(browserEnvironment, /BPANE_URL=https:\/\/intranet\.example\//u);
  assert.doesNotMatch(browserEnvironment, /secret|password|token=/iu);
  assert.equal(workers.workflow.network, 'bpane-production-runtime');
  assert.equal(workers.recording.artifact_volume, 'bpane-production-recording-staging');
  assert.equal(workers.recording.connect_gateway_url, 'https://gateway:4433');
  if (process.platform !== 'win32') {
    assert.equal(fs.statSync(path.join(directory, 'browser.env')).mode & 0o777, 0o644);
    assert.equal(fs.statSync(path.join(directory, 'workers.json')).mode & 0o777, 0o644);
  }
});

test('fails closed when required deployment input is missing', () => {
  const environment = { ...VALID_ENVIRONMENT };
  delete environment.BPANE_OIDC_TOKEN_URL;

  assert.throws(
    () => new SingleNodeConfigRenderer().render(environment, '/unused'),
    /BPANE_OIDC_TOKEN_URL is required/u,
  );
});

test('rejects deployment names that would create invalid runtime DNS labels', () => {
  assert.throws(
    () => new SingleNodeConfigRenderer().render(
      { ...VALID_ENVIRONMENT, BPANE_DEPLOYMENT_NAME: 'a'.repeat(23) },
      '/unused',
    ),
    /valid DNS labels/u,
  );
});

test('rejects unsafe public gateway and credential-bearing URLs', () => {
  assert.throws(
    () => new SingleNodeConfigRenderer().render(
      { ...VALID_ENVIRONMENT, BPANE_PUBLIC_GATEWAY_URL: 'http://browser.example:4433' },
      '/unused',
    ),
    /must use https:/u,
  );
  assert.throws(
    () => new SingleNodeConfigRenderer().render(
      { ...VALID_ENVIRONMENT, BPANE_OIDC_TOKEN_URL: 'https://user:secret@identity.example/token' },
      '/unused',
    ),
    /must not contain credentials/u,
  );
});
