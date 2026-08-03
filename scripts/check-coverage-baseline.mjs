#!/usr/bin/env node

import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { CoverageBaselineChecker } from './coverage/coverage-baseline-checker.mjs';

class CoverageBaselineCommand {
  run(argumentsList) {
    const targetIndex = argumentsList.indexOf('--target');
    if (targetIndex < 0 || !argumentsList[targetIndex + 1] || argumentsList.length !== 2) {
      throw new Error('usage: node scripts/check-coverage-baseline.mjs --target <target>');
    }
    const rootDirectory = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
    const result = new CoverageBaselineChecker(rootDirectory).check(argumentsList[targetIndex + 1]);
    console.log(`Coverage baseline passed for ${result.targetId}.`);
  }
}

try {
  new CoverageBaselineCommand().run(process.argv.slice(2));
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
