#!/usr/bin/env node

import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { RepositoryDocumentChecker } from './validation/repository-document-checker.mjs';

class RepositoryDocumentCommand {
  #rootDirectory;

  constructor() {
    this.#rootDirectory = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
  }

  run() {
    const files = this.#trackedFiles();
    const markdownFiles = files.filter((value) => value.endsWith('.md'));
    const yamlFiles = files.filter((value) => /\.ya?ml$/u.test(value));
    const workflowFiles = yamlFiles.filter((value) => value.startsWith('.github/workflows/'));
    const errors = new RepositoryDocumentChecker(this.#rootDirectory).check({
      markdownFiles,
      yamlFiles,
      workflowFiles
    });
    if (errors.length > 0) {
      for (const error of errors) console.error(`ERROR ${error}`);
      return 1;
    }
    console.log(`Repository documents passed (${markdownFiles.length} Markdown, `
      + `${yamlFiles.length} YAML, ${workflowFiles.length} workflows).`);
    return 0;
  }

  #trackedFiles() {
    const result = spawnSync('git', ['ls-files', '--cached'], {
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
  process.exitCode = new RepositoryDocumentCommand().run();
} catch (error) {
  console.error(`Repository document validation failed: ${error.message}`);
  process.exitCode = 2;
}
