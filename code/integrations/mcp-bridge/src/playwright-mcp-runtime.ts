import { existsSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, resolve } from "node:path";
import process from "node:process";

const require = createRequire(import.meta.url);

export type PlaywrightMcpCommand = {
  readonly command: string;
  readonly args: readonly string[];
  readonly executablePath: string;
  readonly packageVersion: string;
};

export type PlaywrightMcpCommandOptions = {
  readonly packageJsonPath?: string;
};

type PlaywrightMcpPackageJson = {
  readonly version?: unknown;
  readonly bin?: unknown;
};

export function resolvePlaywrightMcpCommand(
  cdpEndpoint: string,
  options: PlaywrightMcpCommandOptions = {},
): PlaywrightMcpCommand {
  const packageJsonPath = options.packageJsonPath
    ?? require.resolve("@playwright/mcp/package.json");
  const packageJson = readPackageJson(packageJsonPath);
  const binPath = playwrightMcpBinPath(packageJson);
  const executablePath = resolve(dirname(packageJsonPath), binPath);
  if (!existsSync(executablePath)) {
    throw new Error(
      `local @playwright/mcp executable was not found at ${executablePath}; run npm ci for code/integrations/mcp-bridge or rebuild the mcp-bridge image`,
    );
  }
  return {
    command: process.execPath,
    args: [executablePath, "--cdp-endpoint", cdpEndpoint],
    executablePath,
    packageVersion: packageVersion(packageJson),
  };
}

function readPackageJson(packageJsonPath: string): PlaywrightMcpPackageJson {
  try {
    return JSON.parse(readFileSync(packageJsonPath, "utf8")) as PlaywrightMcpPackageJson;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(`failed to read local @playwright/mcp package metadata at ${packageJsonPath}: ${message}`);
  }
}

function playwrightMcpBinPath(packageJson: PlaywrightMcpPackageJson): string {
  if (typeof packageJson.bin === "string" && packageJson.bin.trim()) {
    return packageJson.bin;
  }
  if (packageJson.bin && typeof packageJson.bin === "object") {
    const bin = packageJson.bin as Readonly<Record<string, unknown>>;
    const executable = bin["playwright-mcp"];
    if (typeof executable === "string" && executable.trim()) {
      return executable;
    }
  }
  throw new Error("local @playwright/mcp package does not expose a playwright-mcp executable");
}

function packageVersion(packageJson: PlaywrightMcpPackageJson): string {
  return typeof packageJson.version === "string" ? packageJson.version : "unknown";
}
