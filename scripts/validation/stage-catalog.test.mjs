import assert from 'node:assert/strict';
import test from 'node:test';

import { ADMIN_PROMOTION_SMOKES } from './admin-promotion-contract.mjs';
import { ValidationStageCatalog } from './stage-catalog.mjs';
import { ValidationStage } from './validation-stage.mjs';

test('catalog exposes stable, unique fast and compose profiles', () => {
  const catalog = new ValidationStageCatalog('/repo');
  const fast = catalog.forProfile('fast');
  const compose = catalog.forProfile('compose');
  const full = catalog.forProfile('full');

  assert.equal(new Set(full.map((stage) => stage.id)).size, full.length);
  assert.equal(full.length, fast.length + compose.length);
  assert.ok(fast.some((stage) => stage.id === 'dependency-safety'));
  assert.ok(fast.some((stage) => stage.id === 'repository-documents'));
  assert.ok(fast.some((stage) => stage.id === 'production-security-baseline'));
  assert.ok(fast.some((stage) => stage.id === 'rust-coverage'));
  assert.ok(fast.some((stage) => stage.id === 'admin-auth-test'));
  assert.ok(fast.some((stage) => stage.id === 'admin-auth-test-coverage'));
  assert.ok(fast.some((stage) => stage.id === 'admin-new-test'));
  assert.ok(fast.some((stage) => stage.id === 'admin-new-test-coverage'));
  assert.ok(fast.some((stage) => stage.id === 'openapi-install'));
  assert.ok(fast.some((stage) => stage.id === 'openapi-test'));
  assert.ok(fast.some((stage) => stage.id === 'openapi-check'));
  assert.ok(fast.some((stage) => stage.id === 'openapi-compatibility'));
  assert.ok(compose.some((stage) => stage.id === 'compose-gateway-api'));
  assert.ok(compose.some((stage) => stage.id === 'compose-recording'));
  assert.ok(compose.some((stage) => stage.id === 'compose-session-files'));
  assert.ok(compose.some((stage) => stage.id === 'compose-workflow-cli'));
  assert.ok(compose.some((stage) => stage.id === 'compose-workflow-workspace'));
  assert.ok(compose.some((stage) => stage.id === 'compose-workflow-events'));
  for (const smoke of ADMIN_PROMOTION_SMOKES) {
    assert.ok(compose.some((stage) => stage.id === smoke.id));
  }
});

test('promotion surface runner remains part of validation tooling tests', () => {
  const tooling = new ValidationStageCatalog('/repo')
    .forProfile('fast')
    .find((stage) => stage.id === 'validation-tool-tests');

  assert.ok(tooling);
  assert.ok(tooling.args.includes('scripts/validation/admin-promotion-contract.test.mjs'));
  assert.ok(tooling.args.includes('scripts/validation/admin-promotion-runner.test.mjs'));
});

test('catalog selects requested stages in caller order and rejects out-of-profile ids', () => {
  const catalog = new ValidationStageCatalog('/repo');

  const selected = catalog.select('all', ['compose-cli', 'rust-tests']);

  assert.deepEqual(selected.map((stage) => stage.id), ['compose-cli', 'rust-tests']);
  assert.throws(() => catalog.select('fast', ['compose-cli']), /unknown validation stage/);
});

test('validation stages reject unstable ids and invalid timeouts', () => {
  assert.throws(() => new ValidationStage({
    id: 'Bad ID', description: 'invalid', command: 'true', cwd: '/repo', timeoutSeconds: 1
  }), /invalid validation stage id/);
  assert.throws(() => new ValidationStage({
    id: 'valid', description: 'invalid', command: 'true', cwd: '/repo', timeoutSeconds: 0
  }), /invalid timeout/);
});
