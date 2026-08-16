import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import { PrometheusRulesContract } from './prometheus-rules-contract.mjs';

const ROOT = path.resolve(import.meta.dirname, '../..');

test('repository Prometheus rules satisfy the bounded operations contract', () => {
  assert.deepEqual(new PrometheusRulesContract(ROOT).check(), []);
});

test('contract rejects metadata drift, dynamic annotations, and broken runbook links', (context) => {
  const fixture = copyFixture(context);
  const alertPath = path.join(fixture, 'deploy/examples/observability/alert-rules.yml');
  let alerts = fs.readFileSync(alertPath, 'utf8');
  alerts = alerts
    .replace('severity: critical', 'severity: page\n          owner_id: dynamic')
    .replace('summary: BrowserPane gateway metrics are unavailable',
      'summary: BrowserPane gateway {{ $labels.instance }} is unavailable')
    .replace('#browserpanegatewaymetricsunavailable', '#missing-runbook');
  fs.writeFileSync(alertPath, alerts);

  const errors = new PrometheusRulesContract(fixture).check();
  assert(errors.some((error) => error.includes('labels must be exactly')));
  assert(errors.some((error) => error.includes('unsupported severity')));
  assert(errors.some((error) => error.includes('invalid runbook URL')));
  assert(errors.some((error) => error.includes('forbidden fragment owner_id')));
  assert(errors.some((error) => error.includes('forbidden fragment {{')));
});

test('contract rejects missing rules, semantic coverage, and runbook headings', (context) => {
  const fixture = copyFixture(context);
  replace(fixture, 'deploy/examples/observability/recording-rules.yml',
    /      - record: browserpane:gateway_http_requests:rate5m\n        expr:.*\n/u, '');
  replace(fixture, 'deploy/examples/observability/rule-tests.yml',
    '        alertname: BrowserPaneWorkflowEventDeliveryRetrying\n',
    '        alertname: MissingWorkflowRetryAlert\n');
  replace(fixture, 'docs/operations/PROMETHEUS_ALERT_RUNBOOK.md',
    '## BrowserPaneRetentionFailure\n', '## MissingRetentionHeading\n');

  const errors = new PrometheusRulesContract(fixture).check();
  assert(errors.some((error) => error.includes('recording rules missing')));
  assert(errors.some((error) => error.includes('has no semantic rule test')));
  assert(errors.some((error) => error.includes('has no matching runbook heading')));
});

function copyFixture(context) {
  const fixture = fs.mkdtempSync(path.join(os.tmpdir(), 'bpane-prometheus-rules-'));
  context.after(() => fs.rmSync(fixture, { recursive: true, force: true }));
  fs.mkdirSync(path.join(fixture, 'deploy/examples'), { recursive: true });
  fs.cpSync(
    path.join(ROOT, 'deploy/examples/observability'),
    path.join(fixture, 'deploy/examples/observability'),
    { recursive: true },
  );
  fs.mkdirSync(path.join(fixture, 'docs/operations'), { recursive: true });
  fs.copyFileSync(
    path.join(ROOT, 'docs/operations/PROMETHEUS_ALERT_RUNBOOK.md'),
    path.join(fixture, 'docs/operations/PROMETHEUS_ALERT_RUNBOOK.md'),
  );
  return fixture;
}

function replace(root, relativePath, search, replacement) {
  const target = path.join(root, relativePath);
  const source = fs.readFileSync(target, 'utf8');
  const updated = source.replace(search, replacement);
  assert.notEqual(updated, source, `fixture replacement did not match ${relativePath}`);
  fs.writeFileSync(target, updated);
}
