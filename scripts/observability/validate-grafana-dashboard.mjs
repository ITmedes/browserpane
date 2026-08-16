#!/usr/bin/env node

import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

import { DockerCommandRunner } from './docker-command-runner.mjs';
import { GrafanaApiProbe } from './grafana-api-probe.mjs';
import { GrafanaDashboardContract } from './grafana-dashboard-contract.mjs';
import { GrafanaValidationStack } from './grafana-validation-stack.mjs';
import { GRAFANA_IMAGE } from './observability-toolchain.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const errors = new GrafanaDashboardContract(root).check();
if (errors.length > 0) {
  for (const error of errors) console.error(`- ${error}`);
  process.exit(1);
}

const runner = new DockerCommandRunner();
const stack = new GrafanaValidationStack(root, runner);
let stopping = false;

for (const [signal, exitCode] of [['SIGINT', 130], ['SIGTERM', 143]]) {
  process.once(signal, () => {
    if (stopping) return;
    stopping = true;
    stack.stop();
    process.exit(exitCode);
  });
}

try {
  const containers = stack.start();
  const evidence = new GrafanaApiProbe(runner).run(
    containers.grafana, containers.prometheus,
  );
  console.log(
    `Grafana dashboard passed with ${evidence.panels} panels, `
      + `${evidence.queries} live queries, and ${GRAFANA_IMAGE}.`,
  );
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
} finally {
  stopping = true;
  stack.stop();
}
