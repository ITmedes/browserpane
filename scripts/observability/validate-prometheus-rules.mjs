#!/usr/bin/env node

import { spawnSync } from 'node:child_process';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

import { PROMETHEUS_IMAGE } from './observability-toolchain.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const EXAMPLE = path.join(ROOT, 'deploy/examples/observability');
const CHECKS = [
  ['check', 'config', '/work/prometheus.yml'],
  ['check', 'rules', '/work/recording-rules.yml', '/work/alert-rules.yml'],
  ['test', 'rules', '/work/rule-tests.yml'],
];

for (const args of CHECKS) runPromtool(args);
console.log(`Prometheus observability rules passed with ${PROMETHEUS_IMAGE}.`);

function runPromtool(args) {
  const result = spawnSync('docker', [
    'run', '--rm',
    '--network', 'none',
    '--read-only',
    '--cap-drop', 'ALL',
    '--security-opt', 'no-new-privileges:true',
    '--tmpfs', '/tmp:rw,noexec,nosuid,size=64m,uid=65534,gid=65534,mode=0700',
    '--entrypoint', '/bin/promtool',
    '--volume', `${EXAMPLE}:/work:ro`,
    PROMETHEUS_IMAGE,
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
