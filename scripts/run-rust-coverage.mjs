#!/usr/bin/env node

import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { RustCoverageCommand } from './coverage/rust-coverage-command.mjs';

const rootDirectory = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

try {
  process.exitCode = new RustCoverageCommand(rootDirectory).run();
} catch (error) {
  console.error(`Rust coverage failed: ${error.message}`);
  process.exitCode = 1;
}
