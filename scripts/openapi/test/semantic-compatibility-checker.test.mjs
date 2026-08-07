import assert from 'node:assert/strict';
import test from 'node:test';

import { SemanticCompatibilityChecker } from '../src/semantic-compatibility-checker.mjs';

const BASE = `
openapi: 3.0.3
info: { title: Test, version: 1.0.0 }
paths:
  /widgets:
    get:
      operationId: listWidgets
      responses:
        '200': { description: Listed }
`;

test('allows additive operations through the standard diff engine', async () => {
  const revision = `${BASE}
  /health:
    get:
      operationId: getHealth
      responses:
        '200': { description: Healthy }
`;

  const result = await new SemanticCompatibilityChecker().compare(BASE, revision);

  assert.ok(result.nonBreakingChanges > 0);
});

test('rejects removed operations through the standard diff engine', async () => {
  const revision = `
openapi: 3.0.3
info: { title: Test, version: 1.0.0 }
paths: {}
`;

  await assert.rejects(
    new SemanticCompatibilityChecker().compare(BASE, revision),
    /breaking OpenAPI changes detected:[\s\S]*path.remove/
  );
});
