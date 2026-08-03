#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { ComposeDiagnosticsCollector } from './ci/compose-diagnostics-collector.mjs';
import { DiagnosticRedactor } from './ci/diagnostic-redactor.mjs';
import { SubprocessDiagnosticExecutor } from './ci/subprocess-diagnostic-executor.mjs';

class ComposeDiagnosticsCommand {
  #rootDirectory;

  constructor() {
    this.#rootDirectory = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
  }

  run() {
    const collector = new ComposeDiagnosticsCollector(
      new SubprocessDiagnosticExecutor(),
      new DiagnosticRedactor()
    );
    const outputDirectory = path.join(this.#rootDirectory, 'test-results/ci-diagnostics');
    const outputPath = path.join(outputDirectory, 'compose.log');
    fs.mkdirSync(outputDirectory, { recursive: true, mode: 0o700 });
    fs.writeFileSync(outputPath, collector.collect(this.#rootDirectory), { mode: 0o600 });
    console.log(`Wrote redacted compose diagnostics to ${outputPath}`);
    return 0;
  }
}

try {
  process.exitCode = new ComposeDiagnosticsCommand().run();
} catch (error) {
  console.error(`Compose diagnostics failed without emitting raw logs: ${error.message}`);
  process.exitCode = 1;
}
