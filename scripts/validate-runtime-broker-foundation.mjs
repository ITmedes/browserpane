#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

import { RuntimeBrokerFoundationContract } from "./runtime-broker/runtime-broker-foundation-contract.mjs";

const ROOT_DIR = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const composeFile = path.join(ROOT_DIR, "deploy", "compose.yml");
const result = spawnSync(
  "docker",
  ["compose", "-f", composeFile, "config", "--format", "json"],
  { cwd: ROOT_DIR, encoding: "utf8" },
);
if (result.error || result.status !== 0) {
  console.error(result.error?.message ?? result.stderr.trim());
  process.exitCode = 1;
} else {
  try {
    new RuntimeBrokerFoundationContract().validate(JSON.parse(result.stdout));
    console.log("Runtime broker foundation compose contract passed.");
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}
