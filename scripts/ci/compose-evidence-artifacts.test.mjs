import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import { ComposeArtifactCollector } from '../compose-evidence/compose-artifact-collector.mjs';

test('collector retains only bounded explicitly synthetic Playwright evidence', (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'bpane-compose-evidence-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const source = path.join(root, 'test-results/compose-safe-attachments');
  const lane = path.join(root, 'test-results/compose-evidence/browser-integrations');
  fs.mkdirSync(source, { recursive: true });
  fs.mkdirSync(lane, { recursive: true });
  fs.writeFileSync(path.join(source, 'failure.png'), 'synthetic screenshot fixture');
  fs.writeFileSync(path.join(source, 'trace.zip'), 'synthetic trace fixture');
  fs.writeFileSync(path.join(source, 'manifest.json'), JSON.stringify({
    schema_version: 1,
    artifacts: [
      { file: 'failure.png', kind: 'screenshot', content: 'synthetic-fixture' },
      { file: 'trace.zip', kind: 'trace', content: 'synthetic-fixture' },
    ],
  }));

  const result = new ComposeArtifactCollector(root).collect(lane);

  assert.deepEqual(result.errors, []);
  assert.deepEqual(result.artifacts.map((artifact) => artifact.kind), ['screenshot', 'trace']);
  assert.equal(fs.existsSync(path.join(lane, 'attachments/failure.png')), true);
  assert.equal(fs.existsSync(path.join(lane, 'attachments/trace.zip')), true);
});

test('collector rejects traversal, non-synthetic content, symlinks, and oversized diagnostics', (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'bpane-compose-evidence-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const source = path.join(root, 'test-results/compose-safe-attachments');
  const diagnostics = path.join(root, 'test-results/ci-diagnostics');
  const lane = path.join(root, 'test-results/compose-evidence/admin-unified');
  fs.mkdirSync(source, { recursive: true });
  fs.mkdirSync(diagnostics, { recursive: true });
  fs.mkdirSync(lane, { recursive: true });
  fs.writeFileSync(path.join(source, 'real.png'), 'browser content');
  fs.symlinkSync(path.join(source, 'real.png'), path.join(source, 'linked.png'));
  fs.writeFileSync(path.join(diagnostics, 'compose.log'), Buffer.alloc(3 * 1024 * 1024 + 1));
  fs.writeFileSync(path.join(source, 'manifest.json'), JSON.stringify({
    schema_version: 1,
    artifacts: [
      { file: '../escape.png', kind: 'screenshot', content: 'synthetic-fixture' },
      { file: 'real.png', kind: 'screenshot', content: 'browser-content' },
      { file: 'linked.png', kind: 'screenshot', content: 'synthetic-fixture' },
    ],
  }));

  const result = new ComposeArtifactCollector(root).collect(lane);

  assert.deepEqual(result.artifacts, []);
  assert.ok(result.errors.includes('diagnostics-artifact-rejected'));
  assert.equal(result.errors.filter((error) => error === 'attachment-entry-invalid').length, 2);
  assert.ok(result.errors.includes('screenshot-artifact-rejected'));
});
