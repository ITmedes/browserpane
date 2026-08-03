import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import { MarkdownLinkChecker } from './markdown-link-checker.mjs';

test('Markdown checker accepts existing, remote, anchor, and fenced-code links', (context) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'bpane-markdown-'));
  context.after(() => fs.rmSync(root, { recursive: true, force: true }));
  fs.mkdirSync(path.join(root, 'docs'));
  fs.writeFileSync(path.join(root, 'target file.md'), '# Target');
  fs.writeFileSync(path.join(root, 'docs', 'source.md'), [
    '[target](<../target%20file.md#target>)',
    '[remote](https://example.com)',
    '[anchor](#section)',
    '```md',
    '[fixture](missing.md)',
    '```'
  ].join('\n'));

  assert.deepEqual(new MarkdownLinkChecker(root).check('docs/source.md'), []);
});

test('Markdown checker reports missing inline and HTML targets', (context) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'bpane-markdown-'));
  context.after(() => fs.rmSync(root, { recursive: true, force: true }));
  fs.writeFileSync(path.join(root, 'source.md'), '[missing](docs/no.md)\n<img src="no.png">');

  const errors = new MarkdownLinkChecker(root).check('source.md');

  assert.equal(errors.length, 2);
  assert.ok(errors.every((error) => error.includes('missing local path')));
});
