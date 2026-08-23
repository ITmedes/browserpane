import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

const root = path.resolve(import.meta.dirname, '../..');
const helperPath = path.join(root, 'scripts/ci/start-compose-egress-fixtures.sh');
const cleanupPath = path.join(root, 'scripts/ci/cleanup-compose.sh');
const tlsComposePath = path.join(root, 'deploy/examples/egress-observer/compose.tls.yml');
const helper = fs.readFileSync(helperPath, 'utf8');
const cleanup = fs.readFileSync(cleanupPath, 'utf8');
const tlsCompose = fs.readFileSync(tlsComposePath, 'utf8');

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

test('TLS observer isolates host CA ownership from its unprivileged runtime', () => {
  assert.match(tlsCompose, /:\/bootstrap:ro/);
  assert.match(tlsCompose, /tmpfs:/);
  assert.match(tlsCompose, /\/home\/mitmproxy\/\.mitmproxy:mode=0700,uid=1000,gid=1000/);
  assert.match(tlsCompose, /chown -R mitmproxy:mitmproxy/);
  assert.match(tlsCompose, /exec gosu mitmproxy/);
  assert.doesNotMatch(tlsCompose, /confdir=/);
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
  assert.match(cleanup, /RUN_NAMESPACE:0:32/);
  assert.equal((cleanup.match(/down --volumes --remove-orphans/g) ?? []).length, 3);
});

test('compose cleanup reports failure after attempting every cleanup boundary', (t) => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'bpane-cleanup-fixture-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  const dockerPath = path.join(directory, 'docker');
  const callsPath = path.join(directory, 'calls.log');
  fs.writeFileSync(dockerPath, [
    '#!/usr/bin/env bash',
    'printf "%s\\n" "$*" >>"$BPANE_FAKE_DOCKER_CALLS"',
    'if [[ "$1" == "ps" ]]; then exit 0; fi',
    'if [[ "$*" == *"--project-name bpane-ci-egress -f"* ]]; then exit 7; fi',
    'exit 0',
    '',
  ].join('\n'), { mode: 0o700 });

  const result = spawnSync(cleanupPath, [], {
    encoding: 'utf8',
    env: {
      ...process.env,
      PATH: `${directory}:${process.env.PATH}`,
      BPANE_FAKE_DOCKER_CALLS: callsPath,
    },
  });
  const calls = fs.readFileSync(callsPath, 'utf8');

  assert.equal(result.status, 1);
  assert.match(calls, /bpane-ci-egress-tls/);
  assert.match(calls, /bpane-ci-egress -f/);
  assert.match(calls, /deploy\/compose\.yml down --volumes --remove-orphans/);
});

test('compose cleanup removes only containers owned by the current run namespace', (t) => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'bpane-cleanup-ownership-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  const dockerPath = path.join(directory, 'docker');
  const callsPath = path.join(directory, 'calls.log');
  const removedPath = path.join(directory, 'removed');
  fs.writeFileSync(dockerPath, [
    '#!/usr/bin/env bash',
    'printf "%s\\n" "$*" >>"$BPANE_FAKE_DOCKER_CALLS"',
    'if [[ "$1" == "ps" ]]; then',
    '  [[ -f "$BPANE_FAKE_REMOVED" ]] || printf "%s\\n" owned-container',
    '  printf "%s\\n" foreign-container',
    '  exit 0',
    'fi',
    'if [[ "$1" == "inspect" ]]; then',
    '  if [[ "${@: -1}" == "owned-container" ]]; then',
    '    printf "%s\\n" bpane-32632472208-1-admin-compatibility-stage',
    '  else',
    '    printf "%s\\n" bpane-foreign-run-stage',
    '  fi',
    '  exit 0',
    'fi',
    'if [[ "$1" == "rm" && "${@: -1}" == "owned-container" ]]; then',
    '  touch "$BPANE_FAKE_REMOVED"',
    'fi',
    'exit 0',
    '',
  ].join('\n'), { mode: 0o700 });

  const result = spawnSync(cleanupPath, [], {
    encoding: 'utf8',
    env: {
      ...process.env,
      PATH: `${directory}:${process.env.PATH}`,
      BPANE_CI_RUN_NAMESPACE: 'bpane-32632472208-1-admin-compatibility',
      BPANE_FAKE_DOCKER_CALLS: callsPath,
      BPANE_FAKE_REMOVED: removedPath,
    },
  });
  const calls = fs.readFileSync(callsPath, 'utf8');

  assert.equal(result.status, 0, result.stderr);
  assert.match(calls, /rm --force owned-container/);
  assert.doesNotMatch(calls, /rm --force foreign-container/);
});
