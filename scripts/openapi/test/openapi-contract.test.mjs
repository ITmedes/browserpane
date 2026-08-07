import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import { OpenApiContract } from '../src/openapi-contract.mjs';

function withContract(content, callback) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'bpane-openapi-'));
  const filename = path.join(directory, 'contract.yaml');
  try {
    fs.writeFileSync(filename, content);
    return callback(filename);
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
}

test('extracts stable operation and inherited security metadata', () => {
  withContract(`
openapi: 3.0.3
security:
  - bearerAuth: []
paths:
  /api/v1/widgets/{widget_id}:
    get:
      tags: [Widgets]
      summary: Get a widget
      operationId: getWidget
      responses:
        '200': { description: Found }
  /api/v1/public:
    post:
      tags: [Widgets]
      summary: Public operation
      operationId: createPublicWidget
      security: []
      responses:
        '202': { description: Accepted }
`, (filename) => {
    const operations = OpenApiContract.load(filename).operations();

    assert.deepEqual(operations.map((operation) => operation.operationId), [
      'createPublicWidget',
      'getWidget'
    ]);
    assert.equal(operations[0].auth, 'unauthenticated');
    assert.equal(operations[1].auth, 'owner-bearer');
    assert.deepEqual(operations[1].responses, ['200']);
  });
});

test('rejects duplicate YAML keys before inventory generation', () => {
  withContract(`
openapi: 3.0.3
paths:
  /api/v1/widgets:
    get:
      operationId: first
      operationId: second
`, (filename) => {
    assert.throws(() => OpenApiContract.load(filename), /Map keys must be unique/);
  });
});
