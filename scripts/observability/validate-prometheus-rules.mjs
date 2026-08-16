#!/usr/bin/env node

import { spawnSync } from 'node:child_process';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const EXAMPLE = path.join(ROOT, 'deploy/examples/observability');
const IMAGE = 'prom/prometheus@sha256:69f5241418838263316593f7274a304b095c40bcf22e57272865da91bd60a8ac';
const CHECKS = [
  ['check', 'config', '/work/prometheus.yml'],
  ['check', 'rules', '/work/recording-rules.yml', '/work/alert-rules.yml'],
  ['test', 'rules', '/work/rule-tests.yml'],
];

for (const args of CHECKS) runPromtool(args);
console.log(`Prometheus observability rules passed with ${IMAGE}.`);

function runPromtool(args) {
  const result = spawnSync('docker', [
    'run', '--rm',
    '--entrypoint', '/bin/promtool',
    '--volume', `${EXAMPLE}:/work:ro`,
    IMAGE,
    ...args,
  ], {
    cwd: ROOT,
    encoding: 'utf8',
    stdio: 'inherit',
  });
  if (result.error) {
    throw new Error(`failed to run Docker for promtool: ${result.error.message}`);
  }
  if (result.status !== 0) process.exit(result.status ?? 1);
}
