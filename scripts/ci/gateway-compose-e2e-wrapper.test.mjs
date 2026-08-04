import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import test from 'node:test';

const root = path.resolve(import.meta.dirname, '../..');
const wrapper = path.join(root, 'scripts/run-gateway-compose-e2e.sh');

function run(...args) {
  return spawnSync('bash', [wrapper, ...args], {
    cwd: root,
    encoding: 'utf8'
  });
}

test('gateway compose wrapper has valid shell syntax', () => {
  const result = spawnSync('bash', ['-n', wrapper], { cwd: root, encoding: 'utf8' });

  assert.equal(result.status, 0, result.stderr);
});

test('gateway compose wrapper documents stack-only preparation', () => {
  const result = run('--help');

  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /default\|docker-pool\|all\|stack/);
  assert.match(result.stdout, /stack prepares the runtime/);
  assert.match(result.stdout, /session-control parity against the compose Postgres database/);
});

test('gateway compose wrapper runs the persisted session-store contract', () => {
  const source = fs.readFileSync(wrapper, 'utf8');

  assert.match(source, /BPANE_SESSION_STORE_CONTRACT_POSTGRES_URL/);
  assert.match(source, /session_store_contract_postgres/);
  assert.match(source, /if \[\[ "\$SUITE" != "stack" \]\]/);
});

test('gateway compose wrapper rejects missing and unknown suite values', () => {
  const missing = run('--suite');
  const unknown = run('--suite', 'unknown');

  assert.equal(missing.status, 2);
  assert.match(missing.stderr, /--suite requires a value/);
  assert.equal(unknown.status, 2);
  assert.match(unknown.stderr, /invalid --suite value: unknown/);
});

test('gateway compose wrapper rejects unknown options', () => {
  const result = run('--unknown');

  assert.equal(result.status, 2);
  assert.match(result.stderr, /unknown argument: --unknown/);
});
