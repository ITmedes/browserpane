import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import {
  describeCiRustBuilder,
  MATERIAL_INPUTS,
  parseArguments,
  parseToolchain,
} from './ci-rust-builder-ref.mjs';

async function fixture() {
  const rootDirectory = await fs.mkdtemp(path.join(os.tmpdir(), 'bpane-ci-builder-'));
  for (const relativePath of MATERIAL_INPUTS) {
    const absolutePath = path.join(rootDirectory, relativePath);
    await fs.mkdir(path.dirname(absolutePath), { recursive: true });
    const content = relativePath === 'rust-toolchain.toml'
      ? '[toolchain]\nchannel = "1.93.1"\n'
      : `${relativePath}\n`;
    await fs.writeFile(absolutePath, content);
  }
  return rootDirectory;
}

test('builder reference is deterministic and carries the pinned toolchain', async (context) => {
  const rootDirectory = await fixture();
  context.after(() => fs.rm(rootDirectory, { recursive: true, force: true }));

  const first = await describeCiRustBuilder({ rootDirectory });
  const second = await describeCiRustBuilder({ rootDirectory });

  assert.deepEqual(second, first);
  assert.equal(first.toolchain, '1.93.1');
  assert.match(first.tag, /^linux-amd64-rust-1\.93\.1-[0-9a-f]{24}$/);
  assert.equal(first.image, `ghcr.io/itmedes/browserpane-ci-rust:${first.tag}`);
  assert.equal(first.digest.length, 64);
});

test('every material input invalidates the builder reference', async (context) => {
  const rootDirectory = await fixture();
  context.after(() => fs.rm(rootDirectory, { recursive: true, force: true }));
  const baseline = await describeCiRustBuilder({ rootDirectory });

  for (const relativePath of MATERIAL_INPUTS) {
    const original = await fs.readFile(path.join(rootDirectory, relativePath));
    await fs.appendFile(path.join(rootDirectory, relativePath), '\n# changed\n');
    const changed = await describeCiRustBuilder({ rootDirectory });
    assert.notEqual(changed.tag, baseline.tag, relativePath);
    await fs.writeFile(path.join(rootDirectory, relativePath), original);
  }
});

test('builder reference rejects ambiguous registry and toolchain inputs', async (context) => {
  const rootDirectory = await fixture();
  context.after(() => fs.rm(rootDirectory, { recursive: true, force: true }));

  await assert.rejects(
    describeCiRustBuilder({ rootDirectory, registry: 'ghcr.io/ITmedes/Builder' }),
    /invalid lowercase container registry/
  );
  assert.throws(() => parseToolchain('channel = "stable"'), /exact numeric channel/);
  assert.throws(() => parseArguments(['--format', 'yaml']), /unsupported format/);
  assert.throws(() => parseArguments(['--registry']), /unknown or incomplete argument/);
});
