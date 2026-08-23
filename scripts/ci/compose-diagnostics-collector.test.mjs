import assert from 'node:assert/strict';
import test from 'node:test';

import { ComposeDiagnosticsCollector } from './compose-diagnostics-collector.mjs';
import { DiagnosticRedactor } from './diagnostic-redactor.mjs';

test('compose diagnostics use bounded, selected commands and redact every result', () => {
  const calls = [];
  const executor = {
    run(command, args, cwd) {
      calls.push({ command, args, cwd });
      return {
        status: 0,
        stdout: 'gateway Authorization: Bearer do-not-upload',
        stderr: '',
        error: null
      };
    }
  };
  const collector = new ComposeDiagnosticsCollector(executor, new DiagnosticRedactor());

  const runNamespace = 'bpane-32632472208-1-admin-compatibility';
  const result = collector.collect('/repo', { BPANE_CI_RUN_NAMESPACE: runNamespace });

  assert.equal(calls.length, 5);
  assert.ok(calls.every((call) => call.command === 'docker' && call.cwd === '/repo'));
  assert.ok(calls.some((call) => call.args.includes('--tail') && call.args.includes('300')));
  assert.ok(calls.some((call) => call.args.includes(
    `bpane-ci-egress-${runNamespace.slice(0, 32)}`,
  )));
  assert.ok(calls.every((call) => !call.args.includes('inspect')));
  assert.ok(!result.includes('do-not-upload'));
  assert.match(result, /Control-plane service logs/);
  assert.match(result, /Egress observer service status/);
  assert.match(result, /TLS egress observer service status/);
});

test('compose diagnostics truncate oversized sections after redaction', () => {
  const executor = {
    run() {
      return { status: 1, stdout: 'x'.repeat(500_001), stderr: '', error: null };
    }
  };
  const result = new ComposeDiagnosticsCollector(executor, new DiagnosticRedactor())
    .collect('/repo');

  assert.match(result, /<truncated>/);
  assert.ok(result.length < 2_501_000);
});
