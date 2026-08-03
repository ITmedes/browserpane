import assert from 'node:assert/strict';
import test from 'node:test';

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
  assert.ok(fast.some((stage) => stage.id === 'rust-coverage'));
  assert.ok(fast.some((stage) => stage.id === 'admin-new-test'));
  assert.ok(fast.some((stage) => stage.id === 'admin-new-test-coverage'));
  assert.ok(compose.some((stage) => stage.id === 'compose-gateway-api'));
  assert.ok(compose.some((stage) => stage.id === 'compose-recording'));
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
