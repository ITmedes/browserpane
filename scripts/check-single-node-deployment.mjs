#!/usr/bin/env node

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
    new SingleNodePreflight(this.rootDirectory).run(environment);
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
