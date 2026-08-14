#!/usr/bin/env node

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { EnvironmentFileParser } from "./single-node/env-file-parser.mjs";
import { SingleNodePreflight, SingleNodeRepositoryFixture } from "./single-node/single-node-preflight.mjs";

class SingleNodePreflightCommand {
  constructor(rootDirectory) {
    this.rootDirectory = rootDirectory;
  }

  run(args) {
    const environment = this.environment(args);
    const outputDirectory = args[0] === "--repository-fixture"
      ? fs.mkdtempSync(path.join(os.tmpdir(), "bpane-single-node-render-"))
      : path.join(this.rootDirectory, "deploy/single-node/generated");
    new SingleNodePreflight(this.rootDirectory, outputDirectory).run(environment);
    console.log("Single-node deployment preflight passed.");
  }

  environment(args) {
    if (args.length === 1 && args[0] === "--repository-fixture") {
      return new SingleNodeRepositoryFixture().createEnvironment();
    }
    if (args.length === 2 && args[0] === "--env-file") {
      return new EnvironmentFileParser().parse(path.resolve(args[1]));
    }
    throw new Error("usage: check-single-node-deployment.mjs --repository-fixture | --env-file FILE");
  }
}

const rootDirectory = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
try {
  new SingleNodePreflightCommand(rootDirectory).run(process.argv.slice(2));
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
}
