#!/usr/bin/env node

import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { ComposeEvidenceFinalizer } from './compose-evidence/compose-evidence-finalizer.mjs';
import { ComposeEvidenceStore } from './compose-evidence/compose-evidence-store.mjs';
import { ComposeEvidenceValidator } from './compose-evidence/compose-evidence-validator.mjs';
import { ComposeLanePlanCatalog } from './compose-evidence/compose-lane-plans.mjs';
import { ComposeStageRunner } from './compose-evidence/compose-stage-runner.mjs';

class ComposeEvidenceCommand {
  #rootDirectory;
  #store;
  #catalog;

  constructor() {
    this.#rootDirectory = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
    this.#store = new ComposeEvidenceStore(
      path.join(this.#rootDirectory, 'test-results/compose-evidence')
    );
    this.#catalog = new ComposeLanePlanCatalog();
  }

  async run(argv) {
    const operation = argv[0];
    const separator = argv.indexOf('--');
    const optionArguments = separator >= 0 ? argv.slice(1, separator) : argv.slice(1);
    const command = separator >= 0 ? argv.slice(separator + 1) : [];
    const options = this.#parseOptions(optionArguments);
    if (operation === 'init') return this.#initialize(options);
    if (operation === 'run') return this.#runStage(options, command);
    if (operation === 'finalize') return this.#finalize(options);
    throw new Error('usage: compose-evidence.mjs <init|run|finalize> --lane <lane>');
  }

  #initialize(options) {
    this.#require(options, 'lane');
    const plan = this.#catalog.plan(options.lane);
    const errors = new ComposeEvidenceValidator().validatePlan(plan);
    if (errors.length > 0) throw new Error(`invalid Compose lane plan: ${errors.join(',')}`);
    this.#store.initialize(plan);
    return 0;
  }

  async #runStage(options, command) {
    this.#require(options, 'lane');
    this.#require(options, 'stage');
    return new ComposeStageRunner(this.#store, this.#rootDirectory)
      .run(options.lane, options.stage, command);
  }

  #finalize(options) {
    this.#require(options, 'lane');
    return new ComposeEvidenceFinalizer(this.#store, this.#rootDirectory)
      .finalize(options.lane);
  }

  #parseOptions(argv) {
    const options = {};
    for (let index = 0; index < argv.length; index += 2) {
      const name = argv[index];
      const value = argv[index + 1];
      if (!name?.startsWith('--') || value === undefined) {
        throw new Error('Compose evidence options must be name/value pairs');
      }
      options[name.slice(2)] = value;
    }
    return options;
  }

  #require(options, name) {
    if (typeof options[name] !== 'string' || options[name].length === 0) {
      throw new Error(`missing --${name}`);
    }
  }
}

try {
  process.exitCode = await new ComposeEvidenceCommand().run(process.argv.slice(2));
} catch (error) {
  console.error(`Compose evidence failed: ${error.message}`);
  process.exitCode = 1;
}
