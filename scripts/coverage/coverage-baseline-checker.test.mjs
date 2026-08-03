import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import { CoverageBaselineChecker } from './coverage-baseline-checker.mjs';

class CoverageFixture {
  rootDirectory;

  constructor(context, targets) {
    this.rootDirectory = fs.mkdtempSync(path.join(os.tmpdir(), 'bpane-coverage-'));
    context.after(() => fs.rmSync(this.rootDirectory, { recursive: true, force: true }));
    this.writeJson('quality/coverage-baselines.json', { version: 1, targets });
  }

  writeJson(relativePath, value) {
    const absolutePath = path.join(this.rootDirectory, relativePath);
    fs.mkdirSync(path.dirname(absolutePath), { recursive: true });
    fs.writeFileSync(absolutePath, JSON.stringify(value));
  }

  read(relativePath) {
    return fs.readFileSync(path.join(this.rootDirectory, relativePath), 'utf8');
  }
}

function target(format, report, minimums) {
  return { label: 'Fixture target', format, report, minimums };
}

test('accepts Istanbul metrics at or above the baseline and writes a summary', (context) => {
  const fixture = new CoverageFixture(context, {
    web: target('istanbul-summary', 'coverage/web.json', { lines: 80, branches: 70 })
  });
  fixture.writeJson('coverage/web.json', {
    total: { lines: { pct: 80 }, branches: { pct: 72 } }
  });

  const result = new CoverageBaselineChecker(fixture.rootDirectory).check('web');

  assert.equal(result.passed, true);
  assert.match(fixture.read('test-results/coverage/web.md'), /Result: \*\*PASS\*\*/);
  assert.match(fixture.read('test-results/coverage/web.md'), /\| lines \| 80\.00% \| 80\.00% \| PASS \|/);
});

test('rejects a regressed metric after recording a failed summary', (context) => {
  const fixture = new CoverageFixture(context, {
    web: target('istanbul-summary', 'coverage/web.json', { functions: 90 })
  });
  fixture.writeJson('coverage/web.json', { total: { functions: { pct: 89.99 } } });
  const checker = new CoverageBaselineChecker(fixture.rootDirectory);

  assert.throws(() => checker.check('web'), /functions 89\.99% < 90\.00%/);
  assert.match(fixture.read('test-results/coverage/web.md'), /Result: \*\*FAIL\*\*/);
});

test('reads LLVM totals and supports an explicit summary path', (context) => {
  const fixture = new CoverageFixture(context, {
    rust: target('llvm-summary', 'coverage/rust.json', { lines: 56.2 })
  });
  fixture.writeJson('coverage/rust.json', {
    data: [{ totals: { lines: { percent: 56.25 } } }]
  });

  const result = new CoverageBaselineChecker(fixture.rootDirectory).check('rust', {
    outputPath: 'artifacts/rust.md'
  });

  assert.equal(result.comparisons[0].actual, 56.25);
  assert.match(fixture.read('artifacts/rust.md'), /56\.25%/);
});

test('rejects unknown targets, unsupported formats, and malformed reports', (context) => {
  const fixture = new CoverageFixture(context, {
    unknown: target('custom', 'coverage/custom.json', { lines: 1 }),
    broken: target('istanbul-summary', 'coverage/broken.json', { lines: 1 })
  });
  fixture.writeJson('coverage/custom.json', {});
  fixture.writeJson('coverage/broken.json', {});
  const checker = new CoverageBaselineChecker(fixture.rootDirectory);

  assert.throws(() => checker.check('missing'), /unknown coverage target/);
  assert.throws(() => checker.check('unknown'), /unsupported coverage report format/);
  assert.throws(() => checker.check('broken'), /has no total/);
});
