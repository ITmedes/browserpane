import assert from 'node:assert/strict';
import test from 'node:test';

import { CompatibilityArguments } from '../src/compatibility-arguments.mjs';

test('uses explicit, environment, and default base refs in priority order', () => {
  assert.equal(CompatibilityArguments.parse([], {}).baseRef, 'origin/main');
  assert.equal(CompatibilityArguments.parse([], { BPANE_OPENAPI_BASE_REF: 'release/v1' }).baseRef,
    'release/v1');
  assert.equal(CompatibilityArguments.parse(
    ['--base-ref', 'base-sha'],
    { BPANE_OPENAPI_BASE_REF: 'release/v1' }
  ).baseRef, 'base-sha');
});

test('rejects unknown, empty, option-valued, and duplicate arguments', () => {
  assert.throws(() => CompatibilityArguments.parse(['--unknown'], {}), /unknown argument/);
  assert.throws(() => CompatibilityArguments.parse(['--base-ref'], {}), /non-empty/);
  assert.throws(
    () => CompatibilityArguments.parse(['--base-ref', '--help'], {}),
    /non-empty/
  );
  assert.throws(
    () => CompatibilityArguments.parse(['--base-ref', 'main', '--base-ref', 'release'], {}),
    /only once/
  );
});
