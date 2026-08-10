#!/usr/bin/env node

import { spawn } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  ADMIN_PROMOTION_SMOKES,
  COMPATIBILITY_ADMIN_PROMOTION_SMOKES,
  UNIFIED_ADMIN_PROMOTION_SMOKES,
} from './validation/admin-promotion-contract.mjs';

export function selectPromotionSmokes(selectedSurface) {
  if (selectedSurface === 'all') return ADMIN_PROMOTION_SMOKES;
  if (selectedSurface === 'unified') return UNIFIED_ADMIN_PROMOTION_SMOKES;
  if (selectedSurface === 'compatibility') return COMPATIBILITY_ADMIN_PROMOTION_SMOKES;
  throw new Error(
    `unknown admin promotion surface: ${selectedSurface}; expected all, unified, or compatibility`,
  );
}

if (isMainModule()) {
  const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
  const surface = process.argv[2] ?? 'all';
  const selected = selectPromotionSmokes(surface);
  const args = [
    path.join(root, 'scripts/validate.mjs'),
    ...selected.flatMap(({ id }) => ['--stage', id]),
  ];
  const child = spawn(process.execPath, args, { cwd: root, stdio: 'inherit' });

  for (const signal of ['SIGINT', 'SIGTERM']) {
    process.once(signal, () => child.kill(signal));
  }

  child.once('error', (error) => {
    console.error(`Admin promotion validation failed to start: ${error.message}`);
    process.exitCode = 2;
  });
  child.once('exit', (code, signal) => {
    process.exitCode = code ?? signalExitCode(signal);
  });
}

function isMainModule() {
  return process.argv[1] !== undefined
    && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
}

function signalExitCode(signal) {
  if (signal === 'SIGINT') return 130;
  if (signal === 'SIGTERM') return 143;
  return 1;
}
