#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

import { RuntimeBrokerBrowserOverlayContract } from "./runtime-broker/runtime-broker-browser-overlay-contract.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const result = spawnSync(
  "docker",
  [
    "compose",
    "-f",
    path.join(root, "deploy", "compose.yml"),
    "-f",
    path.join(root, "deploy", "compose.runtime-broker.yml"),
    "config",
    "--format",
    "json",
  ],
  {
    cwd: root,
    encoding: "utf8",
    env: {
      ...process.env,
      BPANE_GATEWAY_RUNTIME_BACKEND: "broker_pool",
      BPANE_RUNTIME_BROKER_BROWSER_IMAGE: `sha256:${"a".repeat(64)}`,
      BPANE_RUNTIME_BROKER_WORKFLOW_IMAGE: `sha256:${"b".repeat(64)}`,
      BPANE_RUNTIME_BROKER_RECORDING_IMAGE: `sha256:${"c".repeat(64)}`,
    },
  },
);
if (result.error || result.status !== 0) {
  console.error(result.error?.message ?? result.stderr.trim());
  process.exitCode = 1;
} else {
  try {
    new RuntimeBrokerBrowserOverlayContract().validate(JSON.parse(result.stdout));
    console.log("Runtime broker browser and worker overlay compose contract passed.");
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}
