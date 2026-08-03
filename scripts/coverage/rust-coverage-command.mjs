import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

import { CoverageBaselineChecker } from './coverage-baseline-checker.mjs';

export class RustCoverageCommand {
  #rootDirectory;
  #execute;
  #checker;

  constructor(rootDirectory, options = {}) {
    this.#rootDirectory = rootDirectory;
    this.#execute = options.execute ?? spawnSync;
    this.#checker = options.checker ?? new CoverageBaselineChecker(rootDirectory);
  }

  run() {
    fs.mkdirSync(path.join(this.#rootDirectory, 'target/llvm-cov'), { recursive: true });
    const result = this.#execute('cargo', [
      'llvm-cov',
      '--workspace',
      '--locked',
      '--summary-only',
      '--json',
      '--output-path',
      'target/llvm-cov/coverage-summary.json'
    ], { cwd: this.#rootDirectory, stdio: 'inherit' });
    if (result.error) throw result.error;
    if (result.status !== 0) return result.status ?? 1;
    this.#checker.check('rust-workspace');
    return 0;
  }
}
