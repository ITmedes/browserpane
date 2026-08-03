import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import { RustCoverageCommand } from './rust-coverage-command.mjs';

class CheckerFixture {
  targets = [];

  check(target) {
    this.targets.push(target);
  }
}

test('runs workspace coverage and checks the resulting baseline', (context) => {
  const rootDirectory = fs.mkdtempSync(path.join(os.tmpdir(), 'bpane-rust-coverage-'));
  context.after(() => fs.rmSync(rootDirectory, { recursive: true, force: true }));
  const invocations = [];
  const checker = new CheckerFixture();
  const command = new RustCoverageCommand(rootDirectory, {
    checker,
    execute: (executable, argumentsList, options) => {
      invocations.push({ executable, argumentsList, options });
      return { status: 0 };
    }
  });

  assert.equal(command.run(), 0);
  assert.equal(fs.existsSync(path.join(rootDirectory, 'target/llvm-cov')), true);
  assert.equal(invocations[0].executable, 'cargo');
  assert.ok(invocations[0].argumentsList.includes('--locked'));
  assert.deepEqual(checker.targets, ['rust-workspace']);
});

test('preserves cargo coverage failures without evaluating the baseline', (context) => {
  const rootDirectory = fs.mkdtempSync(path.join(os.tmpdir(), 'bpane-rust-coverage-'));
  context.after(() => fs.rmSync(rootDirectory, { recursive: true, force: true }));
  const checker = new CheckerFixture();
  const command = new RustCoverageCommand(rootDirectory, {
    checker,
    execute: () => ({ status: 23 })
  });

  assert.equal(command.run(), 23);
  assert.deepEqual(checker.targets, []);
});
