import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

test('standard lint rejects unresolved references and invalid examples', () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'bpane-redocly-'));
  const filename = path.join(directory, 'invalid.yaml');
  try {
    fs.writeFileSync(filename, `
openapi: 3.0.3
info: { title: Invalid, version: 1.0.0 }
tags:
  - name: Widgets
    description: Widget operations.
paths:
  /widgets:
    get:
      tags: [Widgets]
      summary: List widgets
      operationId: listWidgets
      responses:
        '200':
          description: Listed
          content:
            application/json:
              schema: { $ref: '#/components/schemas/Missing' }
              example: invalid
        '400': { description: Invalid request }
`);
    const result = spawnSync(
      path.resolve('node_modules/.bin/redocly'),
      ['lint', '--extends=recommended', filename],
      { cwd: path.resolve('.'), encoding: 'utf8' }
    );

    assert.notEqual(result.status, 0);
    assert.match(`${result.stdout}\n${result.stderr}`, /Missing.*does not exist|Can't resolve/);
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});
