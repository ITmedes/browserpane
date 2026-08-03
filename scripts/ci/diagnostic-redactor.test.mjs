import assert from 'node:assert/strict';
import test from 'node:test';

import { DiagnosticRedactor } from './diagnostic-redactor.mjs';

test('diagnostic redactor removes credentials, OIDC material, IDs, and PEM blocks', () => {
  const input = [
    'Authorization: Bearer bearer-value',
    'Proxy-Authorization: Basic dXNlcjpwYXNz',
    'Cookie: session=sensitive',
    'https://proxy-user:proxy-pass@proxy.example',
    'https://example.test/cb?code=oidc-code&access_token=access-value',
    '{"client_secret":"client-value","password":"password-value"}',
    'BPANE_GATEWAY_OIDC_CLIENT_SECRET=environment-secret-value',
    '--token cli-value --credential-vault-token=vault-value',
    'eyJhbGciOiJSUzI1NiJ9.eyJzdWIiOiJ1c2VyIn0.signature',
    '019f1f6d-9ee8-7abc-8def-0123456789ab',
    'bpane-runtime-019f1f6d9ee87abc8def0123456789ab',
    '-----BEGIN PRIVATE KEY-----\nprivate-value\n-----END PRIVATE KEY-----'
  ].join('\n');

  const result = new DiagnosticRedactor().redact(input);

  for (const secret of ['bearer-value', 'dXNlcjpwYXNz', 'sensitive', 'proxy-pass',
    'oidc-code', 'access-value', 'client-value', 'password-value', 'cli-value',
    'environment-secret-value', 'vault-value', 'private-value', '019f1f6d-9ee8']) {
    assert.ok(!result.includes(secret), `expected ${secret} to be redacted`);
  }
  assert.match(result, /<redacted>/);
  assert.match(result, /bpane-runtime-<id>/);
});
