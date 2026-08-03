#!/usr/bin/env node

import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { RepositoryBaselineChecker } from './validation/repository-baseline.mjs';

class RepositoryBaselineCommand {
  #rootDirectory;

  constructor() {
    this.#rootDirectory = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
  }

  run() {
    const trackedJsonFiles = this.#trackedJsonFiles();
    const errors = new RepositoryBaselineChecker(this.#rootDirectory).check(trackedJsonFiles);
    if (errors.length > 0) {
      for (const error of errors) console.error(`ERROR ${error}`);
      return 1;
    }
    console.log(`Repository baseline passed (${trackedJsonFiles.length} tracked JSON files).`);
    return 0;
  }

  #trackedJsonFiles() {
    const result = spawnSync('git', ['ls-files', '--cached', '--', '**/*.json'], {
      cwd: this.#rootDirectory,
      encoding: 'utf8'
    });
    if (result.error || result.status !== 0) {
      throw new Error(String(result.error?.message ?? result.stderr).trim());
    }
    return result.stdout.split('\n').map((value) => value.trim()).filter(Boolean).sort();
  }
}

try {
  process.exitCode = new RepositoryBaselineCommand().run();
} catch (error) {
  console.error(`Repository baseline failed: ${error.message}`);
  process.exitCode = 2;
}
