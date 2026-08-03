#!/usr/bin/env node

import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { ValidationArguments } from './validation/arguments.mjs';
import { ValidationStageCatalog } from './validation/stage-catalog.mjs';
import { SubprocessStageExecutor } from './validation/subprocess-executor.mjs';
import { ValidationRunner } from './validation/validation-runner.mjs';

class ValidationCommand {
  #rootDirectory;

  constructor() {
    this.#rootDirectory = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
  }

  async run(rawArguments) {
    const options = ValidationArguments.parse(rawArguments);
    if (options.help) {
      this.#printHelp();
      return 0;
    }

    const catalog = new ValidationStageCatalog(this.#rootDirectory);
    const stages = catalog.select(options.selectionProfile, options.requestedStages);
    if (options.list) {
      for (const stage of stages) console.log(`${stage.id}\t${stage.description}`);
      return 0;
    }

    const runner = new ValidationRunner(new SubprocessStageExecutor());
    const signals = ['SIGINT', 'SIGTERM'];
    const handlers = new Map(signals.map((signal) => [signal, () => runner.cancel(signal)]));
    for (const [signal, handler] of handlers) process.once(signal, handler);
    try {
      return await runner.run(stages, { dryRun: options.dryRun });
    } finally {
      for (const [signal, handler] of handlers) process.removeListener(signal, handler);
    }
  }

  #printHelp() {
    console.log(`Usage: node scripts/validate.mjs [options]

Profiles:
  fast       dependency, metadata, Rust, maintained Node, and script checks
  compose    bounded compose API/admin/CLI/MCP/recording/workflow smokes
  full       fast plus compose

Options:
  --profile <fast|compose|full>  select a profile (default: fast)
  --stage <id>                   run one stage; may be repeated
  --list                         list the selected stages
  --dry-run                      print commands without running them
  --help                         show this help

Use --stage without --profile to select from every known stage. Execution stops
on the first failure and preserves its exit code. Compose profiles may build or
start the local stack and intentionally leave it running for inspection.`);
  }
}

try {
  process.exitCode = await new ValidationCommand().run(process.argv.slice(2));
} catch (error) {
  console.error(`Validation failed: ${error.message}`);
  process.exitCode = 2;
}
