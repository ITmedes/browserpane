import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import { InventoryFile } from '../src/inventory-file.mjs';

test('writes deterministic inventory and detects stale output', () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'bpane-inventory-'));
  const filename = path.join(directory, 'inventory.json');
  const inventory = { version: 1, operations: [] };
  try {
    InventoryFile.write(filename, inventory);
    InventoryFile.check(filename, inventory);
    fs.writeFileSync(filename, '{}\n');
    assert.throws(() => InventoryFile.check(filename, inventory), /inventory is stale/);
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});
