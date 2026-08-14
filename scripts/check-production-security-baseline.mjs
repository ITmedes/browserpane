#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { ProductionSecurityBaselineContract } from "./security/production-security-baseline-contract.mjs";

class ProductionSecurityBaselineCommand {
  constructor(rootDirectory) {
    this.rootDirectory = rootDirectory;
  }

  run() {
    new ProductionSecurityBaselineContract().validate({
      composeConfig: this.composeConfig(),
      localComposeSource: this.read("deploy/compose.yml"),
      adminHeaderConfig: this.read("deploy/nginx-admin-security-headers.conf"),
      threatModel: this.read("docs/THREAT_MODEL.md"),
      hardeningBaseline: this.read("docs/PRODUCTION_SECURITY_BASELINE.md"),
    });
    console.log("Production security baseline contract passed.");
  }

  composeConfig() {
    const result = spawnSync(
      "docker",
      [
        "compose",
        "-f",
        "deploy/compose.yml",
        "-f",
        "deploy/compose.runtime-broker.yml",
        "config",
        "--format",
        "json",
      ],
      {
        cwd: this.rootDirectory,
        encoding: "utf8",
        env: {
          ...process.env,
          BPANE_GATEWAY_RUNTIME_BACKEND: "broker_pool",
          BPANE_RUNTIME_BROKER_BROWSER_IMAGE: `sha256:${"a".repeat(64)}`,
          BPANE_RUNTIME_BROKER_STORAGE_HELPER_IMAGE: `sha256:${"a".repeat(64)}`,
          BPANE_RUNTIME_BROKER_WORKFLOW_IMAGE: `sha256:${"b".repeat(64)}`,
          BPANE_RUNTIME_BROKER_RECORDING_IMAGE: `sha256:${"c".repeat(64)}`,
        },
      },
    );
    if (result.error || result.status !== 0) {
      throw new Error(result.error?.message ?? result.stderr.trim());
    }
    return JSON.parse(result.stdout);
  }

  read(relativePath) {
    return readFileSync(path.join(this.rootDirectory, relativePath), "utf8");
  }
}

const rootDirectory = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
try {
  new ProductionSecurityBaselineCommand(rootDirectory).run();
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
}
