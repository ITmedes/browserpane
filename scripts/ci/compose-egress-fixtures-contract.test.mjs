import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';

const root = path.resolve(import.meta.dirname, '../..');
const helperPath = path.join(root, 'scripts/ci/start-compose-egress-fixtures.sh');
const cleanupPath = path.join(root, 'scripts/ci/cleanup-compose.sh');
const helper = fs.readFileSync(helperPath, 'utf8');
const cleanup = fs.readFileSync(cleanupPath, 'utf8');

test('egress fixture shell scripts have valid Bash syntax', () => {
  for (const script of [helperPath, cleanupPath]) {
    const result = spawnSync('bash', ['-n', script], { encoding: 'utf8' });
    assert.equal(result.status, 0, result.stderr);
  }
});

test('egress fixture startup owns CA preparation, all observers, and bounded readiness', () => {
  assert.match(helper, /prepare-mitmproxy-ca\.sh/);
  assert.match(helper, /openssl req/);
  assert.match(helper, /docker network inspect/);
  assert.match(helper, /compose\.yml/);
  assert.match(helper, /compose\.tls\.yml/);
  assert.match(helper, /BPANE_EGRESS_FIXTURE_MAX_ATTEMPTS/);
  assert.match(helper, /--max-time 5/);
  assert.match(helper, /127\.0\.0\.1:\$PLAIN_PORT/);
  assert.match(helper, /127\.0\.0\.1:\$AUTH_PORT/);
  assert.match(helper, /127\.0\.0\.1:\$TLS_PORT/);
  assert.match(helper, /proxy-user:proxy-pass/);
  assert.match(helper, /\.State\.Status/);
  assert.doesNotMatch(helper, /compose.*logs/);
});

test('compose cleanup removes observer projects before the primary stack', () => {
  const tlsDown = cleanup.indexOf('--project-name "$TLS_PROJECT"');
  const observerDown = cleanup.indexOf('--project-name "$OBSERVER_PROJECT"');
  const primaryDown = cleanup.indexOf('docker compose -f "$COMPOSE_FILE" down');

  assert.ok(tlsDown >= 0);
  assert.ok(observerDown > tlsDown);
  assert.ok(primaryDown > observerDown);
  assert.match(cleanup, /BPANE_EGRESS_OBSERVER_PROJECT:-bpane-ci-egress/);
  assert.match(cleanup, /BPANE_EGRESS_TLS_OBSERVER_PROJECT:-bpane-ci-egress-tls/);
  assert.equal((cleanup.match(/down --volumes --remove-orphans/g) ?? []).length, 3);
});
