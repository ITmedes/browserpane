import assert from 'node:assert/strict';
import test from 'node:test';

import Enforcer from 'openapi-enforcer';

import { ContractExampleValidator } from '../src/contract-example-validator.mjs';
import { ContractPaths } from '../src/contract-paths.mjs';
import { ExampleCatalog } from '../src/example-catalog.mjs';

test('validates all representative request and response examples', async () => {
  const paths = ContractPaths.fromModule(import.meta.url);
  const openapi = await Enforcer(paths.contract, { hideWarnings: true });
  const examples = ExampleCatalog.load(paths.examples);

  new ContractExampleValidator(openapi).validate(examples);

  assert.equal(examples.length, 19);
});

test('rejects an invalid response and reports the example name', async () => {
  const paths = ContractPaths.fromModule(import.meta.url);
  const openapi = await Enforcer(paths.contract, { hideWarnings: true });
  const invalid = [{
    name: 'invalid-session-list',
    operationId: 'listSessions',
    request: { method: 'GET', path: '/api/v1/sessions' },
    response: { status: 200, body: { wrong: [] } }
  }];

  assert.throws(
    () => new ContractExampleValidator(openapi).validate(invalid),
    /invalid-session-list:[\s\S]*required properties missing: sessions/
  );
});

test('rejects duplicate example names', async () => {
  const paths = ContractPaths.fromModule(import.meta.url);
  const openapi = await Enforcer(paths.contract, { hideWarnings: true });
  const example = {
    name: 'duplicate',
    operationId: 'listSessions',
    request: { method: 'GET', path: '/api/v1/sessions' },
    response: { status: 200, body: { sessions: [] } }
  };

  assert.throws(
    () => new ContractExampleValidator(openapi).validate([example, example]),
    /duplicate: duplicate example name/
  );
});

test('rejects malformed example catalog metadata and shape', () => {
  assert.throws(
    () => ExampleCatalog.fromDocument({ version: 2, contract: 'other', examples: [] }),
    /must target bpane-control-v1/
  );
  assert.throws(
    () => ExampleCatalog.fromDocument({ version: 1, contract: 'bpane-control-v1' }),
    /must contain an examples array/
  );
});
