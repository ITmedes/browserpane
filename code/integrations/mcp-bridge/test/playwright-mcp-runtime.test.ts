import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import process from "node:process";
import test from "node:test";
import { resolvePlaywrightMcpCommand } from "../src/playwright-mcp-runtime.js";

test("runs the installed local @playwright/mcp CLI without npx or @latest", () => {
  const tempDir = mkdtempSync(join(tmpdir(), "bpane-mcp-runtime-"));
  try {
    const packageJsonPath = join(tempDir, "package.json");
    const cliPath = join(tempDir, "cli.js");
    writeFileSync(packageJsonPath, JSON.stringify({
      version: "0.0.68",
      bin: { "playwright-mcp": "cli.js" },
    }));
    writeFileSync(cliPath, "#!/usr/bin/env node\n");

    const command = resolvePlaywrightMcpCommand("ws://runtime:9222", { packageJsonPath });

    assert.equal(command.command, process.execPath);
    assert.equal(command.executablePath, cliPath);
    assert.deepEqual(command.args, [cliPath, "--cdp-endpoint", "ws://runtime:9222"]);
    assert.equal(command.packageVersion, "0.0.68");
    assert.equal(command.args.some((arg) => arg.includes("@latest")), false);
    assert.equal(command.args.some((arg) => arg === "npx"), false);
  } finally {
    rmSync(tempDir, { recursive: true, force: true });
  }
});

test("fails clearly when the local @playwright/mcp executable is missing", () => {
  const tempDir = mkdtempSync(join(tmpdir(), "bpane-mcp-runtime-missing-"));
  try {
    mkdirSync(join(tempDir, "bin"));
    const packageJsonPath = join(tempDir, "package.json");
    writeFileSync(packageJsonPath, JSON.stringify({
      version: "0.0.68",
      bin: { "playwright-mcp": "bin/playwright-mcp.js" },
    }));

    assert.throws(
      () => resolvePlaywrightMcpCommand("ws://runtime:9222", { packageJsonPath }),
      /local @playwright\/mcp executable was not found/,
    );
  } finally {
    rmSync(tempDir, { recursive: true, force: true });
  }
});
