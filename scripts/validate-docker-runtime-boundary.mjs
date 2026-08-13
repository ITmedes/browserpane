#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

import { DockerRuntimeBoundaryContract } from "./runtime-boundary/docker-runtime-boundary-contract.mjs";
import { DockerRuntimeBoundaryLiveCheck } from "./runtime-boundary/docker-runtime-boundary-live-check.mjs";

const ROOT_DIR = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const DEFAULT_COMPOSE_FILE = path.join(ROOT_DIR, "deploy", "compose.yml");

class CommandRunner {
  run(command, args) {
    const result = spawnSync(command, args, {
      cwd: ROOT_DIR,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    });
    if (result.error) {
      throw new Error(`${command} failed to start: ${result.error.message}`);
    }
    if (result.status !== 0) {
      throw new Error(
        `${command} ${args.join(" ")} failed (${result.status}): ${result.stderr.trim()}`,
      );
    }
    return result.stdout.trim();
  }
}

class BoundaryValidatorCli {
  constructor(commandRunner) {
    this.commandRunner = commandRunner;
  }

  run(argv) {
    const { composeFile, staticOnly } = this.parseArguments(argv);
    const config = JSON.parse(
      this.commandRunner.run("docker", [
        "compose",
        "-f",
        composeFile,
        "config",
        "--format",
        "json",
      ]),
    );
    new DockerRuntimeBoundaryContract().validate(config);
    console.log("Docker runtime boundary compose contract passed.");
    if (staticOnly) {
      return;
    }

    const request = (method, endpoint) =>
      this.commandRunner.run("docker", [
        "compose",
        "-f",
        composeFile,
        "exec",
        "-T",
        "gateway",
        "curl",
        "--max-time",
        "5",
        "--silent",
        "--show-error",
        "--output",
        "/dev/null",
        "--write-out",
        "%{http_code}",
        "--request",
        method,
        `http://docker-proxy:2375${endpoint}`,
      ]);
    new DockerRuntimeBoundaryLiveCheck(request).validate();
    console.log("Docker runtime boundary live API allow/deny checks passed.");
  }

  parseArguments(argv) {
    let composeFile = DEFAULT_COMPOSE_FILE;
    let staticOnly = false;
    for (let index = 0; index < argv.length; index += 1) {
      const argument = argv[index];
      if (argument === "--static-only") {
        staticOnly = true;
      } else if (argument === "--compose-file") {
        const value = argv[index + 1];
        if (!value) {
          throw new Error("--compose-file requires a value");
        }
        composeFile = path.resolve(value);
        index += 1;
      } else {
        throw new Error(`unknown argument: ${argument}`);
      }
    }
    return { composeFile, staticOnly };
  }
}

try {
  new BoundaryValidatorCli(new CommandRunner()).run(process.argv.slice(2));
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
}
