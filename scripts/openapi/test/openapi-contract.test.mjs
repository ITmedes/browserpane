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

test('classifies recording-worker capabilities separately from owner access', () => {
  withContract(`
openapi: 3.0.3
paths:
  /api/v1/sessions/{session_id}/recordings/{recording_id}/complete:
    post:
      tags: [Session Recordings]
      operationId: completeSessionRecording
      security:
        - recordingWorkerAccessToken: []
      responses:
        '200': { description: Finalized }
`, (filename) => {
    const [operation] = OpenApiContract.load(filename).operations();
    assert.equal(operation.auth, 'recording-worker');
  });
});

test('classifies confidential machine callers separately from owner access', () => {
  withContract(`
openapi: 3.0.3
paths:
  /api/v1/projects/{project_id}/workflow-endpoints/{endpoint_key}/invocations:
    post:
      tags: [Workflow Endpoints]
      operationId: invokeWorkflowEndpoint
      security:
        - machineBearer: []
      responses:
        '202': { description: Accepted }
`, (filename) => {
    const [operation] = OpenApiContract.load(filename).operations();
    assert.equal(operation.auth, 'machine-bearer');
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

test('rejects external references before a contract tool can fetch them', () => {
  assert.throws(() => OpenApiContract.parse(`
openapi: 3.0.3
paths: {}
components:
  schemas:
    Widget:
      $ref: https://contracts.example/widget.yaml
`), /external OpenAPI reference/);
});
