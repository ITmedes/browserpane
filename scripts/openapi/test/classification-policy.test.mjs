import assert from 'node:assert/strict';
import test from 'node:test';

import { ClassificationPolicy } from '../src/classification-policy.mjs';

test('rejects an operation assigned to multiple audiences', () => {
  const policy = new ClassificationPolicy({
    version: 1,
    contract: 'bpane-control-v1',
    classifications: {
      'ui-primary': ['getWidget'],
      'api-companion': ['getWidget']
    }
  });

  assert.throws(() => policy.assignments(), /multiple classifications/);
});

test('rejects unknown classifications and malformed policy metadata', () => {
  assert.throws(() => new ClassificationPolicy({
    version: 1,
    contract: 'bpane-control-v1',
    classifications: { typo: ['getWidget'] }
  }).assignments(), /unsupported operation classification/);
  assert.throws(() => new ClassificationPolicy({
    version: 2,
    contract: 'other',
    classifications: {}
  }).assignments(), /must target bpane-control-v1/);
});
