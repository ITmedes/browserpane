import assert from "node:assert/strict";
import { chmod, mkdtemp, rm, symlink, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { WorkerSecretStore } from "./worker-secret-store.js";

const ALLOWED = ["PRIMARY_TOKEN", "SECONDARY_TOKEN"] as const;

test("loads only allowed environment secrets when no file is configured", async () => {
  const store = new WorkerSecretStore(ALLOWED, {
    PRIMARY_TOKEN: "primary",
    UNRELATED_TOKEN: "unrelated",
  });

  assert.deepEqual(await store.load(), { PRIMARY_TOKEN: "primary" });
});
test("waits for a private file, loads it once, and deletes it", async () => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "bpane-worker-secrets-"));
  const filePath = path.join(directory, "worker.json");
  const store = new WorkerSecretStore(
    ALLOWED,
    { BPANE_WORKER_SECRETS_FILE: filePath, PRIMARY_TOKEN: "environment-value" },
    1_000,
    5,
  );

  const loading = store.load();
  await writeFile(filePath, JSON.stringify({
    PRIMARY_TOKEN: "file-value",
    SECONDARY_TOKEN: "secondary",
  }), { mode: 0o600 });

  assert.deepEqual(await loading, {
    PRIMARY_TOKEN: "file-value",
    SECONDARY_TOKEN: "secondary",
  });
  await assert.rejects(() => store.load(), /worker secrets file is unavailable/u);
  await rm(directory, { recursive: true, force: true });
});

test("rejects broad permissions, symlinks, unknown keys, and malformed payloads", async () => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "bpane-worker-secrets-"));
  const filePath = path.join(directory, "worker.json");
  const store = (target: string) => new WorkerSecretStore(
    ALLOWED,
    { BPANE_WORKER_SECRETS_FILE: target },
    1,
    1,
  );

  await writeFile(filePath, JSON.stringify({ PRIMARY_TOKEN: "do-not-leak" }), { mode: 0o644 });
  await assert.rejects(() => store(filePath).load(), (error: unknown) => {
    assert(error instanceof Error);
    assert(!error.message.includes("do-not-leak"));
    return true;
  });

  await chmod(filePath, 0o600);
  const linkPath = path.join(directory, "worker-link.json");
  await symlink(filePath, linkPath);
  await assert.rejects(() => store(linkPath).load(), /worker secrets file is unavailable/u);

  await writeFile(filePath, JSON.stringify({ UNKNOWN_TOKEN: "secret" }), { mode: 0o600 });
  await assert.rejects(() => store(filePath).load(), /worker secrets file is invalid/u);

  await writeFile(filePath, "{secret", { mode: 0o600 });
  await assert.rejects(() => store(filePath).load(), /worker secrets file is invalid/u);
  await rm(directory, { recursive: true, force: true });
});
