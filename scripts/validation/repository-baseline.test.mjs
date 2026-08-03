import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import { RepositoryBaselineChecker } from './repository-baseline.mjs';

class RepositoryFixture {
  root;

  constructor(testContext) {
    this.root = fs.mkdtempSync(path.join(os.tmpdir(), 'bpane-baseline-'));
    testContext.after(() => fs.rmSync(this.root, { recursive: true, force: true }));
  }

  write(relativePath, content) {
    const absolutePath = path.join(this.root, relativePath);
    fs.mkdirSync(path.dirname(absolutePath), { recursive: true });
    fs.writeFileSync(absolutePath, content);
  }
}

test('repository baseline accepts aligned paths, commands, package scripts, and JSON', (context) => {
  const fixture = new RepositoryFixture(context);
  fixture.write('README.md', 'Run node scripts/validate.mjs --profile fast');
  fixture.write('pkg/package.json', JSON.stringify({ name: 'fixture', scripts: { test: 'node test' } }));
  fixture.write('pkg/package-lock.json', JSON.stringify({ packages: { '': { name: 'fixture' } } }));
  fixture.write('fixture.json', '{}');
  const checker = new RepositoryBaselineChecker(fixture.root, {
    requiredPaths: ['README.md'],
    packages: [['pkg', ['test']]],
    documentedCommands: [['README.md', 'node scripts/validate.mjs --profile fast']]
  });

  const errors = checker.check([
    'fixture.json',
    'pkg/package.json',
    'pkg/package-lock.json'
  ]);

  assert.deepEqual(errors, []);
});

test('repository baseline reports missing scripts, name drift, docs drift, and invalid JSON', (context) => {
  const fixture = new RepositoryFixture(context);
  fixture.write('README.md', 'No validation command');
  fixture.write('pkg/package.json', JSON.stringify({ name: 'manifest-name', scripts: {} }));
  fixture.write('pkg/package-lock.json', JSON.stringify({ name: 'lock-name' }));
  fixture.write('broken.json', '{');
  const checker = new RepositoryBaselineChecker(fixture.root, {
    requiredPaths: ['README.md', 'ARCH.md'],
    packages: [['pkg', ['test']]],
    documentedCommands: [['README.md', 'node scripts/validate.mjs --profile fast']]
  });

  const errors = checker.check(['broken.json']);

  assert.ok(errors.some((error) => error.includes('required path is missing: ARCH.md')));
  assert.ok(errors.some((error) => error.includes('missing script: test')));
  assert.ok(errors.some((error) => error.includes('root package does not match')));
  assert.ok(errors.some((error) => error.includes('does not document')));
  assert.ok(errors.some((error) => error.includes('invalid JSON in broken.json')));
});
