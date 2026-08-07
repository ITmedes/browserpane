import assert from 'node:assert/strict';
import test from 'node:test';

import { ClassificationPolicy } from '../src/classification-policy.mjs';

test('rejects an operation assigned to multiple audiences', () => {
  const policy = new ClassificationPolicy({
    classifications: {
      'ui-primary': ['getWidget'],
      'api-companion': ['getWidget']
    }
  });

  assert.throws(() => policy.assignments(), /multiple classifications/);
});
