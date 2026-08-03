import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import { YamlDocumentParser } from './yaml-document-parser.mjs';

test('YAML parser safely parses mappings and aliases', (context) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'bpane-yaml-'));
  context.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const document = path.join(root, 'valid.yml');
  fs.writeFileSync(document, 'base: &base\n  enabled: true\ncopy:\n  <<: *base\n');

  assert.deepEqual(new YamlDocumentParser().parse(document), {
    base: { enabled: true },
    copy: { enabled: true }
  });
});

test('YAML parser rejects malformed and unsafe documents', (context) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'bpane-yaml-'));
  context.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const malformed = path.join(root, 'malformed.yml');
  const unsafe = path.join(root, 'unsafe.yml');
  fs.writeFileSync(malformed, 'key: [unterminated\n');
  fs.writeFileSync(unsafe, 'value: !ruby/object:Object {}\n');
  const parser = new YamlDocumentParser();

  assert.throws(() => parser.parse(malformed), /Psych::SyntaxError|syntax error/i);
  assert.throws(() => parser.parse(unsafe), /Psych::DisallowedClass|class/i);
});
