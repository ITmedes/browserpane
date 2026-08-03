import assert from 'node:assert/strict';
import test from 'node:test';

import { ValidationArguments } from './arguments.mjs';

test('arguments default to the fast profile', () => {
  const result = ValidationArguments.parse([]);

  assert.equal(result.selectionProfile, 'fast');
  assert.deepEqual(result.requestedStages, []);
});

test('a standalone stage selection searches the full catalog', () => {
  const result = ValidationArguments.parse(['--stage', 'compose-cli', '--dry-run']);

  assert.equal(result.selectionProfile, 'all');
  assert.deepEqual(result.requestedStages, ['compose-cli']);
  assert.equal(result.dryRun, true);
});

test('an explicit profile constrains stage selection', () => {
  const result = ValidationArguments.parse([
    '--profile', 'compose',
    '--stage', 'compose-cli',
    '--list'
  ]);

  assert.equal(result.selectionProfile, 'compose');
  assert.equal(result.list, true);
});

test('arguments reject unknown profiles, options, duplicates, and missing values', () => {
  assert.throws(() => ValidationArguments.parse(['--profile', 'slow']), /unknown validation profile/);
  assert.throws(() => ValidationArguments.parse(['--unknown']), /unknown argument/);
  assert.throws(() => ValidationArguments.parse(['--stage']), /requires a value/);
  assert.throws(
    () => ValidationArguments.parse(['--stage', 'rust-tests', '--stage', 'rust-tests']),
    /must not be repeated/
  );
});
