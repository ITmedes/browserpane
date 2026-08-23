import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';

import { ComposeFailureClassifier } from '../compose-evidence/compose-failure-classifier.mjs';
import { ComposeEvidenceValidator } from '../compose-evidence/compose-evidence-validator.mjs';
import { ComposeLanePlanCatalog } from '../compose-evidence/compose-lane-plans.mjs';
import { ComposeEvidenceFixture } from './compose-evidence-fixture.mjs';

const root = path.resolve(import.meta.dirname, '../..');

test('checked schema and every fixed lane plan retain the v1 bounded contract', () => {
  const schema = JSON.parse(fs.readFileSync(
    path.join(root, 'quality/compose-stage-evidence-v1.schema.json'), 'utf8'
  ));
  const catalog = new ComposeLanePlanCatalog();
  const validator = new ComposeEvidenceValidator();

  assert.equal(schema.properties.schema_version.const, 1);
  assert.equal(schema.additionalProperties, false);
  assert.equal(schema.properties.stages.maxItems, 32);
  assert.equal(schema.properties.artifacts.maxItems, 12);
  assert.equal(schema.$defs.artifact.properties.bytes.maximum, 5 * 1024 * 1024);
  for (const lane of catalog.lanes()) {
    assert.deepEqual(validator.validatePlan(catalog.plan(lane)), [], lane);
  }
});

test('deterministic fixtures demonstrate every supported failure class', () => {
  const classifier = new ComposeFailureClassifier();
  const base = { exitCode: 1, signal: null, spawnError: false, requestedSignal: null };

  for (const failureClass of ['product', 'harness', 'infrastructure', 'unknown']) {
    assert.deepEqual(classifier.classify({ ...base, defaultClass: failureClass }), {
      outcome: 'failure', failureClass,
    });
  }
  assert.deepEqual(classifier.classify({
    ...base, exitCode: 124, defaultClass: 'product',
  }), { outcome: 'failure', failureClass: 'product' });
  assert.deepEqual(classifier.classify({
    ...base, defaultClass: 'product', spawnError: true,
  }), { outcome: 'failure', failureClass: 'harness' });
  assert.deepEqual(classifier.classify({
    ...base, defaultClass: 'product', requestedSignal: 'SIGTERM',
  }), { outcome: 'cancelled', failureClass: 'infrastructure' });
});

test('unknown failures remain terminal', () => {
  const summary = new ComposeEvidenceFixture().summary({ failureId: 'worker-image' });

  assert.equal(summary.outcome, 'failure');
  assert.equal(summary.primary_failure.stage_id, 'worker-image');
  assert.equal(summary.primary_failure.class, 'unknown');
  assert.deepEqual(new ComposeEvidenceValidator().validateSummary(summary), []);
});

test('reproduction commands reject credential and URL material', () => {
  const plan = new ComposeEvidenceFixture().plan();
  plan.stages[0].reproduction_command = 'curl https://example.invalid?token=value';

  const errors = new ComposeEvidenceValidator().validatePlan(plan);
  assert.ok(errors.includes('plan-reproduction-invalid'));
});
