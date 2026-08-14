#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  RuntimeTracingFixtureContract,
  runtimeTracingCollectorImage,
} from "./runtime-tracing/runtime-tracing-fixture-contract.mjs";
import {
  SingleNodePreflight,
  SingleNodeRepositoryFixture,
} from "./single-node/single-node-preflight.mjs";

const rootDirectory = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const environment = new SingleNodeRepositoryFixture().createEnvironment();

try {
  new SingleNodePreflight(rootDirectory).run(environment);
  const compose = spawnSync(
    "docker",
    [
      "compose",
      "-f",
      "deploy/single-node/compose.yml",
      "-f",
      "deploy/single-node/fixture/compose.yml",
      "config",
      "--format",
      "json",
    ],
    {
      cwd: rootDirectory,
      encoding: "utf8",
      env: { ...process.env, ...environment },
      maxBuffer: 16 * 1024 * 1024,
    },
  );
  if (compose.error || compose.status !== 0) {
    throw new Error(`runtime tracing Compose render failed: ${compose.error?.message ?? compose.stderr.trim()}`);
  }
  new RuntimeTracingFixtureContract().validate(JSON.parse(compose.stdout));

  const collectorConfig = path.join(
    rootDirectory,
    "deploy/single-node/fixture/otel-collector.yaml",
  );
  const collector = spawnSync(
    "docker",
    [
      "run",
      "--rm",
      "--read-only",
      "--cap-drop=ALL",
      "--security-opt=no-new-privileges:true",
      "--entrypoint=/otelcol-contrib",
      "-v",
      `${collectorConfig}:/etc/otelcol-contrib/config.yaml:ro`,
      runtimeTracingCollectorImage,
      "validate",
      "--config=/etc/otelcol-contrib/config.yaml",
    ],
    { cwd: rootDirectory, encoding: "utf8", maxBuffer: 16 * 1024 * 1024 },
  );
  if (collector.error || collector.status !== 0) {
    throw new Error(`collector configuration validation failed: ${collector.error?.message ?? collector.stderr.trim()}`);
  }
  console.log("Runtime tracing fixture contract passed.");
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
}
