import assert from 'node:assert/strict';
import test from 'node:test';

import { DependencyExceptionPolicy } from './policy.mjs';

const FINDING = {
  ecosystem: 'cargo',
  manifest: 'Cargo.lock',
  package: 'rsa',
  advisory: 'RUSTSEC-2023-0071',
  severity: 'vulnerability',
  title: 'Fixture'
};

const EXCEPTION = {
  ecosystem: FINDING.ecosystem,
  manifest: FINDING.manifest,
  package: FINDING.package,
  advisory: FINDING.advisory,
  dependencyPath: 'sqlx -> sqlx-mysql -> rsa',
  runtimeReachability: 'Feature is disabled.',
  compensatingControl: 'Keep the feature disabled.',
  reason: 'No patched version exists.',
  owner: 'security-owner',
  expiresOn: '2026-09-01'
};

test('an exact active exception approves a finding', () => {
  const policy = new DependencyExceptionPolicy(
    { version: 1, exceptions: [EXCEPTION] },
    new Date('2026-08-03T10:00:00Z')
  );

  const result = policy.evaluate([FINDING]);

  assert.equal(result.passed, true);
  assert.equal(result.approved.length, 1);
  assert.equal(result.blocked.length, 0);
});

test('an unapproved finding blocks the policy', () => {
  const policy = new DependencyExceptionPolicy(
    { version: 1, exceptions: [] },
    new Date('2026-08-03T10:00:00Z')
  );

  const result = policy.evaluate([FINDING]);

  assert.equal(result.passed, false);
  assert.match(result.blocked[0].reason, /no approved exception/);
});

test('expired and stale exceptions block the policy', () => {
  const policy = new DependencyExceptionPolicy(
    { version: 1, exceptions: [{ ...EXCEPTION, expiresOn: '2026-08-02' }] },
    new Date('2026-08-03T10:00:00Z')
  );

  const result = policy.evaluate([]);

  assert.equal(result.passed, false);
  assert.equal(result.expired.length, 1);
  assert.equal(result.stale.length, 1);
});

test('invalid dates, fields, and duplicate exception keys are rejected', () => {
  assert.throws(
    () => new DependencyExceptionPolicy({
      version: 1,
      exceptions: [
        { ...EXCEPTION, expiresOn: '2026-02-30', unexpected: 'value' },
        EXCEPTION
      ]
    }),
    /invalid dependency exceptions:[\s\S]*unsupported field:[\s\S]*valid YYYY-MM-DD[\s\S]*duplicates/
  );
  assert.throws(
    () => new DependencyExceptionPolicy({
      version: 1,
      exceptions: [{ ...EXCEPTION, expiresOn: '2026-99-99' }]
    }),
    /valid YYYY-MM-DD date/
  );
});

test('absolute and non-lockfile manifest paths are rejected', () => {
  assert.throws(
    () => new DependencyExceptionPolicy({
      version: 1,
      exceptions: [{ ...EXCEPTION, manifest: '/tmp/Cargo.lock' }]
    }),
    /repository-relative lockfile path/
  );
  assert.throws(
    () => new DependencyExceptionPolicy({
      version: 1,
      exceptions: [{ ...EXCEPTION, manifest: 'C:\\temp\\package-lock.json' }]
    }),
    /repository-relative lockfile path/
  );
  assert.throws(
    () => new DependencyExceptionPolicy({
      version: 1,
      exceptions: [{ ...EXCEPTION, manifest: 'package.json' }]
    }),
    /repository-relative lockfile path/
  );
});
